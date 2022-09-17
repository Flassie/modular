use ansi_term::Color;
use protobuf_tracing::types::{Record, Value, Values};
use protobuf_tracing::{Interest, Recorder};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::stdout;
use std::io::Write;

pub struct DefaultRecorder {
    is_stdout_tty: bool,
    is_stderr_tty: bool,
}

impl DefaultRecorder {
    pub fn new() -> Self {
        Self {
            is_stdout_tty: atty::is(atty::Stream::Stdout),
            is_stderr_tty: atty::is(atty::Stream::Stderr),
        }
    }
}

impl Default for DefaultRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder for DefaultRecorder {
    fn is_interested(&self, interest: &Interest) -> bool {
        true
    }

    fn record(&self, record: &Record) {
        let mut f = stdout().lock();
        let _ = write!(f, "{} ", record.timestamp);

        let color = match record.level.as_str() {
            "ERROR" => Color::Red,
            "WARN" => Color::Yellow,
            "INFO" => Color::Green,
            "DEBUG" => Color::Blue,
            "TRACE" => Color::Purple,
            _ => Color::White,
        };
        let c = Color::Cyan;
        let c2 = Color::Fixed(245);

        let _ = write!(f, "{: >10}", color.paint(&record.level));
        let _ = write!(f, " {}", c.paint(&record.target));

        let event_fields = record.fields.format_fields();

        if !event_fields.is_empty() {
            let _ = write!(f, "{{{}}}", c2.paint(event_fields));
        }

        if let Some(parent) = record.spans.first() {
            let _ = write!(f, "{}{}", c2.paint("::"), c.paint(&parent.name));
            let fields = parent.fields.format_fields();
            if !fields.is_empty() {
                let _ = write!(f, "{{{}}}", c2.paint(fields));
            }
        }

        writeln!(f, ": {}", record.message).unwrap();
    }
}

trait FieldsFormat {
    fn format_fields(&self) -> String;
}

impl FieldsFormat for HashMap<String, Values> {
    fn format_fields(&self) -> String {
        self.iter()
            .map(|(k, v)| {
                let values = v
                    .values
                    .iter()
                    .filter_map(|v| v.value.as_ref().map(|i| format!("`{}`", i)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{}={}", k, values)
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub use protobuf_tracing::register_module_tracer;
