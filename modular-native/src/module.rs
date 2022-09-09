use crate::errors::Error;
use crate::*;

pub trait Module: Send + Sync {
    fn package(&self) -> &str;
    fn version(&self) -> &str;

    fn run(&self);
    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>);
}

impl Module for Box<dyn Module> {
    fn package(&self) -> &str {
        self.as_ref().package()
    }

    fn version(&self) -> &str {
        self.as_ref().version()
    }

    fn run(&self) {
        self.as_ref().run()
    }

    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        self.as_ref().invoke(method, data, callback)
    }
}

#[repr(C)]
pub struct NativeModule {
    instance: *mut (),
    package_fn: extern "C" fn(instance: *mut ()) -> NativeByteSlice,
    version_fn: extern "C" fn(instance: *mut ()) -> NativeByteSlice,
    invoke_fn: extern "C" fn(
        instance: *mut (),
        method: NativeByteSlice,
        data: NativeByteSlice,
        callback: NativeCallback,
    ),
    run_fn: Option<extern "C" fn(instance: *mut ())>,
    drop_fn: extern "C" fn(instance: *mut ()),
}

unsafe impl Send for NativeModule {}
unsafe impl Sync for NativeModule {}

impl NativeModule {
    pub fn new<T: Module + 'static>(module: T) -> Self {
        Self {
            instance: Box::into_raw(Box::new(module)).cast(),
            package_fn: Self::package_fn::<T>,
            version_fn: Self::version_fn::<T>,
            invoke_fn: Self::invoke_fn::<T>,
            run_fn: Some(Self::run_fn::<T>),
            drop_fn: Self::drop_fn::<T>,
        }
    }

    extern "C" fn package_fn<T: Module>(instance: *mut ()) -> NativeByteSlice {
        let module = unsafe { &*(instance as *const T) };
        module.package().into()
    }

    extern "C" fn version_fn<T: Module>(instance: *mut ()) -> NativeByteSlice {
        let module = unsafe { &*(instance as *const T) };
        module.version().into()
    }

    extern "C" fn invoke_fn<T: Module>(
        instance: *mut (),
        method: NativeByteSlice,
        data: NativeByteSlice,
        callback: NativeCallback,
    ) {
        let module = unsafe { &*(instance as *const T) };
        let method = Option::<&[u8]>::from(method).and_then(|s| std::str::from_utf8(s).ok());

        let method = match method {
            Some(v) => v,
            None => {
                Callback::on_error(
                    &callback,
                    CallbackError {
                        code: Error::FfiInvalidMethodName as i32,
                        err_name: Error::FfiInvalidMethodName.as_ref().into(),
                        description: "empty or non-valid (not utf8) method name".into(),
                        data: None,
                    },
                );
                return;
            }
        };

        let data = Option::<&[u8]>::from(data);

        module.invoke(method, data, Box::new(callback));
    }

    extern "C" fn run_fn<T: Module>(instance: *mut ()) {
        let module = unsafe { &*(instance as *const T) };
        module.run();
    }

    extern "C" fn drop_fn<T: Module>(instance: *mut ()) {
        let _ = unsafe { Box::from_raw(instance as *mut T) };
    }
}

impl Module for NativeModule {
    fn package(&self) -> &str {
        get_str!((self.package_fn)(self.instance), package)
    }

    fn version(&self) -> &str {
        get_str!((self.version_fn)(self.instance), name)
    }

    fn run(&self) {
        if let Some(run) = self.run_fn {
            run(self.instance);
        }
    }

    fn invoke(&self, method: &str, data: Option<&[u8]>, callback: Box<dyn Callback>) {
        let method = method.into();
        let data = data.map(NativeByteSlice::from);

        (self.invoke_fn)(
            self.instance,
            method,
            data.unwrap_or_default(),
            NativeCallback::new(callback),
        );
    }
}

impl Drop for NativeModule {
    fn drop(&mut self) {
        (self.drop_fn)(self.instance)
    }
}
