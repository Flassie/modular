use crate::*;

pub trait Callback: Send + Sync {
    fn on_success(&self, result: CallbackSuccess);
    fn on_error(&self, err: CallbackError);
}

impl Callback for Box<dyn Callback> {
    fn on_success(&self, result: CallbackSuccess) {
        (**self).on_success(result);
    }

    fn on_error(&self, err: CallbackError) {
        (**self).on_error(err);
    }
}

#[repr(C)]
pub struct NativeCallback {
    instance: *mut (),
    on_success: extern "C" fn(*mut (), NativeCallbackSuccess),
    on_error: extern "C" fn(*mut (), NativeCallbackError),
    drop: extern "C" fn(*mut ()),
}

unsafe impl Send for NativeCallback {}
unsafe impl Sync for NativeCallback {}

impl NativeCallback {
    pub fn new<T: Callback + 'static>(callback: T) -> Self {
        let instance = Box::into_raw(Box::new(callback)) as *mut ();

        Self {
            instance,
            on_success: Self::on_success::<T>,
            on_error: Self::on_error::<T>,
            drop: Self::drop::<T>,
        }
    }

    extern "C" fn on_success<T: Callback>(instance: *mut (), result: NativeCallbackSuccess) {
        let callback = unsafe { &*(instance as *const T) };
        callback.on_success(result.into());
    }

    extern "C" fn on_error<T: Callback>(instance: *mut (), err: NativeCallbackError) {
        let callback = unsafe { &*(instance as *const T) };
        callback.on_error(err.into());
    }

    extern "C" fn drop<T: Callback>(instance: *mut ()) {
        let callback = unsafe { Box::from_raw(instance as *mut T) };
        drop(callback);
    }
}

impl Callback for NativeCallback {
    fn on_success(&self, result: CallbackSuccess) {
        (self.on_success)(self.instance, result.into());
    }

    fn on_error(&self, err: CallbackError) {
        (self.on_error)(self.instance, err.into());
    }
}

impl Drop for NativeCallback {
    fn drop(&mut self) {
        (self.drop)(self.instance);
    }
}

pub struct CallbackSuccess<'a> {
    pub data: Option<&'a [u8]>,
}

#[repr(C)]
pub struct NativeCallbackSuccess {
    pub data: NativeByteSlice,
}

impl From<CallbackSuccess<'_>> for NativeCallbackSuccess {
    fn from(success: CallbackSuccess) -> Self {
        Self {
            data: success.data.map(|i| i.into()).unwrap_or_default(),
        }
    }
}

impl From<NativeCallbackSuccess> for CallbackSuccess<'_> {
    fn from(v: NativeCallbackSuccess) -> Self {
        Self {
            data: v.data.into(),
        }
    }
}

pub struct CallbackError<'a> {
    pub code: i32,
    pub err_name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub data: Option<&'a [u8]>,
}

#[repr(C)]
pub struct NativeCallbackError {
    pub code: i32,
    pub err_name: NativeByteSlice,
    pub description: NativeByteSlice,
    pub data: NativeByteSlice,
}

impl From<CallbackError<'_>> for NativeCallbackError {
    fn from(err: CallbackError) -> Self {
        Self {
            code: err.code,
            err_name: err.err_name.map(|i| i.into()).unwrap_or_default(),
            description: err.description.map(|i| i.into()).unwrap_or_default(),
            data: err.data.map(|i| i.into()).unwrap_or_default(),
        }
    }
}

impl From<NativeCallbackError> for CallbackError<'_> {
    fn from(v: NativeCallbackError) -> Self {
        Self {
            code: v.code,
            err_name: Option::<&[u8]>::from(v.err_name)
                .map(|i| std::str::from_utf8(i).unwrap_or("UNKNOWN_ERROR_INVALID_UTF8")),
            description: Option::<&[u8]>::from(v.description)
                .map(|i| std::str::from_utf8(i).unwrap_or("empty description (invalid format)")),
            data: v.data.into(),
        }
    }
}
