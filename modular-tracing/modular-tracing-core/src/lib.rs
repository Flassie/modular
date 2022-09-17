use ansi_term::Color;
use native_recorder::{BytesRecorder, NativeBytesRecorder};
use protobuf_tracing::types::{Record, Value, Values};
use protobuf_tracing::{DecodeError, Interest, Message, Recorder};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::stdout;
use std::io::Write;
use std::thread;
use std::time::Duration;
use tracing::error;

pub struct LazyRecorder {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    recorder: &'static dyn Recorder,
}

impl LazyRecorder {
    pub fn run(self) {
        std::thread::spawn(move || {
            let rx = self.rx;
            while let Ok(v) = rx.recv() {
                let record = match Record::decode(v.as_slice()) {
                    Ok(record) => record,
                    Err(e) => {
                        error!("failed to decode record: {:?}", e);
                        continue;
                    }
                };

                self.recorder.record(&record);
            }
        });
    }
}

#[derive(Clone)]
pub struct LazyBytesRecorder {
    recorder: &'static dyn Recorder,
    tx: std::sync::mpsc::SyncSender<Vec<u8>>,
}

impl BytesRecorder for LazyBytesRecorder {
    fn is_interested(&self, interest: &Interest) -> bool {
        self.recorder.is_interested(interest)
    }

    fn record(&self, record: Vec<u8>) {
        let _ = self.tx.send(record);
    }
}

impl LazyRecorder {
    pub fn new(recorder: &'static dyn Recorder) -> (Self, LazyBytesRecorder) {
        let (tx, rx) = std::sync::mpsc::sync_channel(1024);

        (Self { rx, recorder }, LazyBytesRecorder { recorder, tx })
    }
}

#[derive(Clone)]
pub struct DefaultRecorder {
    is_stdout_tty: bool,
    is_stderr_tty: bool,
}

impl BytesRecorder for DefaultRecorder {
    fn is_interested(&self, interest: &Interest) -> bool {
        Recorder::is_interested(self, interest)
    }

    fn record(&self, record: Vec<u8>) {
        match Record::decode(record.as_slice()) {
            Ok(v) => Recorder::record(self, &v),
            Err(err) => {
                error!("Failed to decode protobuf record: {:?}", err);
            }
        }
    }
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
        if interest.target == "wasmer_compiler_cranelift::translator::func_translator"
            || interest.target.starts_with("wasmer_")
        {
            return false;
        }

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
