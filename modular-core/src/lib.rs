#![allow(dead_code)]

mod callback;
mod errors;
mod module;
mod native_byte_slice;
mod registry;

pub use callback::*;
pub use errors::*;
pub use module::*;
pub use native_byte_slice::*;
pub use registry::*;

#[macro_export]
macro_rules! get_str {
    ($f:expr, $name:ident) => {
        match Option::<&[u8]>::from($f) {
            Some(v) => match std::str::from_utf8(v) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("invalid utf8 in {:?}: {}", stringify!($name), e);
                    panic!("invalid utf8 in {:?}: {}", stringify!($name), e);
                }
            },
            None => {
                tracing::error!("{:?} is empty", stringify!($name));
                panic!("{:?} is empty", stringify!($name));
            }
        }
    };
}
