#![allow(dead_code)]

use crate::{
    InvocationContext, Module, Registry, RegistryError, RegistryInvocationCallback, RunError,
};
use modular_native::*;
use tokio::runtime::Builder;
use tracing::*;

#[repr(transparent)]
struct RegistryPtr(*mut Registry);

unsafe impl Send for RegistryPtr {}
unsafe impl Sync for RegistryPtr {}

// vtable impls

impl<T> Module for NativeModuleVTable<T, (RegistryInvocationCallback, RegistryPtr), Registry>
where
    T: Send + Sync,
{
    fn package(&self) -> &str {
        let mut len = 0;
        let f = (self.package_fn)(self.instance.as_ref(), &mut len);
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(f, len)) }
    }

    fn version(&self) -> &str {
        let mut len = 0;
        let f = (self.version_fn)(self.instance.as_ref(), &mut len);
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(f, len)) }
    }

    fn invoke(&self, ctx: InvocationContext, registry: &Registry) {
        extern "C" fn native_on_success(
            user_data_ptr: *mut (RegistryInvocationCallback, RegistryPtr),
            data: NativeByteSlice,
        ) {
            let user_data = if user_data_ptr.is_null() {
                None
            } else {
                Some(unsafe { Box::from_raw(user_data_ptr) })
            };

            if let Some(user_data) = user_data {
                if !(*user_data).1 .0.is_null() {
                    unsafe { Box::from_raw((*user_data).1 .0) };
                }

                (*user_data).0.on_success::<&[u8]>(data.into());
            }
        }

        extern "C" fn native_on_err(
            user_data: *mut (RegistryInvocationCallback, RegistryPtr),
            code: i32,
            name: NativeString,
            description: NativeString,
            data: NativeByteSlice,
        ) {
            let user_data = if user_data.is_null() {
                None
            } else {
                Some(unsafe { Box::from_raw(user_data) })
            };

            if let Some(user_data) = user_data {
                if !(*user_data).1 .0.is_null() {
                    unsafe { Box::from_raw((*user_data).1 .0) };
                }

                (*user_data).0.on_error::<&str, &str, &[u8]>(
                    code,
                    Option::<&str>::unwrap_or_default(name.into()),
                    description.into(),
                    data.into(),
                );
            }
        }

        let registry_ptr = RegistryPtr(Box::into_raw(Box::new(registry.clone())));
        let callback = NativeCallbackVTable {
            user_data: Box::into_raw(Box::new((ctx.callback, registry_ptr))),
            on_success: native_on_success,
            on_error: native_on_err,
        };

        (self.invoke_fn)(
            self.instance.as_ref(),
            ctx.method.into(),
            ctx.data.map(|i| i.into()).unwrap_or_default(),
            NativeModuleInvocationContext {
                registry: Box::into_raw(Box::new(registry.clone())),
                callback,
            },
        );
    }

    fn run(&self, registry: &Registry) -> Result<(), RunError> {
        match self.run_fn {
            Some(f) => match (f)(self.instance.as_ref(), registry) {
                0 => Ok(()),
                v => Err(RunError {
                    code: v,
                    message: "native run failed".to_string(),
                }),
            },
            None => Ok(()),
        }
    }

    fn destroy(&self, registry: &Registry) {
        (self.destroy_fn)(self.instance.0, registry);
    }
}

extern "C" fn registry_release(registry: *mut Registry) {
    if registry.is_null() {
        return;
    }

    unsafe {
        Box::from_raw(registry);
    }
}

extern "C" fn registry_get_chain_items_count(registry: &Registry) -> usize {
    registry.call_chain().len()
}

extern "C" fn registry_get_chain_item(
    registry: &Registry,
    index: usize,
    ptr: *mut *const u8,
) -> usize {
    let chain = registry.call_chain();
    match chain.get(index) {
        Some(v) => {
            unsafe {
                *ptr = v.as_ptr();
            }
            v.len()
        }
        None => 0,
    }
}

extern "C" fn registry_run(registry: &Registry) -> i32 {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(v) => v,
        Err(err) => {
            error!("failed to create async runtime: {}", err);
            return -1;
        }
    };

    let registry = registry.clone();
    runtime.block_on(async move {
        registry.run().await;
        0
    })
}

