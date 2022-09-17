use crate::interest::Interest;
use crate::span_fields::SpanFields;
use crate::types::{Record, Span};
use crate::Recorder;
use chrono::Local;
use tracing::span::Attributes;
use tracing::{Event, Id};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

pub struct ProtobufLayer {
    handler: &'static dyn Recorder,
}
impl ProtobufLayer {
    pub fn new<R: Recorder>(r: &'static R) -> Self {
        Self { handler: r }
    }
}

impl<S> tracing_subscriber::Layer<S> for ProtobufLayer
where
    S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("span not found");

        let mut fields = SpanFields::default();
        attrs.record(&mut fields);

        span.extensions_mut().insert(fields);
    }

    fn event_enabled(&self, _event: &Event<'_>, _ctx: Context<'_, S>) -> bool {
        self.handler.is_interested(&Interest {
            target: _event.metadata().target(),
            parent_span_name: _event
                .parent()
                .and_then(|i| _ctx.span(i))
                .or_else(|| _ctx.lookup_current())
                .map(|i| i.name()),
        })
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = SpanFields::default();
        event.record(&mut fields);

        let span = event
            .parent()
            .and_then(|id| ctx.span(id))
            .or_else(|| ctx.lookup_current());
        let scope = span.into_iter().flat_map(|span| span.scope());

        let mut spans = Vec::with_capacity(4);
        for span in scope {
            let s = Span {
                name: span.name().to_string(),
                target: span.metadata().target().to_string(),
                level: span.metadata().level().to_string(),
                file: span.metadata().file().map(|s| s.to_string()),
                line: span.metadata().line(),
                fields: span
                    .extensions()
                    .get::<SpanFields>()
                    .map(|i| i.fields.clone())
                    .unwrap_or_default(),
            };

            spans.push(s)
        }

        let record = Record {
            message: fields.message.unwrap_or_default(),
            target: event.metadata().target().to_string(),
            level: event.metadata().level().to_string(),
            file: event.metadata().file().map(|s| s.to_string()),
            line: event.metadata().line(),
            fields: fields.fields,
            spans,
            thread: std::thread::current().name().map(|i| i.to_string()),
            timestamp: Local::now().to_string(),
        };

        self.handler.record(&record);
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("span not found");
        span.extensions_mut().remove::<SpanFields>();
    }
}
