use wasm_module_core::{Callback, CallbackError, CallbackSuccess, Module, NativeModule};

struct WasmModule {}

impl Module for WasmModule {
    fn package(&self) -> &str {
        "wasm.module"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn run(&self) {}

    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        println!("hello from {:?}", method);
        callback.on_success(CallbackSuccess { data });
        callback.on_error(CallbackError {
            code: 0,
            err_name: None,
            description: None,
            data,
        });
    }
}

#[no_mangle]
extern "C" fn __wm_create() -> *mut NativeModule {
    Box::into_raw(Box::new(NativeModule::new(WasmModule {})))
}
