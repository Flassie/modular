use protobuf_tracing::types::Record;
use protobuf_tracing::{Interest, Message};
use std::mem::ManuallyDrop;
use std::ptr::null;

pub use protobuf_tracing::register_module_tracer;
pub use protobuf_tracing::Recorder;

pub trait BytesRecorder: Send + Sync + Clone {
    fn is_interested(&self, interest: &Interest) -> bool;
    fn record(&self, record: Vec<u8>);
}

#[repr(C)]
pub struct NativeBytesRecorder {
    obj: *const (),
    is_interested: extern "C" fn(
        *const (),
        target: *const u8,
        target_len: usize,
        span_name: *const u8,
        span_len: usize,
    ) -> u8,
    alloc_protobuf_record: extern "C" fn(*const (), len: usize) -> *mut u8,
    on_protobuf_record: extern "C" fn(*const (), ptr: *mut u8, len: usize),
    clone: extern "C" fn(*const ()) -> NativeBytesRecorder,
    drop: extern "C" fn(*mut ()),
}

unsafe impl Send for NativeBytesRecorder {}
unsafe impl Sync for NativeBytesRecorder {}

impl NativeBytesRecorder {
    pub fn new<R: BytesRecorder + 'static>(recorder: R) -> Self {
        Self {
            obj: Box::into_raw(Box::new(recorder)).cast(),
            is_interested: Self::ffi_is_interested::<R>,
            alloc_protobuf_record: Self::ffi_alloc_protobuf_record::<R>,
            on_protobuf_record: Self::ffi_on_protobuf_record::<R>,
            clone: Self::ffi_clone::<R>,
            drop: Self::ffi_drop::<R>,
        }
    }

    extern "C" fn ffi_is_interested<R: BytesRecorder>(
        obj: *const (),
        target: *const u8,
        target_len: usize,
        span_name: *const u8,
        span_len: usize,
    ) -> u8 {
        let obj = obj as *const R;
        let recorder = unsafe { &*obj };

        let target = unsafe { std::slice::from_raw_parts(target, target_len) };
        let span_name = if !span_name.is_null() {
            Some(unsafe { std::slice::from_raw_parts(span_name, span_len) })
        } else {
            None
        };

        let interest = Interest {
            target: std::str::from_utf8(target).unwrap(),
            parent_span_name: span_name.map(|i| std::str::from_utf8(i).unwrap()),
        };

        recorder.is_interested(&interest) as u8
    }

    extern "C" fn ffi_alloc_protobuf_record<R: BytesRecorder>(_: *const (), len: usize) -> *mut u8 {
        let data = vec![0u8; len];
        let data = ManuallyDrop::new(data);

        data.as_ptr() as *mut u8
    }

    extern "C" fn ffi_on_protobuf_record<R: BytesRecorder>(
        obj: *const (),
        ptr: *mut u8,
        len: usize,
    ) {
        let obj = obj as *const R;
        let recorder = unsafe { &*obj };

        let data = unsafe { Vec::from_raw_parts(ptr, len, len) };
        recorder.record(data)
    }

    extern "C" fn ffi_clone<R: BytesRecorder + 'static>(obj: *const ()) -> NativeBytesRecorder {
        let obj = obj as *const R;
        let recorder = unsafe { &*obj }.clone();

        Self::new(recorder)
    }

    extern "C" fn ffi_drop<R: BytesRecorder>(obj: *mut ()) {
        if !obj.is_null() {
            let obj = obj as *mut R;
            unsafe { Box::from_raw(obj) };
        }
    }
}

impl Clone for NativeBytesRecorder {
    fn clone(&self) -> Self {
        (self.clone)(self.obj)
    }
}

impl Drop for NativeBytesRecorder {
    fn drop(&mut self) {
        if !self.obj.is_null() {
            (self.drop)(self.obj as *mut ());
        }
    }
}

impl Recorder for NativeBytesRecorder {
    fn is_interested(&self, interest: &Interest) -> bool {
        (self.is_interested)(
            self.obj as *const _ as *const (),
            interest.target.as_ptr(),
            interest.target.len(),
            interest
                .parent_span_name
                .map(|i| i.as_ptr())
                .unwrap_or(null()),
            interest
                .parent_span_name
                .map(|i| i.len())
                .unwrap_or_default(),
        ) != 0
    }

    fn record(&self, record: &Record) {
        let len = record.encoded_len();
        let ptr = (self.alloc_protobuf_record)(self.obj as *const _ as *const (), len);

        if ptr.is_null() {
            eprintln!("ptr for record is null");
            return;
        }

        let mut buf = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        record.encode(&mut buf).unwrap();

        (self.on_protobuf_record)(self.obj as *const _ as *const (), ptr, len);
    }
}
