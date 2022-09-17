use crate::types::Values;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use tracing::field::{Field, Visit};

#[derive(Default)]
pub(crate) struct SpanFields {
    pub message: Option<String>,
    pub fields: HashMap<String, Values>,
}

macro_rules! record {
    ($this:ident, $field:ident, $msg:ident, $value:ident, $ty:ident) => {
        if $field.name() == "message" && $this.message.is_none() {
            $this.message = Some($msg.to_string());
        } else {
            $this
                .fields
                .entry($field.name().to_string())
                .or_default()
                .values
                .push(crate::types::Value {
                    value: Some(crate::types::ValueType::$ty($value)),
                });
        }
    };

    ($this:ident, $field:ident, $value:ident, $ty:ident) => {
        record!($this, $field, $value, $value, $ty)
    };
}

impl Visit for SpanFields {
    fn record_f64(&mut self, field: &Field, value: f64) {
        record!(self, field, value, F64);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        record!(self, field, value, I64);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        record!(self, field, value, U64);
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        let v = value.to_le_bytes().to_vec();
        record!(self, field, value, v, I128);
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        let v = value.to_le_bytes().to_vec();
        record!(self, field, value, v, U128);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        record!(self, field, value, Bool);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let value = value.to_string();
        record!(self, field, value, String);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn Error + 'static)) {
        let value = value.to_string();
        record!(self, field, value, Error);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        let value = format!("{:?}", value);
        record!(self, field, value, String);
    }
}
