use modular_core::*;
use uuid::*;
use wasmer::*;

pub struct OptionalCallback(Option<Box<dyn Callback>>);
pub struct OptionalCallbackRef<'a>(Option<&'a dyn Callback>);

impl OptionalCallback {
    pub fn new(callback: Option<Box<dyn Callback>>) -> Self {
        Self(callback)
    }
}

impl<'a> OptionalCallbackRef<'a> {
    pub fn new(callback: Option<&'a dyn Callback>) -> Self {
        Self(callback)
    }
}

impl Callback for OptionalCallback {
    fn on_success(&self, result: CallbackSuccess) {
        if let Some(callback) = &self.0 {
            callback.on_success(result);
        }
    }

    fn on_error(&self, err: CallbackError) {
        if let Some(callback) = &self.0 {
            callback.on_error(err);
        }
    }
}

impl Callback for OptionalCallbackRef<'_> {
    fn on_success(&self, result: CallbackSuccess) {
        if let Some(callback) = &self.0 {
            callback.on_success(result);
        }
    }

    fn on_error(&self, err: CallbackError) {
        if let Some(callback) = &self.0 {
            callback.on_error(err);
        }
    }
}

#[inline]
pub fn get_uid(memory: &Memory, ptr: i32, store: &impl AsStoreRef) -> Uuid {
    let view = memory.view(store);

    let uid = WasmPtr::<u128>::new(ptr as _).read(&view).unwrap();
    Uuid::from_u128(uid)
}

#[inline]
pub fn read_bytes(memory: &Memory, ptr: i32, len: i32, store: &impl AsStoreRef) -> Option<Vec<u8>> {
    let view = memory.view(store);

    let slice = WasmSlice::<u8>::new(&view, ptr as _, len as _).ok()?;
    slice.read_to_vec().ok()
}

#[inline]
pub fn read_string(memory: &Memory, ptr: i32, len: u32, store: &impl AsStoreRef) -> Option<String> {
    let bytes = read_bytes(memory, ptr, len as _, store)?;
    String::from_utf8(bytes).ok()
}
