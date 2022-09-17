use modular_core::{
    Callback, CallbackError, CallbackSuccess, Module, NativeModule, NativeRegistry, Registry,
};
use native_recorder::{register_module_tracer, NativeBytesRecorder};
use tracing::{error, info, instrument};

struct Module1 {
    registry: NativeRegistry,
}

impl Module1 {
    #[instrument(skip(registry))]
    pub fn new(registry: NativeRegistry) -> Self {
        info!("hello from module1");
        Self { registry }
    }
}

impl Module for Module1 {
    #[instrument(skip(self))]
    fn package(&self) -> &str {
        "dll.module1"
    }

    #[instrument(skip(self))]
    fn version(&self) -> &str {
        "1.0.0"
    }

    #[instrument(skip(self))]
    fn run(&self) {
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
                info!("dll.module1::on_success: {:?}", result.data)
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
                error!("module1::on_err: {:?}", err.code)
            }
        }

        self.registry
            .invoke("dll.module2", "1", None, Box::new(TestCallback {}))
    }

    #[instrument(skip(self, callback))]
    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        info!(
            "dll.module1::invoke: method = {}, data = {:?}",
            method, data
        );
        callback.on_success(CallbackSuccess {
            data: Some(b"dll.module1::invoke"),
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

    NativeModule::new(Module1::new(registry))
}
