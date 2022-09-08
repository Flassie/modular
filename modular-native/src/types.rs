#![allow(dead_code)]

#[repr(transparent)]
pub struct NativeMutPtr<T>(pub *mut T);

impl<T> NativeMutPtr<T> {
    pub fn as_ref(&self) -> Option<&T> {
        if self.0.is_null() {
            None
        } else {
            Some(unsafe { &*self.0 })
        }
    }
}

#[repr(C)]
pub struct NativeByteSlice {
    pub ptr: *const u8,
    pub len: usize,
}

impl Default for NativeByteSlice {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

pub type NativeString = NativeByteSlice;

impl<T: AsRef<[u8]>> From<T> for NativeByteSlice {
    fn from(v: T) -> Self {
        Self {
            ptr: v.as_ref().as_ptr(),
            len: v.as_ref().len(),
        }
    }
}

impl From<NativeByteSlice> for Option<&[u8]> {
    fn from(v: NativeByteSlice) -> Self {
        if v.ptr.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(v.ptr, v.len) })
        }
    }
}

impl From<NativeByteSlice> for Option<&str> {
    fn from(v: NativeByteSlice) -> Self {
        Option::<&[u8]>::from(v).and_then(|i| std::str::from_utf8(i).ok())
    }
}

#[repr(C)]
pub struct NativeRegistryInvocationData<D>
where
    D: Send + Sync,
{
    pub module: NativeString,
    pub method: NativeString,
    pub data: NativeByteSlice,
    pub callback_vtable: NativeCallbackVTable<D>,
}

pub type NativeRegistryReleaseFn<R> = unsafe extern "C" fn(registry: *mut R);
pub type NativeRegistryGetChainItemsCount<R> = unsafe extern "C" fn(registry: &R) -> usize;
pub type NativeRegistryGetChainItem<R> =
    unsafe extern "C" fn(registry: &R, index: usize, ptr: *mut *const u8) -> usize;
pub type NativeRegistryRun<R> = unsafe extern "C" fn(registry: &R) -> i32;
pub type NativeRegistryInvoke<R, D> =
    unsafe extern "C" fn(registry: &R, invocation_data: NativeRegistryInvocationData<D>) -> i32;
pub type NativeRegistryRegisterModule<T, C, R> =
    unsafe extern "C" fn(registry: &R, native_module: NativeModuleVTable<T, C, R>);
pub type NativeRegistryDeregisterModule<R> = unsafe extern "C" fn(registry: &R, name: NativeString);

#[repr(C)]
pub struct NativeRegistryVTable<T, C, R, D>
where
    T: Send + Sync,
    C: Send + Sync,
    R: Send + Sync,
    D: Send + Sync,
{
    pub instance: *mut R,
    pub release_fn: NativeRegistryReleaseFn<R>,
    pub get_chain_items_count: NativeRegistryGetChainItemsCount<R>,
    pub get_chain_item: NativeRegistryGetChainItem<R>,
    pub run: NativeRegistryRun<R>,
    pub invoke: NativeRegistryInvoke<R, D>,
    pub register_module: NativeRegistryRegisterModule<T, C, R>,
    pub deregister_module: NativeRegistryDeregisterModule<R>,
}

unsafe impl<T, C, R, D> Send for NativeRegistryVTable<T, C, R, D>
where
    T: Send + Sync,
    C: Send + Sync,
    R: Send + Sync,
    D: Send + Sync,
{
}

unsafe impl<T, C, R, D> Sync for NativeRegistryVTable<T, C, R, D>
where
    T: Send + Sync,
    C: Send + Sync,
    R: Send + Sync,
    D: Send + Sync,
{
}

#[repr(C)]
pub struct NativeModuleInvocationContext<D, T>
where
    D: Send + Sync,
    T: Send + Sync,
{
    pub registry: *mut T,
    pub callback: NativeCallbackVTable<D>,
}

pub type NativeModulePackageFn<I> =
    extern "C" fn(instance: Option<&I>, len: &mut usize) -> *const u8;
pub type NativeModuleVersionFn<I> =
    extern "C" fn(instance: Option<&I>, len: &mut usize) -> *const u8;
pub type NativeModuleInvokeFn<I, C, R> = extern "C" fn(
    instance: Option<&I>,
    method: NativeString,
    data: NativeByteSlice,
    ctx: NativeModuleInvocationContext<C, R>,
) -> i32;
pub type NativeModuleRunFn<I, R> = extern "C" fn(instance: Option<&I>, registry: &R) -> i32;
pub type NativeModuleDestroyFn<I, R> = extern "C" fn(instance: *mut I, registry: &R);

#[repr(C)]
pub struct NativeModuleVTable<T, C, R>
where
    T: Send + Sync,
    C: Send + Sync,
    R: Send + Sync,
{
    pub instance: NativeMutPtr<T>,
    pub package_fn: NativeModulePackageFn<T>,
    pub version_fn: NativeModuleVersionFn<T>,
    pub invoke_fn: NativeModuleInvokeFn<T, C, R>,
    pub run_fn: Option<NativeModuleRunFn<T, R>>,
    pub destroy_fn: NativeModuleDestroyFn<T, R>,
}

unsafe impl<T, C, R> Send for NativeModuleVTable<T, C, R>
where
    T: Send + Sync,
    C: Send + Sync,
    R: Send + Sync,
{
}

unsafe impl<T, C, R> Sync for NativeModuleVTable<T, C, R>
where
    T: Send + Sync,
    C: Send + Sync,
    R: Send + Sync,
{
}

pub type NativeCallbackOnSuccess<D> = extern "C" fn(user_data: *mut D, data: NativeByteSlice);
pub type NativeCallbackOnFailure<D> = extern "C" fn(
    user_data: *mut D,
    code: i32,
    name: NativeString,
    description: NativeString,
    data: NativeByteSlice,
);

#[repr(C)]
pub struct NativeCallbackVTable<D: Send + Sync> {
    pub user_data: *mut D,
    pub on_success: NativeCallbackOnSuccess<D>,
    pub on_error: NativeCallbackOnFailure<D>,
}

unsafe impl<D: Send + Sync> Send for NativeCallbackVTable<D> {}
unsafe impl<D: Send + Sync> Sync for NativeCallbackVTable<D> {}
