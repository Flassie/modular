use crate::interest::Interest;
use crate::types::Record;

pub trait Recorder: Send + Sync {
    fn is_interested(&self, interest: &Interest) -> bool;
    fn record(&self, record: &Record);
}
