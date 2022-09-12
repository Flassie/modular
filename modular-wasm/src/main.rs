use modular_core::{Callback, CallbackError, CallbackSuccess};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::convert::Into;
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;
use wasmer::{
    imports, AsStoreRef, Cranelift, Engine, Function, FunctionEnv, FunctionEnvMut, Imports,
    Instance, Memory, Module, Store, TypedFunction, WasmPtr, WasmSlice,
};
use wasmer_wasi::WasiState;

struct OptionalCallback(Option<Box<dyn Callback>>);

impl Callback for OptionalCallback {
    fn on_success(&self, result: CallbackSuccess) {
        if let Some(callback) = &self.0 {
            callback.on_success(result);
        }
    }

    fn on_error(&self, err: CallbackError) {
        if let Some(callback) = &self.0 {
            callback.on_error(err);
        }
    }
}

pub struct WasmModule {
    store: RwLock<Store>,
    _instance: Instance,
    memory: Memory,

    instance_ptr: i32,
    vtable: WasmModuleVTable,

    package: String,
    version: String,

    state: FunctionEnv<WasmModuleState>,
}

// Arg1 - instance
// Arg2 - *mut *const u8
// Arg3 = *mut usize
type GetStringFunction = TypedFunction<(i32, i32, i32), ()>;

struct WasmModuleVTable {
    __wm_alloc: TypedFunction<u32, i32>,
    __wm_free: TypedFunction<(i32, u32), ()>,

    __wm_create: TypedFunction<(), i32>,

    __wm_module_package: GetStringFunction,
    __wm_module_version: GetStringFunction,
    __wm_module_invoke: TypedFunction<(i32, i32, i32, i32), ()>,
}

impl WasmModuleVTable {
    pub fn new(instance: &Instance, store: &Store) -> anyhow::Result<Self> {
        Ok(Self {
            __wm_alloc: instance.exports.get_typed_function(store, "__wm_alloc")?,
            __wm_free: instance.exports.get_typed_function(store, "__wm_free")?,

            __wm_create: instance.exports.get_typed_function(store, "__wm_create")?,

            __wm_module_package: instance
                .exports
                .get_typed_function(store, "__wm_module_package")?,
            __wm_module_version: instance
                .exports
                .get_typed_function(store, "__wm_module_version")?,
            __wm_module_invoke: instance
                .exports
                .get_typed_function(store, "__wm_module_invoke")?,
        })
    }

    pub fn create(&self, store: &mut Store) -> anyhow::Result<i32> {
        Ok(self.__wm_create.call(store)?)
    }

    pub fn alloc(&self, len: u32, store: &mut Store) -> anyhow::Result<i32> {
        Ok(self.__wm_alloc.call(store, len)?)
    }

    pub fn free(&self, ptr: i32, len: u32, store: &mut Store) -> anyhow::Result<()> {
        Ok(self.__wm_free.call(store, ptr, len)?)
    }

    pub fn package(
        &self,
        instance: i32,
        store: &mut Store,
        mem: &Memory,
    ) -> anyhow::Result<String> {
        self.call_get_string(&self.__wm_module_package, instance, store, mem)
    }

    pub fn version(
        &self,
        instance: i32,
        store: &mut Store,
        mem: &Memory,
    ) -> anyhow::Result<String> {
        self.call_get_string(&self.__wm_module_version, instance, store, mem)
    }

    pub fn invoke(
        &self,
        instance: i32,
        action: i32,
        data: i32,
        callback: i32,
        store: &mut Store,
    ) -> anyhow::Result<()> {
        Ok(self
            .__wm_module_invoke
            .call(store, instance, action, data, callback)?)
    }

    pub fn create_native_byte_slice<B: AsRef<[u8]>>(
        &self,
        bytes: Option<B>,
        store: &mut Store,
        mem: &Memory,
    ) -> anyhow::Result<i32> {
        let bytes = bytes.as_ref().map(|i| i.as_ref());
        let len = bytes.map(|i| i.len()).unwrap_or(0) as u32;

        let bytes_ptr = match bytes {
            Some(bytes) => self.write_bytes(bytes, store, mem)?,
            None => 0,
        };

        let native_byte_slice_ptr = self.alloc(8, store)?;

        let view = mem.view(&store);

        let slice = WasmSlice::<u32>::new(&view, native_byte_slice_ptr as u64, 2)?;
        slice.write_slice(&[bytes_ptr as u32, len as u32])?;

        Ok(native_byte_slice_ptr)
    }

