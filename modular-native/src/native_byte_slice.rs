#[derive(Copy, Clone)]
#[repr(C)]
pub struct NativeByteSlice {
    pub data: *const u8,
    pub len: usize,
}

impl Default for NativeByteSlice {
    fn default() -> Self {
        Self {
            data: std::ptr::null(),
            len: 0,
        }
    }
}

impl<T: AsRef<[u8]>> From<T> for NativeByteSlice {
    fn from(v: T) -> Self {
        Self {
            data: v.as_ref().as_ptr(),
            len: v.as_ref().len(),
        }
    }
}

impl From<NativeByteSlice> for Option<&[u8]> {
    fn from(v: NativeByteSlice) -> Self {
        if v.data.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(v.data, v.len) })
        }
    }
}
