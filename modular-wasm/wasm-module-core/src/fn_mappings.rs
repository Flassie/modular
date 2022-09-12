use modular_core::{
    get_str, Callback, CallbackError, CallbackSuccess, Module, NativeByteSlice, NativeCallback,
    NativeModule,
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

    fn __wm_registry_invoke(
        package: *const u8,
        package_len: usize,
        method: *const u8,
        method_len: usize,
        data: *const u8,
        data_len: usize,
        callback_id: i32,
    ) -> i32;
}

pub fn registry_invoke<C: Callback + 'static>(
    package: &str,
    method: &str,
    data: Option<&[u8]>,
    callback: C,
) -> i32 {
    let callback = Box::into_raw(Box::new(NativeCallback::new(callback)));
    unsafe {
        __wm_registry_invoke(
            package.as_ptr(),
            package.len(),
            method.as_ptr(),
            method.len(),
            data.map(|i| i.as_ptr()).unwrap_or(null_mut()),
            data.map(|i| i.len()).unwrap_or(0),
            callback as i32,
        )
    }
}

#[no_mangle]
extern "C" fn __wm_host_callback_on_success(callback: &mut NativeCallback, data: NativeByteSlice) {
    let data: Option<&[u8]> = data.into();
    callback.on_success(CallbackSuccess { data });
}

#[no_mangle]
extern "C" fn __wm_host_callback_on_error(
    callback: &mut NativeCallback,
    code: i32,
    err_name: NativeByteSlice,
    err_description: NativeByteSlice,
    err_data: NativeByteSlice,
) {
    let err_name = Option::<&[u8]>::from(err_name).map(|i| String::from_utf8_lossy(i).to_string());
    let err_description =
        Option::<&[u8]>::from(err_description).map(|i| String::from_utf8_lossy(i).to_string());
    let err_data: Option<&[u8]> = err_data.into();

    callback.on_error(CallbackError {
        code,
        err_name: err_name.as_deref(),
        description: err_description.as_deref(),
        data: err_data,
    });
}

#[no_mangle]
extern "C" fn __wm_host_callback_destroy(callback: *mut NativeCallback) {
    if !callback.is_null() {
        unsafe {
            Box::from_raw(callback);
        }
    }
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

#[no_mangle]
extern "C" fn __wm_module_destroy(module: *mut NativeModule) {
    if !module.is_null() {
        unsafe {
            Box::from_raw(module);
        }
    }
}
