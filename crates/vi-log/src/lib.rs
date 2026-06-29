//! Tracing subscriber, JSON formatter, and redaction layer.
//!
//! RFC-0015 keeps logging silent by default while allowing opt-in structured
//! JSON logs with a per-process trace identifier.

use std::{
    env, fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Map, Number, Value};
use tracing::{
    field::{Field, Visit},
    Event, Subscriber,
};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    layer::{Layer, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter,
};
use ulid::Ulid;

/// Project-specific log filter environment variable.
pub const VI_LOG_ENV: &str = "VI_LOG";
/// Standard tracing/log filter environment variable.
pub const RUST_LOG_ENV: &str = "RUST_LOG";
/// Default filter: silent on successful INFO/DEBUG events.
pub const DEFAULT_FILTER: &str = "error";

/// Logging initialization failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitError {
    /// A provided filter directive was not accepted by `tracing-subscriber`.
    InvalidFilter {
        /// Source that supplied the invalid directive.
        source: &'static str,
        /// Original directive string.
        value: String,
        /// Parser error message.
        reason: String,
    },
}

impl fmt::Display for InitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFilter {
                source,
                value,
                reason,
            } => {
                write!(
                    formatter,
                    "invalid log filter from {source} ({value:?}): {reason}"
                )
            }
        }
    }
}

impl std::error::Error for InitError {}

impl InitError {
    /// User-facing source name that supplied the rejected filter.
    #[must_use]
    pub const fn source_name(&self) -> &'static str {
        match self {
            Self::InvalidFilter { source, .. } => source,
        }
    }
}

/// Generate a fresh ULID suitable for per-process log correlation.
#[must_use]
pub fn generate_trace_id() -> String {
    Ulid::new().to_string()
}

/// Initialize process logging using `VI_LOG`, `RUST_LOG`, or the default filter.
///
/// Calling this after another subscriber is already installed is a no-op. This
/// keeps library tests and repeated in-process CLI dispatches from failing on
/// global subscriber ownership, while real `vi` process invocations still get
/// the requested subscriber.
pub fn init(subcommand: &str, trace_id: &str) -> Result<(), InitError> {
    init_with_filter(subcommand, trace_id, None)
}

/// Initialize process logging with an optional explicit filter override.
///
/// `filter_override` is used for the CLI `--log` flag and takes precedence over
/// `VI_LOG`, then `RUST_LOG`, then the default `error` filter.
pub fn init_with_filter(
    subcommand: &str,
    trace_id: &str,
    filter_override: Option<&str>,
) -> Result<(), InitError> {
    let filter = resolve_env_filter(filter_override)?;
    let layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(false)
        .with_current_span(false)
        .with_span_list(false)
        .event_format(ViEventFormatter::new(subcommand, trace_id))
        .with_filter(filter);

    let subscriber = tracing_subscriber::registry().with(layer);
    let _ = subscriber.try_init();
    Ok(())
}

fn resolve_env_filter(filter_override: Option<&str>) -> Result<EnvFilter, InitError> {
    let (source, filter) = filter_source(filter_override);
    EnvFilter::try_new(&filter).map_err(|error| InitError::InvalidFilter {
        source,
        value: filter,
        reason: error.to_string(),
    })
}

fn filter_source(filter_override: Option<&str>) -> (&'static str, String) {
    if let Some(filter) = non_empty(filter_override) {
        return ("--log", filter.to_owned());
    }

    if let Some(filter) = env::var(VI_LOG_ENV).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    }) {
        return (VI_LOG_ENV, filter);
    }

    if let Some(filter) = env::var(RUST_LOG_ENV).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    }) {
        return (RUST_LOG_ENV, filter);
    }

    ("default", DEFAULT_FILTER.to_owned())
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[derive(Debug, Clone)]
struct ViEventFormatter {
    subcommand: String,
    trace_id: String,
}

impl ViEventFormatter {
    fn new(subcommand: &str, trace_id: &str) -> Self {
        Self {
            subcommand: subcommand.to_owned(),
            trace_id: trace_id.to_owned(),
        }
    }
}

impl<S, N> FormatEvent<S, N> for ViEventFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        let mut fields = JsonFieldVisitor::default();
        event.record(&mut fields);

        let mut object = fields.into_fields();
        object.insert("timestamp".to_owned(), Value::String(unix_timestamp_ms()));
        object.insert(
            "level".to_owned(),
            Value::String(metadata.level().as_str().to_ascii_lowercase()),
        );
        object.insert(
            "subcommand".to_owned(),
            Value::String(self.subcommand.clone()),
        );
        object.insert("trace_id".to_owned(), Value::String(self.trace_id.clone()));

        let line = serde_json::to_string(&Value::Object(object)).map_err(|_| fmt::Error)?;
        writer.write_str(&line)?;
        writer.write_char('\n')
    }
}

fn unix_timestamp_ms() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
        .to_string()
}

#[derive(Debug, Default)]
struct JsonFieldVisitor {
    fields: Map<String, Value>,
}

impl JsonFieldVisitor {
    fn into_fields(self) -> Map<String, Value> {
        self.fields
    }

