use modular_core::{
    Callback, CallbackError, CallbackSuccess, Module, NativeModule, NativeRegistry, Registry,
};
use native_recorder::{register_module_tracer, NativeBytesRecorder};
use tracing::{error, info, instrument};

struct Module2 {
    registry: NativeRegistry,
}

impl Module2 {
    #[instrument(skip(registry))]
    pub fn new(registry: NativeRegistry) -> Self {
        info!("hello from module2");
        Self { registry }
    }
}

impl Module for Module2 {
    #[instrument(skip(self))]
    fn package(&self) -> &str {
        "dll.module2"
    }

    #[instrument(skip(self))]
    fn version(&self) -> &str {
        "0.0.1"
    }

    #[instrument(skip(self))]
    fn run(&self) {
        info!("dll.module2::run");
    }

    #[instrument(skip(self, callback))]
    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        struct TestCallback {}

        impl Callback for TestCallback {
            #[instrument(
                skip(self, result),
                fields(
                    data = ?result.data,
                    data_len = result.data.map(|i| i.len()).unwrap_or_default()
                )
            )]
            fn on_success(&self, result: CallbackSuccess) {
                info!("dll.module2::on_success: {:?}", result.data)
            }

            #[instrument(
                skip(self, err),
                fields(
                    code = %err.code,
                    err_name = %err.err_name.unwrap_or_default(),
                    description = %err.description.unwrap_or_default()
                )
            )]
            fn on_error(&self, err: CallbackError) {
                error!("dll.module2::on_err: {:?}", err.code)
            }
        }

        info!(a = "hello", a = "world", "test with fields");

        self.registry
            .invoke("wasm.module", "hello!", None, Box::new(TestCallback {}));

        callback.on_success(CallbackSuccess {
            data: Some(b"dll.module2::invoke"),
        });

        callback.on_success(CallbackSuccess {
            data: Some(b"dll.module2::invoke"),
        });
    }
}

#[instrument(skip(registry, recorder))]
#[no_mangle]
pub extern "C" fn create_module(
    registry: NativeRegistry,
    recorder: NativeBytesRecorder,
) -> NativeModule {
    register_module_tracer(Box::leak(Box::new(recorder)));

    NativeModule::new(Module2::new(registry))
}