extern "C" fn registry_invoke(
    registry: &Registry,
    invocation_data: NativeRegistryInvocationData<()>,
) -> i32 {
    if invocation_data.module.ptr.is_null() || invocation_data.method.ptr.is_null() {
        return -1;
    }

    let module = unsafe {
        std::slice::from_raw_parts(invocation_data.module.ptr, invocation_data.module.len)
    };
    let method = unsafe {
        std::slice::from_raw_parts(invocation_data.method.ptr, invocation_data.method.len)
    };
    let data = if invocation_data.data.ptr.is_null() {
        None
    } else {
        Some(unsafe {
            std::slice::from_raw_parts(invocation_data.data.ptr, invocation_data.data.len)
        })
    };

    let module = match std::str::from_utf8(module) {
        Ok(v) => v,
        Err(err) => {
            error!("failed to convert module name to utf8: {}", err);
            return -1;
        }
    };

    let method = match std::str::from_utf8(method) {
        Ok(v) => v,
        Err(err) => {
            error!("failed to convert method name to utf8: {}", err);
            return -2;
        }
    };

    match registry.invoke(module, method, data, invocation_data.callback_vtable) {
        Ok(()) => 0,
        Err(err) => match err {
            RegistryError::UnknownModule => 1,
        },
    }
}

extern "C" fn registry_register_module(
    registry: &Registry,
    module: NativeModuleVTable<(), (RegistryInvocationCallback, RegistryPtr), Registry>,
) -> i32 {
    registry.register_module(module);
    0
}