    pub fn free_native_byte_slice(
        &self,
        native_byte_slice_ptr: i32,
        store: &mut Store,
        mem: &Memory,
    ) -> anyhow::Result<()> {
        let view = mem.view(&store);

        let slice = WasmSlice::<u32>::new(&view, native_byte_slice_ptr as u64, 2)?;
        let bytes_ptr = slice.read(0)? as i32;

        self.free(bytes_ptr, 4, store)?;
        self.free(native_byte_slice_ptr, 8, store)?;

        Ok(())
    }

    pub fn write_bytes<B: AsRef<[u8]>>(
        &self,
        bytes: B,
        store: &mut Store,
        mem: &Memory,
    ) -> anyhow::Result<i32> {
        let bytes = bytes.as_ref();
        let len = bytes.len() as u32;
        let ptr = self.alloc(len, store)?;
        let view = mem.view(&store);
        view.write(ptr as _, bytes)?;
        Ok(ptr)
    }

    pub fn call_get_string(
        &self,
        f: &GetStringFunction,
        instance: i32,
        store: &mut Store,
        mem: &Memory,
    ) -> anyhow::Result<String> {
        let str = self.alloc(4, store)?;
        let len = self.alloc(4, store)?;

        f.call(store, instance, str, len)?;

        let ptr = WasmPtr::<u32>::new(str as _);
        let len_value = WasmPtr::<u32>::new(len as _);

        let mem_view = mem.view(&store);
        let dest_ptr = ptr.deref(&mem_view);
        let dest_ptr = dest_ptr.read()?;

        let len_value = len_value.deref(&mem_view);
        let len_value = len_value.read()?;

        let slice = WasmSlice::<u8>::new(&mem_view, dest_ptr as _, len_value as _)?;
        let data = slice.read_to_vec()?;

        self.free(str, 4, store)?;
        self.free(len, 4, store)?;

        Ok(String::from_utf8(data)?)
    }
}

struct WasmModuleState {
    callbacks: HashMap<Uuid, Box<dyn Callback>>,
    memory: Option<Memory>,
}

impl WasmModule {
    pub fn new<B: AsRef<[u8]>>(bytes: B, engine: impl Into<Engine>) -> anyhow::Result<Self> {
        let mut store = Store::new(engine);
        let module = Module::new(&store, bytes)?;

        let wasi_env = WasiState::new("wasm_module")
            .stdout(Box::new(wasmer_wasi::Stdout))
            .map_dir("/", ".")?
            .finalize(&mut store)?;

        let state = WasmModuleState {
            memory: None,
            callbacks: HashMap::new(),
        };
        let env = FunctionEnv::new(&mut store, state);

        let mut wms_imports = WasmModule::generate_imports(&mut store, &env);
        wms_imports.extend(wasi_env.import_object(&mut store, &module)?.into_iter());

        let instance = Instance::new(&mut store, &module, &wms_imports)?;
        let memory = instance.exports.get_memory("memory")?.clone();
        wasi_env.data_mut(&mut store).set_memory(memory.clone());
        env.as_mut(&mut store).memory = Some(memory.clone());

        let vtable = WasmModuleVTable::new(&instance, &store)?;
        let instance_ptr = vtable.create(&mut store)?;
        let package = vtable.package(instance_ptr, &mut store, &memory)?;
        let version = vtable.version(instance_ptr, &mut store, &memory)?;

        Ok(Self {
            store: RwLock::new(store),
            _instance: instance,
            vtable,
            instance_ptr,
            version,
            package,
            memory,
            state: env,
        })
    }

