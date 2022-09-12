use modular_core::{
    Callback, CallbackError, CallbackSuccess, Module, NativeModule, NativeRegistry, Registry,
};

struct Module1 {
    registry: NativeRegistry,
}

impl Module for Module1 {
    fn package(&self) -> &str {
        "wasm-example.module2"
    }

    fn version(&self) -> &str {
        "0.0.1"
    }

    fn run(&self) {
        println!("Module1::run");
    }

    fn invoke(&self, _: &str, _: Option<&[u8]>, callback: Box<dyn Callback>) {
        struct TestCallback {}

        impl Callback for TestCallback {
            fn on_success(&self, result: CallbackSuccess) {
                println!("module2::on_success: {:?}", result.data)
            }

            fn on_error(&self, err: CallbackError) {
                println!("module2::on_err: {:?}", err.code)
            }
        }

        self.registry.invoke(
            "wasm-example.module3",
            "hello!",
            None,
            Box::new(TestCallback {}),
        );

        callback.on_success(CallbackSuccess {
            data: Some(b"Module1::invoke"),
        });

        callback.on_success(CallbackSuccess {
            data: Some(b"Module1::invoke"),
        });
    }
}

#[no_mangle]
pub extern "C" fn create_module(registry: NativeRegistry) -> NativeModule {
    NativeModule::new(Module1 { registry })
}
