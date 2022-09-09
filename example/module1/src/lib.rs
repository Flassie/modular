use modular::{
    Callback, CallbackError, CallbackSuccess, Module, NativeModule, NativeRegistry, Registry,
};

struct Module1 {
    registry: NativeRegistry,
}

impl Module for Module1 {
    fn package(&self) -> &str {
        "example.module1"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn run(&self) {
        struct TestCallback {}

        impl Callback for TestCallback {
            fn on_success(&self, result: CallbackSuccess) {
                println!("module1::on_success: {:?}", result.data)
            }

            fn on_error(&self, err: CallbackError) {
                println!("module1::on_err: {:?}", err.code)
            }
        }

        self.registry
            .invoke("example.module2", "1", None, Box::new(TestCallback {}))
    }

    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        println!("Module1::invoke: method = {}, data = {:?}", method, data);
        callback.on_success(CallbackSuccess {
            data: Some(b"Module1::invoke"),
        });
    }
}

#[no_mangle]
pub extern "C" fn create_module(registry: NativeRegistry) -> NativeModule {
    NativeModule::new(Module1 { registry })
}