    pub fn invoke(&self, action: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        let mut store = self.store.write();

        macro_rules! call {
            ($expr:expr, $code:literal, $err_name:literal, $callback:ident) => {
                match $expr {
                    Ok(v) => v,
                    Err(e) => {
                        callback.on_error(modular_core::CallbackError {
                            code: $code,
                            err_name: Some($err_name),
                            description: Some(&format!("{:#?}", e)),
                            data: None,
                        });
                        return;
                    }
                }
            };
        }

        let action_ptr = call!(
            self.vtable
                .create_native_byte_slice(Some(action), &mut store, &self.memory),
            -10000,
            "WasmMemError",
            callback
        );

        let data_ptr = call!(
            self.vtable
                .create_native_byte_slice(data, &mut store, &self.memory),
            -10000,
            "WasmMemError",
            callback
        );

        let id = Uuid::new_v4();
        let id_ptr = call!(
            self.vtable.write_bytes(&id, &mut store, &self.memory),
            -10000,
            "WasmMemError",
            callback
        );

        {
            let state = self.state.as_mut(&mut *store);
            state.callbacks.insert(id, callback);
        }

        let result =
            self.vtable
                .invoke(self.instance_ptr, action_ptr, data_ptr, id_ptr, &mut *store);

        let callback = {
            let state = self.state.as_mut(&mut *store);
            OptionalCallback(state.callbacks.remove(&id))
        };

        if let Err(err) = result {
            error!("Failed to invoke wasm function: {}", err);
            callback.on_error(CallbackError {
                code: -10001,
                err_name: Some("WasmInvokeError"),
                description: Some(&format!("{:#?}", err)),
                data: None,
            });
        }

        if let Err(err) = self
            .vtable
            .free_native_byte_slice(action_ptr, &mut *store, &self.memory)
        {
            error!("Failed to free native byte slice: {}", err);
        }

        if let Err(err) = self
            .vtable
            .free_native_byte_slice(data_ptr, &mut *store, &self.memory)
        {
            error!("Failed to free native byte slice: {}", err);
        }

        if let Err(err) = self
            .vtable
            .free(id_ptr, id.as_ref().len() as _, &mut *store)
        {
            error!("Failed to free native byte slice: {}", err);
        }
    }

    fn generate_imports(store: &mut Store, function_env: &FunctionEnv<WasmModuleState>) -> Imports {
        imports! {
            "env" => {
                "__wm_callback_on_success" => Function::new_typed_with_env(store, function_env, on_success_fn),
                "__wm_callback_on_error" => Function::new_typed_with_env(store, function_env, on_err_fn),
            }
        }
    }
}

#[inline]
fn get_uid(memory: &Memory, ptr: i32, store: &impl AsStoreRef) -> Uuid {
    let view = memory.view(store);

    let uid = WasmPtr::<u128>::new(ptr as _).read(&view).unwrap();
    Uuid::from_u128(uid)
}

#[inline]
fn read_bytes(memory: &Memory, ptr: i32, len: i32, store: &impl AsStoreRef) -> Option<Vec<u8>> {
    let view = memory.view(store);

    let slice = WasmSlice::<u8>::new(&view, ptr as _, len as _).ok()?;
    slice.read_to_vec().ok()
}

#[inline]
fn read_string(memory: &Memory, ptr: i32, len: u32, store: &impl AsStoreRef) -> Option<String> {
    let bytes = read_bytes(memory, ptr, len as _, store)?;
    String::from_utf8(bytes).ok()
}

fn on_success_fn(mut env: FunctionEnvMut<WasmModuleState>, ptr: i32, data_ptr: i32, data_len: i32) {
    let mem = env.data_mut().memory.as_ref().cloned().unwrap();
    let uid = get_uid(&mem, ptr, &env);

    let data = read_bytes(&mem, data_ptr, data_len, &env);

    if let Some(callback) = env.data().callbacks.get(&uid) {
        callback.on_success(CallbackSuccess {
            data: data.as_deref(),
        });
    } else {
        error!("callback not found for id: {}", uid);
    }
}

#[allow(clippy::too_many_arguments)]
fn on_err_fn(
    mut env: FunctionEnvMut<WasmModuleState>,
    ptr: i32,
    code: i32,
    err_name_ptr: i32,
    err_name_len: i32,
    err_description_ptr: i32,
    err_description_len: i32,
    err_data_ptr: i32,
    err_data_len: i32,
) {
    let mem = env.data_mut().memory.as_ref().cloned().unwrap();
    let uid = get_uid(&mem, ptr, &env);

    let err_name = read_string(&mem, err_name_ptr, err_name_len as _, &env);
    let err_description = read_string(&mem, err_description_ptr, err_description_len as _, &env);
    let err_data = read_bytes(&mem, err_data_ptr, err_data_len, &env);

    if let Some(v) = env.data().callbacks.get(&uid) {
        v.on_error(CallbackError {
            code,
            err_name: err_name.as_deref(),
            description: err_description.as_deref(),
            data: err_data.as_deref(),
        });
    } else {
        error!("callback not found for id: {}", uid);
    }
}

impl modular_core::Module for WasmModule {
    fn package(&self) -> &str {
        &self.package
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn run(&self) {}

    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        WasmModule::invoke(self, method, data, callback)
    }
}

