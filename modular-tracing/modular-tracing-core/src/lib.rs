use protobuf_tracing::types::Record;
use protobuf_tracing::{Interest, Message, Recorder};
use std::mem::ManuallyDrop;
use std::ptr::null;

#[derive(Copy, Clone)]
pub struct NativeRecorder {
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
}

unsafe impl Send for NativeRecorder {}
unsafe impl Sync for NativeRecorder {}

impl NativeRecorder {
    pub fn new<R: Recorder + 'static>(recorder: &R) -> Self {
        Self {
            obj: recorder as *const R as *const (),
            is_interested: Self::ffi_is_interested::<R>,
            alloc_protobuf_record: Self::ffi_alloc_protobuf_record::<R>,
            on_protobuf_record: Self::ffi_on_protobuf_record::<R>,
        }
    }

    extern "C" fn ffi_is_interested<R: Recorder>(
        obj: *const (),
        target: *const u8,
        target_len: usize,
        span_name: *const u8,
        span_len: usize,
    ) -> u8 {
        let obj = obj as *const R;

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

        let recorder = unsafe { &*obj };
        recorder.is_interested(&interest) as u8
    }

    extern "C" fn ffi_alloc_protobuf_record<R: Recorder>(_: *const (), len: usize) -> *mut u8 {
        let data = vec![0u8; len];
        let data = ManuallyDrop::new(data);

        data.as_ptr() as *mut u8
    }

    extern "C" fn ffi_on_protobuf_record<R: Recorder>(obj: *const (), ptr: *mut u8, len: usize) {
        let obj = obj as *const R;

        let data = unsafe { std::slice::from_raw_parts(ptr, len) };
        let record = Record::decode(data).unwrap();

        let recorder = unsafe { &*obj };
        recorder.record(&record);
    }
}

impl Recorder for NativeRecorder {
    fn is_interested(&self, interest: &Interest) -> bool {
        (self.is_interested)(
            self.obj,
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
        let ptr = (self.alloc_protobuf_record)(self.obj, len);

        if ptr.is_null() {
            eprintln!("ptr for record is null");
            return;
        }

        let mut buf = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        record.encode(&mut buf).unwrap();

        (self.on_protobuf_record)(self.obj, ptr, len);
    }
}

#[test]
fn a() {}
