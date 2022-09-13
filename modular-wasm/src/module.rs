use crate::registry_imports::registry_invoke;
use crate::state::WasmModuleState;
use crate::utils::{get_uid, read_bytes, read_string};
use crate::vtable::WasmModuleVTable;
use modular_core::*;
use parking_lot::lock_api::MutexGuard;
use parking_lot::{Mutex, RawMutex, RwLock};
use std::time::Duration;
use tracing::error;
use uuid::Uuid;
use wasmer::*;
use wasmer_wasi::WasiState;

pub struct WasmModule {
    _instance: Instance,
    store: Mutex<Store>,
    memory: Memory,

    instance_ptr: i32,
    vtable: WasmModuleVTable,

    package: String,
    version: String,

    state: FunctionEnv<WasmModuleState>,
}

impl WasmModule {
    pub fn new<B: AsRef<[u8]>, R: Registry + 'static>(
        bytes: B,
        registry: R,
    ) -> anyhow::Result<Self> {
        let mut store = Store::new(Cranelift::default());
        let module = wasmer::Module::new(&store, bytes)?;

        let wasi_env = WasiState::new("wasm_module")
            .stdout(Box::new(wasmer_wasi::Stdout))
            .map_dir("/", ".")?
            .finalize(&mut store)?;

        let state = WasmModuleState::new(registry);
        let env = FunctionEnv::new(&mut store, state);

        let mut wms_imports = WasmModule::generate_imports(&mut store, &env);
        wms_imports.extend(wasi_env.import_object(&mut store, &module)?.into_iter());

        let instance = Instance::new(&mut store, &module, &wms_imports)?;
        let memory = instance.exports.get_memory("memory")?.clone();
        wasi_env.data_mut(&mut store).set_memory(memory.clone());
        env.as_mut(&mut store).set_memory(memory.clone());

        let vtable = WasmModuleVTable::new(&instance, &store)?;
        env.as_mut(&mut store).set_vtable(&vtable);

        let instance_ptr = vtable.create(&mut store)?;
        let package = vtable.package(instance_ptr, &mut store, &memory)?;
        let version = vtable.version(instance_ptr, &mut store, &memory)?;

        Ok(Self {
            store: Mutex::new(store),
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
        let mut store = self.store.lock();

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
                .create_native_byte_slice(Some(action), &mut *store, &self.memory),
            -10000,
            "WasmMemError",
            callback
        );

        let data_ptr = call!(
            self.vtable
                .create_native_byte_slice(data, &mut *store, &self.memory),
            -10000,
            "WasmMemError",
            callback
        );

        let id = Uuid::new_v4();
        let id_ptr = call!(
            self.vtable.write_bytes(&id, &mut *store, &self.memory),
            -10000,
            "WasmMemError",
            callback
        );

        self.state.as_mut(&mut *store).add_callback(id, callback);

        let result =
            self.vtable
                .invoke(self.instance_ptr, action_ptr, data_ptr, id_ptr, &mut *store);

        let callback = self.state.as_mut(&mut *store).remove_callback(&id);

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
                "__wm_registry_invoke" => Function::new_typed_with_env(store, function_env, registry_invoke),
            }
        }
    }
}

fn on_success_fn(mut env: FunctionEnvMut<WasmModuleState>, ptr: i32, data_ptr: i32, data_len: i32) {
    let mem = env.data_mut().get_memory().cloned().unwrap();
    let uid = get_uid(&mem, ptr, &env);

    let data = read_bytes(&mem, data_ptr, data_len, &env);

    env.data().get_callback(&uid).on_success(CallbackSuccess {
        data: data.as_deref(),
    });
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
    let mem = env.data_mut().get_memory().cloned().unwrap();
    let uid = get_uid(&mem, ptr, &env);

    let err_name = read_string(&mem, err_name_ptr, err_name_len as _, &env);
    let err_description = read_string(&mem, err_description_ptr, err_description_len as _, &env);
    let err_data = read_bytes(&mem, err_data_ptr, err_data_len, &env);

    env.data().get_callback(&uid).on_error(CallbackError {
        code,
        err_name: err_name.as_deref(),
        description: err_description.as_deref(),
        data: err_data.as_deref(),
    });
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

impl Drop for WasmModule {
    fn drop(&mut self) {
        if let Err(err) = self
            .vtable
            .destroy(self.instance_ptr, &mut *self.store.lock())
        {
            error!("Failed to destroy wasm module: {}", err);
        }
    }
}
