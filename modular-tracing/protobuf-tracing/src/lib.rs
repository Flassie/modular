pub use crate::interest::Interest;
use crate::layer::ProtobufLayer;
pub use crate::recorder::*;

mod interest;
pub mod layer;
mod recorder;
pub(crate) mod span_fields;
pub mod types;

pub use prost::*;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry;

pub fn layer<R: Recorder>(r: &'static R) -> ProtobufLayer {
    ProtobufLayer::new(r)
}

pub fn register_module_tracer<R: Recorder>(recorder: &'static R) {
    let registry = registry().with(layer(recorder));
    let _ = tracing::subscriber::set_global_default(registry);
}
