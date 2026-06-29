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
    Event, Level, Subscriber,
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
        let mut fields = JsonFieldVisitor::new(*metadata.level());
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

/// Subscriber-level redaction policy for event fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct RedactionLayer;

impl RedactionLayer {
    fn redact_value(field: &str, value: Value, level: Level) -> Option<Value> {
        match redaction_action(field) {
            RedactionAction::Keep => Some(value),
            RedactionAction::Drop => None,
            RedactionAction::BytesPrefixAtTrace => {
                if level == Level::TRACE {
                    Some(value_to_hex_prefix(value))
                } else {
                    None
                }
            }
            RedactionAction::SanitizeArgs => Some(sanitize_args_value(value)),
        }
    }

    fn redact_bytes(field: &str, value: &[u8], level: Level) -> Option<Value> {
        match redaction_action(field) {
            RedactionAction::Keep => Some(Value::Array(
                value
                    .iter()
                    .map(|byte| Value::Number(Number::from(*byte)))
                    .collect(),
            )),
            RedactionAction::Drop => None,
            RedactionAction::BytesPrefixAtTrace => {
                (level == Level::TRACE).then(|| Value::String(hex_prefix(value)))
            }
            RedactionAction::SanitizeArgs => Some(sanitize_args_value(Value::String(
                String::from_utf8_lossy(value).into_owned(),
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RedactionAction {
    Keep,
    Drop,
    BytesPrefixAtTrace,
    SanitizeArgs,
}

fn redaction_action(field: &str) -> RedactionAction {
    match field.to_ascii_lowercase().as_str() {
        "prompt" | "generated_text" | "answer" | "bundle_json" | "key_bytes" | "authorization"
        | "cookie" | "set-cookie" | "set_cookie" | "api_key" | "vi_api_key" => {
            RedactionAction::Drop
        }
        "receipt_bytes" | "audit_bytes" => RedactionAction::BytesPrefixAtTrace,
        "args" => RedactionAction::SanitizeArgs,
        _ => RedactionAction::Keep,
    }
}

fn value_to_hex_prefix(value: Value) -> Value {
    match value {
        Value::String(value) => Value::String(hex_prefix(value.as_bytes())),
        Value::Array(values) => {
            let bytes: Vec<u8> = values
                .into_iter()
                .filter_map(|value| value.as_u64().and_then(|byte| u8::try_from(byte).ok()))
                .collect();
            Value::String(hex_prefix(&bytes))
        }
        value => Value::String(hex_prefix(value.to_string().as_bytes())),
    }
}

fn hex_prefix(bytes: &[u8]) -> String {
    const PREFIX_BYTES: usize = 32;
    let mut output = String::with_capacity(PREFIX_BYTES * 2 + 3);
    for byte in bytes.iter().take(PREFIX_BYTES) {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    if bytes.len() > PREFIX_BYTES {
        output.push_str("...");
    }
    output
}

fn sanitize_args_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(sanitize_args(values)),
        Value::String(value) => Value::String(sanitize_args_string(&value)),
        value => Value::String(sanitize_args_string(&value.to_string())),
    }
}

fn sanitize_args(values: Vec<Value>) -> Vec<Value> {
    let mut sanitized = Vec::with_capacity(values.len());
    let mut redact_next = false;

    for value in values {
        let Some(token) = value.as_str() else {
            sanitized.push(value);
            continue;
        };

        if redact_next {
            sanitized.push(Value::String("[REDACTED]".to_owned()));
            redact_next = false;
            continue;
        }

        if token == "--api-key" {
            sanitized.push(Value::String(token.to_owned()));
            redact_next = true;
        } else if token.starts_with("--api-key=") {
            sanitized.push(Value::String("--api-key=[REDACTED]".to_owned()));
        } else if token.to_ascii_uppercase().starts_with("VI_API_KEY=") {
            sanitized.push(Value::String("VI_API_KEY=[REDACTED]".to_owned()));
        } else {
            sanitized.push(Value::String(token.to_owned()));
        }
    }

    sanitized
}

fn sanitize_args_string(value: &str) -> String {
    let tokens = value
        .split_whitespace()
        .map(|token| Value::String(token.to_owned()))
        .collect();
    sanitize_args(tokens)
        .into_iter()
        .map(|value| {
            value
                .as_str()
                .map_or_else(|| value.to_string(), ToOwned::to_owned)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug)]
struct JsonFieldVisitor {
    fields: Map<String, Value>,
    level: Level,
}

impl JsonFieldVisitor {
    fn new(level: Level) -> Self {
        Self {
            fields: Map::new(),
            level,
        }
    }

    fn into_fields(self) -> Map<String, Value> {
        self.fields
    }

    fn insert(&mut self, field: &Field, value: Value) {
        if let Some(value) = RedactionLayer::redact_value(field.name(), value, self.level) {
            self.fields.insert(field.name().to_owned(), value);
        }
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

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        if let Some(value) = RedactionLayer::redact_bytes(field.name(), value, self.level) {
            self.fields.insert(field.name().to_owned(), value);
        }
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

    #[test]
    fn redaction_policy_covers_field_map_at_info_debug_and_trace() {
        let levels = [Level::INFO, Level::DEBUG, Level::TRACE];
        let cases = [
            ("prompt", RedactionAction::Drop),
            ("prompt_hash", RedactionAction::Keep),
            ("generated_text", RedactionAction::Drop),
            ("answer", RedactionAction::Drop),
            ("answer_hash", RedactionAction::Keep),
            ("text_chars", RedactionAction::Keep),
            ("key_bytes", RedactionAction::Drop),
            ("receipt_bytes", RedactionAction::BytesPrefixAtTrace),
            ("audit_bytes", RedactionAction::BytesPrefixAtTrace),
            ("authorization", RedactionAction::Drop),
            ("cookie", RedactionAction::Drop),
            ("set-cookie", RedactionAction::Drop),
            ("api_key", RedactionAction::Drop),
            ("VI_API_KEY", RedactionAction::Drop),
            ("args", RedactionAction::SanitizeArgs),
        ];

        for level in levels {
            for (field, action) in cases {
                let output = RedactionLayer::redact_value(field, sample_value(field), level);
                match action {
                    RedactionAction::Keep => assert!(output.is_some(), "{field} at {level}"),
                    RedactionAction::Drop => assert!(output.is_none(), "{field} at {level}"),
                    RedactionAction::BytesPrefixAtTrace => {
                        if level == Level::TRACE {
                            assert_eq!(
                                output.expect("TRACE byte prefix").as_str(),
                                Some("7365637265742d6279746573")
                            );
                        } else {
                            assert!(output.is_none(), "{field} at {level}");
                        }
                    }
                    RedactionAction::SanitizeArgs => {
                        let output = output.expect("args are sanitized");
                        let rendered = output.to_string();
                        assert!(!rendered.contains("secret-token"));
                        assert!(rendered.contains("[REDACTED]"));
                    }
                }
            }
        }
    }

    #[test]
    fn info_prompt_misuse_does_not_emit_prompt() {
        let buffer = SharedBuffer::default();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_span_list(false)
                .with_writer(buffer.clone())
                .event_format(ViEventFormatter::new("chat", "trace-test"))
                .with_filter(EnvFilter::new("info")),
        );

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                event = "chat.request",
                prompt = "raw user prompt",
                prompt_hash = "sha256:prompt"
            );
        });

        let lines = buffer.lines();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].contains("raw user prompt"));
        let value: Value = serde_json::from_str(&lines[0]).expect("log line is JSON");
        assert!(value.get("prompt").is_none());
        assert_eq!(value["prompt_hash"], "sha256:prompt");
    }

    #[test]
    fn trace_byte_fields_emit_hex_prefix_only() {
        let buffer = SharedBuffer::default();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_span_list(false)
                .with_writer(buffer.clone())
                .event_format(ViEventFormatter::new("verify", "trace-test"))
                .with_filter(EnvFilter::new("trace")),
        );
        let bytes: Vec<u8> = (0..40).collect();

        tracing::subscriber::with_default(subscriber, || {
            tracing::trace!(
                event = "verify.bytes",
                receipt_bytes = bytes.as_slice(),
                audit_bytes = bytes.as_slice()
            );
        });

        let lines = buffer.lines();
        assert_eq!(lines.len(), 1);
        let value: Value = serde_json::from_str(&lines[0]).expect("log line is JSON");
        let expected = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f...";
        assert_eq!(value["receipt_bytes"], expected);
        assert_eq!(value["audit_bytes"], expected);
        assert!(!value["receipt_bytes"]
            .as_str()
            .expect("receipt prefix is a string")
            .contains("202122"));
    }

    fn sample_value(field: &str) -> Value {
        match field {
            "args" => serde_json::json!(["vi", "chat", "--api-key", "secret-token"]),
            "receipt_bytes" | "audit_bytes" => Value::String("secret-bytes".to_owned()),
            _ => Value::String(format!("sample-{field}")),
        }
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
