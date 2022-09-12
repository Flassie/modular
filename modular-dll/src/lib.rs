use modular_core::{Callback, Module, NativeModule, NativeRegistry, Registry};
use std::ffi::OsStr;

pub use modular_core::*;

pub struct DllModule {
    _lib: libloading::Library,
    module: NativeModule,
}

impl DllModule {
    pub fn new<S: AsRef<OsStr>, R: Registry + 'static>(
        path: S,
        registry: &R,
    ) -> Result<Self, libloading::Error> {
        unsafe {
            let lib = libloading::Library::new(path)?;

            let create_module =
                lib.get::<unsafe extern "C" fn(NativeRegistry) -> NativeModule>(b"create_module")?;

            let module = create_module(NativeRegistry::new(registry.clone()));

            Ok(Self { _lib: lib, module })
        }
    }
}

impl Module for DllModule {
    fn package(&self) -> &str {
        self.module.package()
    }

    fn version(&self) -> &str {
        self.module.version()
    }

    fn run(&self) {
        self.module.run()
    }

    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        self.module.invoke(method, data, callback)
    }
}
