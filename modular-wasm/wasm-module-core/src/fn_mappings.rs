use modular_core::{
    get_str, Callback, CallbackError, CallbackSuccess, Module, NativeByteSlice, NativeModule,
};
use std::ptr::null_mut;

extern "C" {
    fn __wm_callback_on_success(ptr: i32, data_ptr: *const u8, data_len: usize);
    fn __wm_callback_on_error(
        ptr: i32,
        code: i32,
        err_name: *const u8,
        err_name_len: usize,
        err_description: *const u8,
        err_description_len: usize,
        err_data: *const u8,
        err_data_len: usize,
    );
}

#[no_mangle]
extern "C" fn __wm_module_package(module: &NativeModule, dest: &mut *const u8, len: &mut usize) {
    *dest = module.package().as_ptr();
    *len = module.package().len();
}

#[no_mangle]
extern "C" fn __wm_module_version(module: &NativeModule, dest: &mut *const u8, len: &mut usize) {
    *dest = module.version().as_ptr();
    *len = module.version().len();
}

#[no_mangle]
extern "C" fn __wm_module_invoke(
    module: &NativeModule,
    method: NativeByteSlice,
    data: NativeByteSlice,
    callback_id: i32,
) {
    let method = get_str!(method, method);
    let data: Option<&[u8]> = data.into();

    println!("method: {:?}, data: {:?}", method, data);

    struct WasmCallback {
        callback_id: i32,
    }

    impl Callback for WasmCallback {
        fn on_success(&self, result: CallbackSuccess) {
            unsafe {
                __wm_callback_on_success(
                    self.callback_id,
                    result.data.map(|i| i.as_ptr()).unwrap_or(null_mut()),
                    result.data.map(|i| i.len()).unwrap_or(0),
                );
            }
        }

        fn on_error(&self, err: CallbackError) {
            unsafe {
                __wm_callback_on_error(
                    self.callback_id,
                    err.code,
                    err.err_name
                        .map(|i| i.as_bytes().as_ptr())
                        .unwrap_or(null_mut()),
                    err.err_name.map(|i| i.len()).unwrap_or(0),
                    err.description
                        .map(|i| i.as_bytes().as_ptr())
                        .unwrap_or(null_mut()),
                    err.description.map(|i| i.len()).unwrap_or(0),
                    err.data.map(|i| i.as_ptr()).unwrap_or(null_mut()),
                    err.data.map(|i| i.len()).unwrap_or(0),
                )
            }
        }
    }

    module.invoke(method, data, Box::new(WasmCallback { callback_id }))
}
