use wasmer::*;

// Arg1 - instance
// Arg2 - *mut *const u8
// Arg3 = *mut usize
pub type GetStringFunction = TypedFunction<(i32, i32, i32), ()>;

#[derive(Clone)]
pub struct WasmModuleVTable {
    __wm_alloc: TypedFunction<u32, i32>,
    __wm_free: TypedFunction<(i32, u32), ()>,

    __wm_create: TypedFunction<(), i32>,

    __wm_module_package: GetStringFunction,
    __wm_module_version: GetStringFunction,
    __wm_module_invoke: TypedFunction<(i32, i32, i32, i32), ()>,
    __wm_host_callback_on_success: TypedFunction<(i32, i32), ()>,
    __wm_host_callback_on_error: TypedFunction<(i32, i32, i32, i32, i32), ()>,
    __wm_host_callback_destroy: TypedFunction<i32, ()>,
    __wm_module_destroy: TypedFunction<i32, ()>,
}

// extern "C" fn __wm_host_callback_on_success(callback: &mut NativeCallback, data: NativeByteSlice) {
// extern "C" fn __wm_host_callback_on_error(
//     callback: &mut NativeCallback,
//     code: i32,
//     err_name: NativeByteSlice,
//     err_description: NativeByteSlice,
//     err_data: NativeByteSlice,
// )
// extern "C" fn __wm_host_callback_destroy(callback: *mut NativeCallback) {
// extern "C" fn __wm_module_destroy(module: *mut NativeModule) {

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
            __wm_host_callback_on_success: instance
                .exports
                .get_typed_function(store, "__wm_host_callback_on_success")?,
            __wm_host_callback_on_error: instance
                .exports
                .get_typed_function(store, "__wm_host_callback_on_error")?,
            __wm_host_callback_destroy: instance
                .exports
                .get_typed_function(store, "__wm_host_callback_destroy")?,
            __wm_module_destroy: instance
                .exports
                .get_typed_function(store, "__wm_module_destroy")?,
        })
    }

    pub fn destroy(&self, ptr: i32, store: &mut Store) -> anyhow::Result<()> {
        Ok(self.__wm_module_destroy.call(store, ptr)?)
    }

    pub fn callback_on_success(
        &self,
        callback: i32,
        data: Option<&[u8]>,
        store: &mut impl AsStoreMut,
        memory: &Memory,
    ) -> anyhow::Result<()> {
        let data_ptr = self.create_native_byte_slice(data, store, memory)?;
        self.__wm_host_callback_on_success
            .call(store, callback, data_ptr)?;
        self.free_native_byte_slice(data_ptr, store, memory)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn callback_on_error(
        &self,
        callback: i32,
        code: i32,
        err_name: Option<&[u8]>,
        err_description: Option<&[u8]>,
        err_data: Option<&[u8]>,
        store: &mut impl AsStoreMut,
        memory: &Memory,
    ) -> anyhow::Result<()> {
        let err_name_ptr = self.create_native_byte_slice(err_name, store, memory)?;
        let err_description_ptr = self.create_native_byte_slice(err_description, store, memory)?;
        let err_data_ptr = self.create_native_byte_slice(err_data, store, memory)?;
        self.__wm_host_callback_on_error.call(
            store,
            callback,
            code,
            err_name_ptr,
            err_description_ptr,
            err_data_ptr,
        )?;
        self.free_native_byte_slice(err_name_ptr, store, memory)?;
        self.free_native_byte_slice(err_description_ptr, store, memory)?;
        self.free_native_byte_slice(err_data_ptr, store, memory)?;
        Ok(())
    }

    pub fn callback_destroy(&self, ptr: i32, store: &mut impl AsStoreMut) -> anyhow::Result<()> {
        Ok(self.__wm_host_callback_destroy.call(store, ptr)?)
    }

    pub fn create(&self, store: &mut impl AsStoreMut) -> anyhow::Result<i32> {
        Ok(self.__wm_create.call(store)?)
    }

    pub fn alloc(&self, len: u32, store: &mut impl AsStoreMut) -> anyhow::Result<i32> {
        Ok(self.__wm_alloc.call(store, len)?)
    }

    pub fn free(&self, ptr: i32, len: u32, store: &mut impl AsStoreMut) -> anyhow::Result<()> {
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
        store: &mut impl AsStoreMut,
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
        store: &mut impl AsStoreMut,
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
        store: &mut impl AsStoreMut,
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
        store: &mut impl AsStoreMut,
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
