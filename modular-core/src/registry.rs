use crate::errors::Error;
use crate::*;
use tracing::error;

pub trait Registry: Clone + Send + Sync {
    fn run(&self) -> Result<(), Error>;
    fn register_module(&self, module: Box<dyn Module>);
    fn deregister_module(&self, package: &str);
    fn invoke(&self, package: &str, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>);
}

#[repr(C)]
pub struct NativeRegistry {
    instance: *mut (),
    run: extern "C" fn(instance: *mut ()) -> Error,
    register_module: extern "C" fn(instance: *mut (), module: NativeModule),
    deregister_module: extern "C" fn(instance: *mut (), package: NativeByteSlice),
    invoke: extern "C" fn(
        instance: *mut (),
        package: NativeByteSlice,
        method: NativeByteSlice,
        data: NativeByteSlice,
        callback: NativeCallback,
    ),
    clone_fn: extern "C" fn(instance: *mut ()) -> Self,
    drop: extern "C" fn(instance: *mut ()),
}

unsafe impl Send for NativeRegistry {}
unsafe impl Sync for NativeRegistry {}

impl NativeRegistry {
    pub fn new<R: Registry + 'static>(registry: R) -> Self {
        let registry = Box::into_raw(Box::new(registry)) as *mut ();

        Self {
            instance: registry,
            run: Self::run::<R>,
            register_module: Self::register_module::<R>,
            deregister_module: Self::deregister_module::<R>,
            invoke: Self::invoke::<R>,
            clone_fn: Self::clone::<R>,
            drop: Self::drop::<R>,
        }
    }

    extern "C" fn run<R: Registry>(instance: *mut ()) -> Error {
        let registry = unsafe { &*(instance as *const R) };

        match registry.run() {
            Ok(()) => Error::default(),
            Err(e) => e,
        }
    }

    extern "C" fn register_module<R: Registry>(instance: *mut (), module: NativeModule) {
        let registry = unsafe { &*(instance as *const R) };
        registry.register_module(Box::new(module));
    }

    extern "C" fn deregister_module<R: Registry>(instance: *mut (), package: NativeByteSlice) {
        let registry = unsafe { &*(instance as *const R) };
        let package: Option<&[u8]> = package.into();

        let package = get_str!(package, package);
        registry.deregister_module(package);
    }

    extern "C" fn invoke<R: Registry>(
        instance: *mut (),
        package: NativeByteSlice,
        method: NativeByteSlice,
        data: NativeByteSlice,
        callback: NativeCallback,
    ) {
        let registry = unsafe { &*(instance as *const R) };
        let package = get_str!(package, package);
        let method = get_str!(method, method);
        let data: Option<&[u8]> = data.into();

        registry.invoke(package, method, data, Box::new(callback));
    }

    extern "C" fn drop<R: Registry + 'static>(instance: *mut ()) {
        let _ = unsafe { Box::from_raw(instance as *mut R) };
    }

    extern "C" fn clone<R: Registry + 'static>(instance: *mut ()) -> Self {
        let instance = unsafe { &*(instance as *const R) }.clone();
        Self::new(instance)
    }
}

impl Registry for NativeRegistry {
    fn run(&self) -> Result<(), Error> {
        let e = (self.run)(self.instance);

        if e == Error::NoError {
            Ok(())
        } else {
            Err(e)
        }
    }

    fn register_module(&self, module: Box<dyn Module>) {
        let module = NativeModule::new(module);
        (self.register_module)(self.instance, module)
    }

    fn deregister_module(&self, package: &str) {
        let package = NativeByteSlice::from(package);
        (self.deregister_module)(self.instance, package)
    }

    fn invoke(
        &self,
        package: &str,
        method: &str,
        data: Option<&[u8]>,
        callback: Box<dyn Callback>,
    ) {
        let package = NativeByteSlice::from(package);
        let method = NativeByteSlice::from(method);
        let data = data.map(NativeByteSlice::from).unwrap_or_default();
        let callback = NativeCallback::new(callback);
        (self.invoke)(self.instance, package, method, data, callback)
    }
}

impl Drop for NativeRegistry {
    fn drop(&mut self) {
        error!("dropping native registry");
        (self.drop)(self.instance)
    }
}

impl Clone for NativeRegistry {
    fn clone(&self) -> Self {
        (self.clone_fn)(self.instance)
    }
}
