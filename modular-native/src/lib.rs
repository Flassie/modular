#![allow(clippy::borrowed_box)]

mod types;

use std::ptr::null_mut;
use tracing::error;
pub use types::*;

pub trait Callback: Send + Sync {
    fn on_success(&mut self, result: Option<&[u8]>);
    fn on_error(
        &mut self,
        code: i32,
        err_name: &str,
        description: Option<&str>,
        data: Option<&[u8]>,
    );
}

impl<D: Send + Sync> Callback for NativeCallbackVTable<D> {
    fn on_success(&mut self, result: Option<&[u8]>) {
        (self.on_success)(
            self.user_data,
            result.map(NativeByteSlice::from).unwrap_or_default(),
        );
    }

    fn on_error(
        &mut self,
        code: i32,
        err_name: &str,
        description: Option<&str>,
        data: Option<&[u8]>,
    ) {
        (self.on_error)(
            self.user_data,
            code,
            NativeString::from(err_name),
            description.map(NativeString::from).unwrap_or_default(),
            data.map(NativeByteSlice::from).unwrap_or_default(),
        );
    }
}

trait NativeModule: Send + Sync {
    fn package(&self) -> &str;
    fn version(&self) -> &str;
    fn invoke(&self, method: &str, args: Option<&[u8]>, callback: Box<dyn Callback>);
    fn run(&self);
}

trait NativeRegistry: Send + Sync {
    fn get_chain_items(&self) -> Vec<&str>;
    fn run(&self);
    fn invoke<C: Callback + 'static>(
        &self,
        package: &str,
        method: &str,
        data: Option<&[u8]>,
        callback: C,
    );
    fn register_module<M>(&self, module: M)
    where
        M: NativeModule + 'static;
    fn deregister_module(&self, package: &str);
}

#[repr(transparent)]
struct NativeCallback(*mut Box<dyn Callback>);

unsafe impl Send for NativeCallback {}
unsafe impl Sync for NativeCallback {}

impl NativeCallback {
    pub fn new<C: Callback + 'static>(callback: C) -> Self {
        let callback: Box<dyn Callback> = Box::new(callback);
        let callback = Box::into_raw(Box::new(callback));
        Self(callback)
    }
}

macro_rules! get_callback {
    ($user_data:ident) => {
        unsafe {
            if $user_data.is_null() {
                return;
            }

            // Safety: we've checked user_data is not null
            let user_data_ref = &*$user_data;

            if user_data_ref.0.is_null() {
                return;
            }

            // Safety: we've checked user_data_ref.0 is not null
            Box::from_raw(user_data_ref.0)
        }
    };
}

macro_rules! clear_callback {
    ($user_data:ident, $callback:ident) => {
        drop($callback);

        unsafe {
            (*$user_data).0 = null_mut();
        }
    };
}

macro_rules! get_str {
    ($v:ident) => {
        match Option::<&str>::from($v) {
            Some(s) => s,
            None => {
                error!("error decoding string {:?}", stringify!($v));
                return -1;
            }
        }
    };
}

impl NativeRegistry
    for NativeRegistryVTable<Box<dyn NativeModule>, NativeCallback, (), NativeCallback>
{
    fn get_chain_items(&self) -> Vec<&str> {
        unsafe {
            let count = (self.get_chain_items_count)(&*self.instance);
            let mut chain_items = Vec::with_capacity(count);

            for i in 0..count {
                let mut ptr = std::ptr::null();
                let chain_item_len = (self.get_chain_item)(&*self.instance, i, &mut ptr);
                let chain_item = std::slice::from_raw_parts(ptr, chain_item_len);

                if ptr.is_null() {
                    continue;
                }

                match std::str::from_utf8(chain_item) {
                    Ok(s) => chain_items.push(s),
                    Err(err) => {
                        error!("error decoding chain item: {}", err);
                    }
                }
            }

            chain_items
        }
    }

    fn run(&self) {
        unsafe { (self.run)(&*self.instance) };
    }

    fn invoke<C: Callback + 'static>(
        &self,
        package: &str,
        method: &str,
        data: Option<&[u8]>,
        callback: C,
    ) {
        let callback = NativeCallback::new(callback);

        extern "C" fn callback_on_success(user_data: *mut NativeCallback, data: NativeByteSlice) {
            let mut callback = get_callback!(user_data);
            callback.on_success(data.into());
            clear_callback!(user_data, callback);
        }

        extern "C" fn callback_on_error(
            user_data: *mut NativeCallback,
            code: i32,
            name: NativeString,
            description: NativeString,
            data: NativeByteSlice,
        ) {
            let mut callback = get_callback!(user_data);
            callback.on_error(
                code,
                Option::<&str>::from(name).unwrap_or("NO_ERR_NAME"),
                description.into(),
                data.into(),
            );

            clear_callback!(user_data, callback);
        }

        let invocation_data = NativeRegistryInvocationData {
            module: package.into(),
            method: method.into(),
            data: data.map(|i| i.into()).unwrap_or_default(),
            callback_vtable: NativeCallbackVTable {
                user_data: Box::into_raw(Box::new(callback)),
                on_success: callback_on_success,
                on_error: callback_on_error,
            },
        };

        unsafe { (self.invoke)(&*self.instance, invocation_data) };
    }

    fn register_module<M>(&self, module: M)
    where
        M: NativeModule + 'static,
    {
        let boxed_module: Box<dyn NativeModule> = Box::new(module);
        let native_module_vtable = NativeModuleVTable {
            instance: NativeMutPtr(Box::into_raw(Box::new(boxed_module))),
            package_fn,
            version_fn,
            invoke_fn,
            run_fn: Some(run_fn),
            destroy_fn,
        };

        unsafe { (self.register_module)(&*self.instance, native_module_vtable) };
    }

    fn deregister_module(&self, package: &str) {
        unsafe { (self.deregister_module)(&*self.instance, package.into()) };
    }
}

macro_rules! instance {
    ($instance:ident) => {
        let $instance = $instance.expect("instance is null");
    };
}

extern "C" fn package_fn(instance: Option<&Box<dyn NativeModule>>, size: &mut usize) -> *const u8 {
    instance!(instance);

    *size = instance.package().len();
    instance.package().as_ptr()
}

extern "C" fn version_fn(instance: Option<&Box<dyn NativeModule>>, size: &mut usize) -> *const u8 {
    instance!(instance);

    *size = instance.version().len();
    instance.version().as_ptr()
}

extern "C" fn invoke_fn(
    instance: Option<&Box<dyn NativeModule>>,
    method: NativeString,
    data: NativeByteSlice,
    ctx: NativeModuleInvocationContext<NativeCallback, ()>,
) -> i32 {
    instance!(instance);

    let method = get_str!(method);
    let data: Option<&[u8]> = data.into();

    let callback: Box<dyn Callback> = Box::new(ctx.callback);
    instance.invoke(method, data, callback);

    0
}

extern "C" fn run_fn(instance: Option<&Box<dyn NativeModule>>, _: &()) -> i32 {
    if let Some(instance) = instance {
        instance.run();
    }

    0
}

extern "C" fn destroy_fn(instance: *mut Box<dyn NativeModule>, _: &()) {
    unsafe {
        if instance.is_null() {
            return;
        }

        Box::from_raw(instance);
    }
}

#[test]
fn a() {}