fn main() {
    let module = WasmModule::new(
        include_bytes!("../../target/wasm32-wasi/debug/wasm_example.wasm"),
        Cranelift::default(),
    )
    .unwrap();

    struct WasmCallback {}
    impl Callback for WasmCallback {
        fn on_success(&self, result: CallbackSuccess) {
            println!("ON SUCCESS!!!")
        }

        fn on_error(&self, err: CallbackError) {
            println!("ON ERROR!")
        }
    }

    module.invoke("hello, world!", None, Box::new(WasmCallback {}));

    // let engine = Cranelift::default();
    // let mut store = Store::new(engine);
    // let wasi_ctx = wasmer_wasi::WasiState::new("wasi-test")
    //     .stdout(Box::new(wasmer_wasi::Stdout))
    //     .finalize(&mut store)
    //     .unwrap();
    //
    // let module = Module::new(
    //     &store,
    //     include_bytes!("../../target/wasm32-wasi/debug/wasm_example.wasm"),
    // )
    // .unwrap();
    //
    // let obj = wasi_ctx.import_object(&mut store, &module).unwrap();
    //
    // struct State {
    //     callbacks: HashMap<u32, ()>,
    // }
    //
    // let state = State {
    //     callbacks: Default::default(),
    // };
    //
    // let fn_env = FunctionEnv::new(&mut store, state);
    //
    // let on_success = Function::new_typed_with_env(&mut store, &fn_env, on_success_fn);
    // let on_error = Function::new_typed_with_env(&mut store, &fn_env, on_err_fn);
    //
    // let mut imports_object = imports! {
    //     "env" => {
    //         "__wm_callback_on_success" => on_success,
    //         "__wm_callback_on_error" => on_error,
    //     },
    // };
    //
    // imports_object.extend(obj.into_iter());
    //
    // let instance = Instance::new(&mut store, &module, &imports_object).unwrap();
    // let mem = instance.exports.get_memory("memory").unwrap();
    // wasi_ctx.data_mut(&mut store).set_memory(mem.clone());
    //
    // let create_module = instance
    //     .exports
    //     .get_typed_function::<(), i32>(&store, "create_module")
    //     .unwrap();
    // let v = create_module.call(&mut store).unwrap();
    //
    // let version_fn = instance
    //     .exports
    //     .get_typed_function::<i32, i32>(&store, "__wm_module_version")
    //     .unwrap();
    // let version_len_fn = instance
    //     .exports
    //     .get_typed_function::<i32, i32>(&store, "__wm_module_version_len")
    //     .unwrap();
    //
    // let ptr = version_fn.call(&mut store, v).unwrap();
    // let len = version_len_fn.call(&mut store, v).unwrap();
    //
    // let mem = instance.exports.get_memory("memory").unwrap();
    // let view = mem.view(&store);
    //
    // let mut version = vec![0; len as usize];
    // view.read(ptr as u64, &mut version).unwrap();
    //
    // module
    //     .exports()
    //     .functions()
    //     .for_each(|i| println!("{:?}", i));
    //
    // let malloc = instance
    //     .exports
    //     .get_typed_function::<i32, i32>(&store, "__wm_alloc")
    //     .unwrap();
    //
    // let free = instance
    //     .exports
    //     .get_typed_function::<(i32, i32), ()>(&store, "__wm_free")
    //     .unwrap();
    //
    // let invoke_fn = instance
    //     .exports
    //     .get_typed_function::<(i32, i32, i32, i32), ()>(&store, "__wm_module_invoke")
    //     .unwrap();
    //
    // let ptr1 = malloc.call(&mut store, 8).unwrap();
    //
    // let str = "hello, world!";
    // let str_ptr = malloc.call(&mut store, str.len() as i32).unwrap();
    //
    // let view = mem.view(&store);
    // view.write(str_ptr as u64, str.as_bytes()).unwrap();
    // view.write(ptr1 as _, &str_ptr.to_le_bytes()).unwrap();
    // view.write((ptr1 as u64) + 4, &((str.len() as i32).to_le_bytes()))
    //     .unwrap();
    //
    // let ptr2 = malloc.call(&mut store, 8).unwrap();
    // let view = mem.view(&store);
    // view.write(ptr2 as _, &[0, 0, 0, 0]).unwrap();
    // view.write((ptr2 as u64) + 4, &[0, 0, 0, 0]).unwrap();
    //
    // invoke_fn.call(&mut store, v, ptr1, ptr2, 0).unwrap();
    //
    // free.call(&mut store, ptr1, 8).unwrap();
    // free.call(&mut store, ptr2, 8).unwrap();
    // free.call(&mut store, str_ptr, str.len() as _).unwrap();
}