// pub static CALLBACK_VTABLE: &'static CallbackVTable = &CallbackVTable {
//     on_success: invocation_callback_on_success,
//     on_error: invocation_callback_on_error,
// };
//
// pub static VTABLE: &'static RegistryVTable = &RegistryVTable {
//     create: registry_create,
//     release: registry_release,
//     get_chain_items_count: registry_get_chain_items_count,
//     get_chain_item: registry_get_chain_item,
//     run: registry_run,
//     invoke: registry_invoke,
//     register_module: registry_register_module,
//     deregister_module: registry_deregister_module,
//     callback_vtable: &CALLBACK_VTABLE,
// };
//
// #[repr(C)]
// pub struct RegistryVTable {
//     pub create: extern "C" fn() -> *mut Registry,
//     pub release: extern "C" fn(registry: *mut Registry),
//     pub get_chain_items_count: extern "C" fn(registry: &Registry) -> usize,
//     pub get_chain_item: extern "C" fn(&Registry, idx: usize, ptr: *mut *const u8) -> usize,
//     pub run: extern "C" fn(&Registry) -> i32,
//     pub invoke: extern "C" fn(
//         &Registry,
//         module: *const u8,
//         module_len: usize,
//         method: *const u8,
//         method_len: usize,
//         data: *const u8,
//         data_len: usize,
//         callback: NativeCallback,
//     ) -> i32,
//     pub register_module: extern "C" fn(&Registry, module: NativeModuleVTable),
//     pub deregister_module:
//         extern "C" fn(registry: &Registry, package: *const u8, package_len: usize),
//     pub callback_vtable: &'static CallbackVTable,
// }
//
// #[repr(C)]
// pub struct CallbackVTable {
//     pub on_success: extern "C" fn(
//         callback_ptr: *mut *mut RegistryInvocationCallback,
//         data: *const u8,
//         data_len: usize,
//     ),
//     pub on_error: extern "C" fn(
//         callback_ptr: *mut *mut RegistryInvocationCallback,
//         code: i32,
//         name: *const u8,
//         name_len: usize,
//         descr: *const u8,
//         descr_len: usize,
//         data: *const u8,
//         data_len: usize,
//     ),
// }
//
// #[repr(C)]
// pub struct NativeModuleVTable {
//     instance: *mut (),
//     package_fn: extern "C" fn(instance: *const (), len: &mut usize) -> *const u8,
//     version_fn: extern "C" fn(instance: *const (), len: &mut usize) -> *const u8,
//     invoke_fn: extern "C" fn(
//         instance: *const (),
//         data: *const u8,
//         data_len: usize,
//         method: *const u8,
//         method_len: usize,
//         callback: *mut RegistryInvocationCallback,
//         registry: *mut Registry,
//     ),
//     run_fn: Option<extern "C" fn(instance: *const (), registry: *mut Registry) -> i32>,
//     destroy_fn: Option<extern "C" fn(instance: *const (), registry: *mut Registry)>,
// }
//
// unsafe impl Send for NativeModuleVTable {}
// unsafe impl Sync for NativeModuleVTable {}
//
// impl Module for NativeModuleVTable {
//     fn package(&self) -> &str {
//         let mut len = 0;
//         let bytes = (self.package_fn)(self.instance, &mut len);
//
//         unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(bytes, len)) }
//     }
//
//     fn version(&self) -> &str {
//         let mut len = 0;
//         let bytes = (self.version_fn)(self.instance, &mut len);
//
//         unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(bytes, len)) }
//     }
//
//     fn invoke(&self, ctx: InvocationContext, registry: &Registry) {
//         (self.invoke_fn)(
//             self.instance,
//             ctx.data.map(|i| i.as_ptr()).unwrap_or(null()),
//             ctx.data.map(|i| i.len()).unwrap_or(0),
//             ctx.method.as_ptr(),
//             ctx.method.len(),
//             Box::into_raw(Box::new(ctx.callback)),
//             Box::into_raw(Box::new(registry.clone())),
//         );
//     }
//
//     fn run(&self, registry: &Registry) -> Result<(), RunError> {
//         if let Some(run_fn) = self.run_fn {
//             let mut registry = registry.clone();
//             let result = run_fn(self.instance, &mut registry);
//
//             if result == 0 {
//                 Ok(())
//             } else {
//                 Err(RunError::new(result, format!("run_fn returned {}", result)))
//             }
//         } else {
//             Ok(())
//         }
//     }
//
//     fn destroy(&self, registry: &Registry) {
//         if let Some(destroy_fn) = self.destroy_fn {
//             let mut registry = registry.clone();
//             destroy_fn(self.instance, &mut registry);
//         }
//     }
// }
//
// #[instrument]
// extern "C" fn registry_create() -> *mut Registry {
//     Box::into_raw(Box::new(Registry::new()))
// }
//
// #[instrument(skip(registry), fields(module = registry.root_caller()))]
// extern "C" fn registry_get_chain_items_count(registry: &Registry) -> usize {
//     registry.call_chain().len()
// }
//
// #[instrument(skip(registry, ptr), fields(module = registry.root_caller()))]
// extern "C" fn registry_get_chain_item(
//     registry: &Registry,
//     idx: usize,
//     ptr: *mut *const u8,
// ) -> usize {
//     match registry.call_chain().get(idx) {
//         Some(v) => {
//             if !ptr.is_null() {
//                 unsafe {
//                     *ptr = v.as_ptr();
//                 }
//             }
//
//             v.len()
//         }
//         None => {
//             error!("index out of bounds");
//             0
//         }
//     }
// }
//
// #[instrument(skip(registry), fields(module = registry.root_caller()), ret)]
// extern "C" fn registry_run(registry: &Registry) -> i32 {
//     let runtime = Builder::new_current_thread()
//         .enable_all()
//         .thread_name("registry")
//         .build();
//
//     match runtime {
//         Ok(v) => {
//             v.block_on(async move {
//                 registry.run().await;
//             });
//
//             0
//         }
//         Err(err) => {
//             error!("failed to create runtime: {}", err);
//             -1
//         }
//     }
// }
//
// #[instrument(skip(registry, native_module), fields(module = registry.root_caller()))]
// extern "C" fn registry_register_module(registry: &Registry, native_module: NativeModuleVTable) {
//     registry.register_module(native_module);
// }
//
// #[instrument(skip(registry), fields(module = registry.root_caller()))]
// extern "C" fn registry_deregister_module(registry: &Registry, package: *const u8, len: usize) {
//     if package.is_null() {
//         error!("package is null");
//         return;
//     }
//
//     let package =
//         unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(package, len)) };
//
//     registry.deregister_module(package);
// }
//
// #[repr(C)]
// pub struct NativeCallback {
//     user_data: *mut (),
//     on_success: extern "C" fn(user_data: *mut (), data: *const u8, data_len: usize),
//     on_error: extern "C" fn(
//         user_data: *mut (),
//         code: i32,
//         message: *const u8,
//         message_len: usize,
//         descr: *const u8,
//         descr_len: usize,
//         data: *const u8,
//         data_len: usize,
//     ),
// }
//
// unsafe impl Send for NativeCallback {}
// unsafe impl Sync for NativeCallback {}
//
// impl Callback for NativeCallback {
//     fn on_success(&self, result: Option<&[u8]>) {
//         (self.on_success)(
//             self.user_data,
//             result.map(|i| i.as_ptr()).unwrap_or(null()),
//             result.map(|i| i.len()).unwrap_or(0),
//         );
//     }
//
//     fn on_error(&self, code: i32, err_name: &str, description: Option<&str>, data: Option<&[u8]>) {
//         (self.on_error)(
//             self.user_data,
//             code,
//             err_name.as_ptr(),
//             err_name.len(),
//             description.map(|i| i.as_ptr()).unwrap_or(null()),
//             description.map(|i| i.len()).unwrap_or(0),
//             data.map(|i| i.as_ptr()).unwrap_or(null()),
//             data.map(|i| i.len()).unwrap_or(0),
//         );
//     }
// }
//
// extern "C" fn registry_invoke(
//     registry: &Registry,
//     package: *const u8,
//     package_len: usize,
//     method: *const u8,
//     method_len: usize,
//     data: *const u8,
//     data_len: usize,
//     callback: NativeCallback,
// ) -> i32 {
//     if package.is_null() || method.is_null() {
//         return -1;
//     }
//
//     let package = match as_str(package, package_len) {
//         Some(v) => v,
//         None => {
//             error!("invalid `package`");
//             return -2;
//         }
//     };
//
//     let method = match as_str(method, method_len) {
//         Some(v) => v,
//         None => {
//             error!("invalid `method`");
//             return -3;
//         }
//     };
//
//     let data = if data.is_null() {
//         None
//     } else {
//         Some(unsafe { std::slice::from_raw_parts(data, data_len) })
//     };
//
//     match registry.invoke(package, method, data, callback) {
//         Ok(_) => 0,
//         Err(err) => match err {
//             RegistryError::UnknownModule => 1,
//         },
//     }
// }
//
// extern "C" fn registry_release(registry: *mut Registry) {
//     if !registry.is_null() {
//         unsafe { Box::from_raw(registry) };
//     }
// }
//
// extern "C" fn invocation_callback_on_success(
//     callback_ptr: *mut *mut RegistryInvocationCallback,
//     data: *const u8,
//     data_len: usize,
// ) {
//     if callback_ptr.is_null() {
//         error!("trying to call on_success on null callback");
//         return;
//     }
//
//     let callback = unsafe { Box::from_raw(*callback_ptr) };
//     let data = if data.is_null() {
//         None
//     } else {
//         Some(unsafe { std::slice::from_raw_parts(data, data_len) })
//     };
//
//     callback.on_success(data);
//
//     unsafe {
//         *callback_ptr = null_mut();
//     }
// }
//
// extern "C" fn invocation_callback_on_error(
//     callback_ptr: *mut *mut RegistryInvocationCallback,
//     code: i32,
//     name: *const u8,
//     name_len: usize,
//     descr: *const u8,
//     descr_len: usize,
//     data: *const u8,
//     data_len: usize,
// ) {
//     if callback_ptr.is_null() {
//         error!("trying to call on_error on null callback");
//         return;
//     }
//
//     let callback = unsafe { Box::from_raw(*callback_ptr) };
//     let name = as_str(name, name_len);
//
//     if name.is_none() {
//         error!("trying to call on_error with null name");
//         return;
//     }
//
//     let descr = as_str(descr, descr_len);
//     let data = if data.is_null() {
//         None
//     } else {
//         Some(unsafe { std::slice::from_raw_parts(data, data_len) })
//     };
//
//     callback.on_error(code, name.unwrap(), descr, data);
//
//     unsafe {
//         *callback_ptr = null_mut();
//     }
// }
//
// fn as_str<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
//     if ptr.is_null() {
//         None
//     } else {
//         Some(unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) })
//     }
// }