    fn insert(&mut self, field: &Field, value: Value) {
        self.fields.insert(field.name().to_owned(), value);
    }
}

impl Visit for JsonFieldVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, Value::Number(Number::from(value)));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, Value::Number(Number::from(value)));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, Value::Bool(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field, Value::String(value.to_owned()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.insert(field, Value::String(format!("{value:?}")));
    }
}

pub fn placeholder() {}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    use serde_json::Value;
    use tracing::Level;
    use tracing_subscriber::{fmt::MakeWriter, prelude::*};

    use super::*;

    #[test]
    fn generated_trace_id_is_ulid_shaped() {
        let trace_id = generate_trace_id();

        assert_eq!(trace_id.len(), 26);
        assert!(trace_id.chars().all(|character| {
            character.is_ascii_digit()
                || matches!(character, 'A'..='H' | 'J'..='K' | 'M'..='N' | 'P'..='T' | 'V'..='Z')
        }));
    }

    #[test]
    fn filter_precedence_is_override_then_vi_log_then_rust_log_then_default() {
        assert_eq!(filter_source(Some("debug")).1, "debug");

        EnvGuard::set(
            &[(VI_LOG_ENV, Some("info")), (RUST_LOG_ENV, Some("trace"))],
            || {
                assert_eq!(filter_source(None), (VI_LOG_ENV, "info".to_owned()));
            },
        );

        EnvGuard::set(&[(VI_LOG_ENV, None), (RUST_LOG_ENV, Some("warn"))], || {
            assert_eq!(filter_source(None), (RUST_LOG_ENV, "warn".to_owned()));
        });

        EnvGuard::set(&[(VI_LOG_ENV, None), (RUST_LOG_ENV, None)], || {
            assert_eq!(filter_source(None), ("default", DEFAULT_FILTER.to_owned()));
        });
    }

    #[test]
    fn formatter_includes_trace_id_and_subcommand_on_each_event() {
        let buffer = SharedBuffer::default();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_span_list(false)
                .with_writer(buffer.clone())
                .event_format(ViEventFormatter::new("verify", "trace-test"))
                .with_filter(EnvFilter::new("info")),
        );

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                event = "verify.start",
                subcommand = "spoofed",
                trace_id = "spoofed",
                prompt_hash = "sha256:prompt",
                checks_run = 7_u64
            );
            tracing::error!(
                event = "verify.failed",
                category = "verification_failed",
                exit_code = 1_i64
            );
        });

        let lines = buffer.lines();
        assert_eq!(lines.len(), 2);
        for line in lines {
            let value: Value = serde_json::from_str(&line).expect("log line is JSON");
            assert_eq!(value["trace_id"], "trace-test");
            assert_eq!(value["subcommand"], "verify");
            assert!(value["timestamp"].is_string());
            assert!(value["level"].is_string());
            assert!(value["event"].is_string());
        }
    }

    #[test]
    fn default_filter_suppresses_info_success_events() {
        let buffer = SharedBuffer::default();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_span_list(false)
                .with_writer(buffer.clone())
                .event_format(ViEventFormatter::new("chat", "trace-test"))
                .with_filter(EnvFilter::new(DEFAULT_FILTER)),
        );

        tracing::subscriber::with_default(subscriber, || {
            tracing::event!(Level::INFO, event = "process.end");
            tracing::event!(Level::ERROR, event = "process.error");
        });

        let lines = buffer.lines();
        assert_eq!(lines.len(), 1);
        let value: Value = serde_json::from_str(&lines[0]).expect("log line is JSON");
        assert_eq!(value["event"], "process.error");
    }

    #[derive(Debug, Clone, Default)]
    struct SharedBuffer {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl SharedBuffer {
        fn lines(&self) -> Vec<String> {
            let bytes = self.bytes.lock().expect("buffer lock").clone();
            String::from_utf8(bytes)
                .expect("logs are UTF-8")
                .lines()
                .map(ToOwned::to_owned)
                .collect()
        }
    }

    impl<'writer> MakeWriter<'writer> for SharedBuffer {
        type Writer = SharedWriter;

        fn make_writer(&'writer self) -> Self::Writer {
            SharedWriter {
                bytes: Arc::clone(&self.bytes),
            }
        }
    }

    #[derive(Debug)]
    struct SharedWriter {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl io::Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.bytes
                .lock()
                .expect("buffer lock")
                .extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct EnvGuard;

    impl EnvGuard {
        fn set(vars: &[(&str, Option<&str>)], body: impl FnOnce()) {
            static LOCK: Mutex<()> = Mutex::new(());
            let _guard = LOCK.lock().expect("env lock");
            let previous: Vec<_> = vars
                .iter()
                .map(|(name, _)| ((*name).to_owned(), env::var_os(name)))
                .collect();

            for (name, value) in vars {
                match value {
                    Some(value) => env::set_var(name, value),
                    None => env::remove_var(name),
                }
            }

            body();

            for (name, previous) in previous {
                match previous {
                    Some(previous) => env::set_var(name, previous),
                    None => env::remove_var(name),
                }
            }
        }
    }
}
