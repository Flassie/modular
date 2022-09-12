mod fn_mappings;

pub use fn_mappings::*;
pub use modular_core::*;
use std::mem::ManuallyDrop;

#[no_mangle]
extern "C" fn __wm_alloc(len: usize) -> *mut u8 {
    let v = vec![0; len];
    let mut v = ManuallyDrop::new(v);
    v.shrink_to_fit();
    v.as_mut_ptr()
}

#[no_mangle]
extern "C" fn __wm_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
    }
}
