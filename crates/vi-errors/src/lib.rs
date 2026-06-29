//! Error taxonomy and exit-code map for the verifiable-intelligence CLI.
//!
//! Filled in by RFC-0014 implementation issues.

use std::fmt;

use serde_json::{Map, Value};

/// Stable schema version for stderr error envelopes.
pub const ERROR_ENVELOPE_SCHEMA_VERSION: u16 = 1;

/// Stable error category strings in exit-code order, excluding process-level
/// `usage` and `sigint` outcomes handled outside `ViError`.
pub const CATEGORY_STRINGS: [&str; 10] = [
    "verification_failed",
    "input",
    "network",
    "hash_mismatch",
    "receipt_missing",
    "unknown_version",
    "identity_mismatch",
    "unsupported_tier",
    "corrupt_envelope",
    "internal",
];

/// Canonical process exit code for top-level CLI usage errors.
pub const USAGE_EXIT_CODE: i32 = 64;

/// Canonical process exit code for SIGINT interruption.
pub const SIGINT_EXIT_CODE: i32 = 130;

const DEFAULT_SUBCOMMAND: &str = "";
const DEFAULT_TRACE_ID: &str = "";

/// Serializable stderr error envelope emitted by CLI boundaries.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ErrorEnvelope<'a> {
    /// Always true for error envelopes.
    pub error: bool,
    /// Error envelope schema version.
    pub schema_version: u16,
    /// Subcommand that produced the error.
    pub subcommand: &'a str,
    /// Stable error category string.
    pub category: &'static str,
    /// Process exit code for this error.
    pub exit_code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Category-specific machine-readable detail.
    pub detail: Value,
    /// Canonical remediation hint for this error category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<&'static str>,
    /// Per-process trace identifier for log correlation.
    pub trace_id: &'a str,
}

impl<'a> ErrorEnvelope<'a> {
    /// Build an error envelope while preserving the caller's `trace_id`.
    #[must_use]
    pub fn new(subcommand: &'a str, trace_id: &'a str, error: &ViError) -> Self {
        Self {
            error: true,
            schema_version: ERROR_ENVELOPE_SCHEMA_VERSION,
            subcommand,
            category: error.category(),
            exit_code: error.exit_code(),
            message: error.to_string(),
            detail: error_detail(error),
            remediation: Some(error.remediation()),
            trace_id,
        }
    }
}

impl From<&ViError> for ErrorEnvelope<'static> {
    fn from(error: &ViError) -> Self {
        Self::new(DEFAULT_SUBCOMMAND, DEFAULT_TRACE_ID, error)
    }
}

/// Typed error taxonomy for all library errors that reach a CLI boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum ViError {
    /// Bad arguments, malformed input, or a missing required value.
    Input {
        /// Argument, file, or user-visible field that caused the error.
        arg: String,
        /// Human-readable reason suitable for the error envelope message.
        reason: String,
        /// Category-specific detail preserved for later JSON envelope shaping.
        detail: Option<serde_json::Value>,
    },
    /// Transport or HTTP failure talking to a provider or audit endpoint.
    Network {
        /// Endpoint URL or endpoint identifier.
        endpoint: String,
        /// Stable network failure kind.
        kind: NetworkErrorKind,
        /// HTTP status when the transport completed with a non-success status.
        http_status: Option<u16>,
    },
    /// Structurally valid receipt failed a verification phase.
    VerificationFailed {
        /// Verification phase that failed.
        phase: PhaseId,
        /// Measured value when the phase produces a numeric comparison.
        measured: Option<f64>,
        /// Tolerance value when the phase has one.
        tolerance: Option<f64>,
        /// Extra phase-specific detail preserved for later JSON envelope shaping.
        extra: Option<serde_json::Value>,
    },
    /// Receipt identity did not bind to the loaded verifier key.
    IdentityMismatch {
        /// Identity fields expected by the loaded key or caller.
        expected: IdentityFields,
        /// Identity fields carried by the receipt or provider response.
        actual: IdentityFields,
    },
    /// Envelope version or schema version is not supported.
    UnknownVersion {
        /// Envelope magic or envelope name, such as `VIRC`.
        envelope: &'static str,
        /// Field containing the unsupported version.
        field: &'static str,
        /// Unsupported version value.
        value: u32,
        /// Version values supported by this binary.
        supported: Vec<u32>,
    },
    /// Requested verification tier cannot be satisfied with available inputs.
    UnsupportedTier {
        /// User-requested tier name.
        requested: String,
        /// Human-readable reason the tier cannot run.
        reason: String,
    },
    /// Envelope bytes are malformed or fail structural integrity checks.
    CorruptEnvelope {
        /// Envelope magic or envelope name, such as `VIRC`.
        envelope: &'static str,
        /// Byte offset where corruption was detected.
        offset: usize,
        /// Static parser or validation reason.
        reason: &'static str,
    },
    /// Checkpoint or artifact hash does not match the expected canonical hash.
    HashMismatch {
        /// Expected hash string.
        expected: String,
        /// Actual hash string.
        actual: String,
    },
    /// Provider accepted receipt opt-in but did not return a receipt.
    ReceiptMissing {
        /// Endpoint URL or endpoint identifier.
        endpoint: String,
        /// Response content type observed by the client.
        content_type: String,
    },
    /// Panic or programmer error caught at the binary boundary.
    Internal {
        /// Captured backtrace or boundary diagnostic.
        backtrace: String,
    },
}

impl ViError {
    /// Stable wire category string for this error.
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::VerificationFailed { .. } => "verification_failed",
            Self::Input { .. } => "input",
            Self::Network { .. } => "network",
            Self::HashMismatch { .. } => "hash_mismatch",
            Self::ReceiptMissing { .. } => "receipt_missing",
            Self::UnknownVersion { .. } => "unknown_version",
            Self::IdentityMismatch { .. } => "identity_mismatch",
            Self::UnsupportedTier { .. } => "unsupported_tier",
            Self::CorruptEnvelope { .. } => "corrupt_envelope",
            Self::Internal { .. } => "internal",
        }
    }

    /// Stable process exit code for this error category.
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::VerificationFailed { .. } => 1,
            Self::Input { .. } => 2,
            Self::Network { .. } => 3,
            Self::HashMismatch { .. } => 4,
            Self::ReceiptMissing { .. } => 5,
            Self::UnknownVersion { .. } => 6,
            Self::IdentityMismatch { .. } => 7,
            Self::UnsupportedTier { .. } => 8,
            Self::CorruptEnvelope { .. } => 9,
            Self::Internal { .. } => 70,
        }
    }

    /// Canonical remediation hint for this error category.
    #[must_use]
    pub const fn remediation(&self) -> &'static str {
        match self {
            Self::Input { .. } => "Check the argument value or file path.",
            Self::Network { .. } => {
                "Check the endpoint URL and network connectivity; retry on transient failures."
            }
            Self::VerificationFailed { .. } => {
                "Re-fetch the receipt; if the failure persists, the provider's deployment may have drifted from the pinned model."
            }
            Self::IdentityMismatch { .. } => {
                "Use the verifier key that matches this receipt's model and CommitLLM pin."
            }
            Self::UnknownVersion { .. } => "Upgrade `vi` or downgrade the provider so versions align.",
            Self::UnsupportedTier { .. } => {
                "Use a supported tier or provide the missing inputs (e.g., `--audit-endpoint`)."
            }
            Self::CorruptEnvelope { .. } => {
                "The artifact has been damaged. Re-fetch the original."
            }
            Self::HashMismatch { .. } => {
                "The checkpoint at the source has changed. Re-download or use `--allow-checkpoint-drift` if you are intentionally pinning."
            }
            Self::ReceiptMissing { .. } => {
                "The provider did not emit a receipt. Check that the endpoint supports `X-Verifiable-Receipt: 1` and that the prover is healthy."
            }
            Self::Internal { .. } => "Please file an issue with the trace_id.",
        }
    }
}

impl fmt::Display for ViError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input { arg, reason, .. } => write!(formatter, "invalid input for {arg}: {reason}"),
            Self::Network {
                endpoint,
                kind,
                http_status,
            } => match http_status {
                Some(status) => write!(
                    formatter,
                    "network error calling {endpoint}: {kind} (HTTP {status})"
                ),
                None => write!(formatter, "network error calling {endpoint}: {kind}"),
            },
            Self::VerificationFailed {
                phase,
                measured,
                tolerance,
                ..
            } => match (measured, tolerance) {
                (Some(measured), Some(tolerance)) => write!(
                    formatter,
                    "verification phase {phase} failed: measured {measured} exceeds tolerance {tolerance}"
                ),
                _ => write!(formatter, "verification phase {phase} failed"),
            },
            Self::IdentityMismatch { expected, actual } => write!(
                formatter,
                "identity mismatch: expected {expected}, actual {actual}"
            ),
            Self::UnknownVersion {
                envelope,
                field,
                value,
                supported,
            } => write!(
                formatter,
                "unsupported {envelope} {field} version {value}; supported versions: {supported:?}"
            ),
            Self::UnsupportedTier { requested, reason } => {
                write!(formatter, "unsupported tier {requested}: {reason}")
            }
            Self::CorruptEnvelope {
                envelope,
                offset,
                reason,
            } => write!(
                formatter,
                "corrupt {envelope} envelope at offset {offset}: {reason}"
            ),
            Self::HashMismatch { expected, actual } => {
                write!(formatter, "hash mismatch: expected {expected}, actual {actual}")
            }
            Self::ReceiptMissing {
                endpoint,
                content_type,
            } => write!(
                formatter,
                "receipt missing from {endpoint} response with content type {content_type}"
            ),
            Self::Internal { .. } => formatter.write_str("internal error"),
        }
    }
}

impl std::error::Error for ViError {}

/// Stable verification phase identifiers used in error detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseId {
    /// Embedding Merkle commitment phase.
    EmbeddingMerkle,
    /// Shell Freivalds check phase.
    ShellFreivalds,
    /// Bridge replay check phase.
    BridgeReplay,
    /// Attention corridor check phase.
    AttentionCorridor,
    /// KV provenance check phase.
    KvProvenance,
    /// LM head check phase.
    LmHead,
    /// Decode-policy check phase.
    DecodePolicy,
}

impl PhaseId {
    /// Stable wire string for this verification phase.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmbeddingMerkle => "embedding_merkle",
            Self::ShellFreivalds => "shell_freivalds",
            Self::BridgeReplay => "bridge_replay",
            Self::AttentionCorridor => "attention_corridor",
            Self::KvProvenance => "kv_provenance",
            Self::LmHead => "lm_head",
            Self::DecodePolicy => "decode_policy",
        }
    }
}

impl fmt::Display for PhaseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable network failure kind for provider and audit endpoint calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkErrorKind {
    /// DNS resolution failed.
    Dns,
    /// TCP connection was refused.
    ConnectionRefused,
    /// TLS handshake failed.
    Tls,
    /// TLS stream ended during the handshake.
    TlsHandshakeEof,
    /// Request timed out.
    Timeout,
    /// Endpoint returned a non-success HTTP status.
    HttpStatus,
    /// Failure did not fit a more specific stable kind.
    Other,
}

impl NetworkErrorKind {
    /// Stable wire string for this network failure kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::ConnectionRefused => "connection_refused",
            Self::Tls => "tls",
            Self::TlsHandshakeEof => "tls_handshake_eof",
            Self::Timeout => "timeout",
            Self::HttpStatus => "http_status",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for NetworkErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Identity binding fields compared between verifier keys and receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityFields {
    /// Public model identifier.
    pub model_id: String,
    /// Canonical checkpoint hash.
    pub checkpoint_hash: String,
    /// Pinned upstream `CommitLLM` revision.
    pub commitllm_pin: String,
    /// Optional verifier-key envelope hash when the mismatch is key-specific.
    pub key_hash: Option<String>,
}

impl IdentityFields {
    /// Build identity fields from owned or borrowed strings.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        checkpoint_hash: impl Into<String>,
        commitllm_pin: impl Into<String>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            checkpoint_hash: checkpoint_hash.into(),
            commitllm_pin: commitllm_pin.into(),
            key_hash: None,
        }
    }

    /// Attach a verifier-key envelope hash to identity details.
    #[must_use]
    pub fn with_key_hash(mut self, key_hash: impl Into<String>) -> Self {
        self.key_hash = Some(key_hash.into());
        self
    }
}

impl fmt::Display for IdentityFields {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.key_hash {
            Some(key_hash) => write!(
                formatter,
                "model_id={}, checkpoint_hash={}, commitllm_pin={}, key_hash={}",
                self.model_id, self.checkpoint_hash, self.commitllm_pin, key_hash
            ),
            None => write!(
                formatter,
                "model_id={}, checkpoint_hash={}, commitllm_pin={}",
                self.model_id, self.checkpoint_hash, self.commitllm_pin
            ),
        }
    }
}

fn error_detail(error: &ViError) -> Value {
    match error {
        ViError::Input {
            arg,
            reason,
            detail,
        } => input_detail(arg, reason, detail.as_ref()),
        ViError::Network {
            endpoint,
            kind,
            http_status,
        } => serde_json::json!({
            "endpoint": endpoint,
            "kind": kind.as_str(),
            "http_status": http_status,
        }),
        ViError::VerificationFailed {
            phase,
            measured,
            tolerance,
            extra,
        } => serde_json::json!({
            "phase": phase.as_str(),
            "measured": measured,
            "tolerance": tolerance,
            "extra": extra,
        }),
        ViError::IdentityMismatch { expected, actual } => serde_json::json!({
            "expected": identity_detail(expected),
            "actual": identity_detail(actual),
        }),
        ViError::UnknownVersion {
            envelope,
            field,
            value,
            supported,
        } => serde_json::json!({
            "envelope": envelope,
            "field": field,
            "value": value,
            "supported": supported,
        }),
        ViError::UnsupportedTier { requested, reason } => serde_json::json!({
            "requested_tier": requested,
            "reason": reason,
        }),
        ViError::CorruptEnvelope {
            envelope,
            offset,
            reason,
        } => serde_json::json!({
            "envelope": envelope,
            "offset": offset,
            "reason": reason,
        }),
        ViError::HashMismatch { expected, actual } => serde_json::json!({
            "expected": expected,
            "actual": actual,
        }),
        ViError::ReceiptMissing {
            endpoint,
            content_type,
        } => serde_json::json!({
            "endpoint": endpoint,
            "content_type": content_type,
            "expected": "multipart/mixed",
        }),
        ViError::Internal { backtrace } => serde_json::json!({
            "backtrace": backtrace,
        }),
    }
}

fn input_detail(arg: &str, reason: &str, detail: Option<&Value>) -> Value {
    let mut fields = Map::new();
    fields.insert("arg".to_owned(), Value::String(arg.to_owned()));
    fields.insert("reason".to_owned(), Value::String(reason.to_owned()));

    match detail {
        Some(Value::Object(extra)) => {
            for (key, value) in extra {
                if !fields.contains_key(key) {
                    fields.insert(key.clone(), value.clone());
                }
            }
        }
        Some(value) => {
            fields.insert("detail".to_owned(), value.clone());
        }
        None => {}
    }

    Value::Object(fields)
}

fn identity_detail(identity: &IdentityFields) -> Value {
    let mut fields = Map::new();
    fields.insert(
        "model_id".to_owned(),
        Value::String(identity.model_id.clone()),
    );
    fields.insert(
        "checkpoint_hash".to_owned(),
        Value::String(identity.checkpoint_hash.clone()),
    );
    fields.insert(
        "commitllm_pin".to_owned(),
        Value::String(identity.commitllm_pin.clone()),
    );
    if let Some(key_hash) = &identity.key_hash {
        fields.insert("key_hash".to_owned(), Value::String(key_hash.clone()));
    }
    Value::Object(fields)
}

#[cfg(test)]
mod tests {
    use super::{
        ErrorEnvelope, IdentityFields, NetworkErrorKind, PhaseId, ViError, CATEGORY_STRINGS,
        ERROR_ENVELOPE_SCHEMA_VERSION, SIGINT_EXIT_CODE, USAGE_EXIT_CODE,
    };
    use serde_json::{json, Value};

    const SUBCOMMAND: &str = "verify";
    const TRACE_ID: &str = "01JTRACE";

    fn sample_identity() -> IdentityFields {
        IdentityFields::new("model", "sha256:abc", "25541e83")
    }

    fn envelope_value(error: &ViError) -> Value {
        serde_json::to_value(ErrorEnvelope::new(SUBCOMMAND, TRACE_ID, error))
            .expect("error envelope should serialize")
    }

    fn assert_envelope(error: &ViError, expected: &Value) {
        assert_eq!(&envelope_value(error), expected);
    }

    fn sample_errors() -> Vec<ViError> {
        vec![
            ViError::VerificationFailed {
                phase: PhaseId::BridgeReplay,
                measured: Some(47.0),
                tolerance: Some(10.0),
                extra: None,
            },
            ViError::Input {
                arg: "--receipt".to_owned(),
                reason: "file not found".to_owned(),
                detail: None,
            },
            ViError::Network {
                endpoint: "https://provider.example".to_owned(),
                kind: NetworkErrorKind::Timeout,
                http_status: None,
            },
            ViError::HashMismatch {
                expected: "sha256:expected".to_owned(),
                actual: "sha256:actual".to_owned(),
            },
            ViError::ReceiptMissing {
                endpoint: "https://provider.example".to_owned(),
                content_type: "application/json".to_owned(),
            },
            ViError::UnknownVersion {
                envelope: "VIRC",
                field: "ver",
                value: 9,
                supported: vec![1],
            },
            ViError::IdentityMismatch {
                expected: sample_identity(),
                actual: IdentityFields::new("other-model", "sha256:def", "25541e83"),
            },
            ViError::UnsupportedTier {
                requested: "full".to_owned(),
                reason: "missing --audit-endpoint".to_owned(),
            },
            ViError::CorruptEnvelope {
                envelope: "VIRC",
                offset: 7,
                reason: "binding_crc32 mismatch",
            },
            ViError::Internal {
                backtrace: "panic at boundary".to_owned(),
            },
        ]
    }

    #[test]
    fn exit_codes_match_spec() {
        let pairs: Vec<(&'static str, i32)> = sample_errors()
            .iter()
            .map(|error| (error.category(), error.exit_code()))
            .collect();

        assert_eq!(
            pairs,
            vec![
                ("verification_failed", 1),
                ("input", 2),
                ("network", 3),
                ("hash_mismatch", 4),
                ("receipt_missing", 5),
                ("unknown_version", 6),
                ("identity_mismatch", 7),
                ("unsupported_tier", 8),
                ("corrupt_envelope", 9),
                ("internal", 70),
            ]
        );
        assert_eq!(USAGE_EXIT_CODE, 64);
        assert_eq!(SIGINT_EXIT_CODE, 130);
    }

    #[test]
    fn category_strings_are_stable() {
        assert_eq!(
            CATEGORY_STRINGS,
            [
                "verification_failed",
                "input",
                "network",
                "hash_mismatch",
                "receipt_missing",
                "unknown_version",
                "identity_mismatch",
                "unsupported_tier",
                "corrupt_envelope",
                "internal",
            ]
        );

        let sample_categories: Vec<_> = sample_errors().iter().map(ViError::category).collect();
        assert_eq!(sample_categories, CATEGORY_STRINGS);
    }

    #[test]
    fn phase_strings_are_stable() {
        let phases = [
            PhaseId::EmbeddingMerkle,
            PhaseId::ShellFreivalds,
            PhaseId::BridgeReplay,
            PhaseId::AttentionCorridor,
            PhaseId::KvProvenance,
            PhaseId::LmHead,
            PhaseId::DecodePolicy,
        ];

        assert_eq!(
            phases.map(PhaseId::as_str),
            [
                "embedding_merkle",
                "shell_freivalds",
                "bridge_replay",
                "attention_corridor",
                "kv_provenance",
                "lm_head",
                "decode_policy",
            ]
        );
    }

    #[test]
    fn network_kind_strings_are_stable() {
        let kinds = [
            NetworkErrorKind::Dns,
            NetworkErrorKind::ConnectionRefused,
            NetworkErrorKind::Tls,
            NetworkErrorKind::TlsHandshakeEof,
            NetworkErrorKind::Timeout,
            NetworkErrorKind::HttpStatus,
            NetworkErrorKind::Other,
        ];

        assert_eq!(
            kinds.map(NetworkErrorKind::as_str),
            [
                "dns",
                "connection_refused",
                "tls",
                "tls_handshake_eof",
                "timeout",
                "http_status",
                "other",
            ]
        );
    }

    #[test]
    fn error_envelope_from_error_uses_default_context() {
        let envelope = ErrorEnvelope::from(&ViError::Internal {
            backtrace: "panic".to_owned(),
        });

        assert!(envelope.error);
        assert_eq!(envelope.schema_version, ERROR_ENVELOPE_SCHEMA_VERSION);
        assert_eq!(envelope.subcommand, "");
        assert_eq!(envelope.trace_id, "");
    }

    #[test]
    fn serializes_input_envelope() {
        assert_envelope(
            &ViError::Input {
                arg: "--receipt".to_owned(),
                reason: "file not found".to_owned(),
                detail: Some(json!({ "path": "./receipt.bin" })),
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "input",
                "exit_code": 2,
                "message": "invalid input for --receipt: file not found",
                "detail": {
                    "arg": "--receipt",
                    "path": "./receipt.bin",
                    "reason": "file not found",
                },
                "remediation": "Check the argument value or file path.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_network_envelope() {
        assert_envelope(
            &ViError::Network {
                endpoint: "https://provider.example".to_owned(),
                kind: NetworkErrorKind::Timeout,
                http_status: None,
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "network",
                "exit_code": 3,
                "message": "network error calling https://provider.example: timeout",
                "detail": {
                    "endpoint": "https://provider.example",
                    "http_status": null,
                    "kind": "timeout",
                },
                "remediation": "Check the endpoint URL and network connectivity; retry on transient failures.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_verification_failed_envelope() {
        assert_envelope(
            &ViError::VerificationFailed {
                phase: PhaseId::BridgeReplay,
                measured: Some(47.0),
                tolerance: Some(10.0),
                extra: Some(json!({ "norm": "L_inf" })),
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "verification_failed",
                "exit_code": 1,
                "message": "verification phase bridge_replay failed: measured 47 exceeds tolerance 10",
                "detail": {
                    "extra": { "norm": "L_inf" },
                    "measured": 47.0,
                    "phase": "bridge_replay",
                    "tolerance": 10.0,
                },
                "remediation": "Re-fetch the receipt; if the failure persists, the provider's deployment may have drifted from the pinned model.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_identity_mismatch_envelope() {
        assert_envelope(
            &ViError::IdentityMismatch {
                expected: sample_identity(),
                actual: IdentityFields::new("other-model", "sha256:def", "25541e83"),
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "identity_mismatch",
                "exit_code": 7,
                "message": "identity mismatch: expected model_id=model, checkpoint_hash=sha256:abc, commitllm_pin=25541e83, actual model_id=other-model, checkpoint_hash=sha256:def, commitllm_pin=25541e83",
                "detail": {
                    "actual": {
                        "checkpoint_hash": "sha256:def",
                        "commitllm_pin": "25541e83",
                        "model_id": "other-model",
                    },
                    "expected": {
                        "checkpoint_hash": "sha256:abc",
                        "commitllm_pin": "25541e83",
                        "model_id": "model",
                    },
                },
                "remediation": "Use the verifier key that matches this receipt's model and CommitLLM pin.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_unknown_version_envelope() {
        assert_envelope(
            &ViError::UnknownVersion {
                envelope: "VIRC",
                field: "ver",
                value: 9,
                supported: vec![1],
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "unknown_version",
                "exit_code": 6,
                "message": "unsupported VIRC ver version 9; supported versions: [1]",
                "detail": {
                    "envelope": "VIRC",
                    "field": "ver",
                    "supported": [1],
                    "value": 9,
                },
                "remediation": "Upgrade `vi` or downgrade the provider so versions align.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_unsupported_tier_envelope() {
        assert_envelope(
            &ViError::UnsupportedTier {
                requested: "full".to_owned(),
                reason: "missing --audit-endpoint".to_owned(),
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "unsupported_tier",
                "exit_code": 8,
                "message": "unsupported tier full: missing --audit-endpoint",
                "detail": {
                    "reason": "missing --audit-endpoint",
                    "requested_tier": "full",
                },
                "remediation": "Use a supported tier or provide the missing inputs (e.g., `--audit-endpoint`).",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_corrupt_envelope() {
        assert_envelope(
            &ViError::CorruptEnvelope {
                envelope: "VIRC",
                offset: 7,
                reason: "binding_crc32 mismatch",
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "corrupt_envelope",
                "exit_code": 9,
                "message": "corrupt VIRC envelope at offset 7: binding_crc32 mismatch",
                "detail": {
                    "envelope": "VIRC",
                    "offset": 7,
                    "reason": "binding_crc32 mismatch",
                },
                "remediation": "The artifact has been damaged. Re-fetch the original.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_hash_mismatch_envelope() {
        assert_envelope(
            &ViError::HashMismatch {
                expected: "sha256:expected".to_owned(),
                actual: "sha256:actual".to_owned(),
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "hash_mismatch",
                "exit_code": 4,
                "message": "hash mismatch: expected sha256:expected, actual sha256:actual",
                "detail": {
                    "actual": "sha256:actual",
                    "expected": "sha256:expected",
                },
                "remediation": "The checkpoint at the source has changed. Re-download or use `--allow-checkpoint-drift` if you are intentionally pinning.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_receipt_missing_envelope() {
        assert_envelope(
            &ViError::ReceiptMissing {
                endpoint: "https://provider.example".to_owned(),
                content_type: "application/json".to_owned(),
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "receipt_missing",
                "exit_code": 5,
                "message": "receipt missing from https://provider.example response with content type application/json",
                "detail": {
                    "content_type": "application/json",
                    "endpoint": "https://provider.example",
                    "expected": "multipart/mixed",
                },
                "remediation": "The provider did not emit a receipt. Check that the endpoint supports `X-Verifiable-Receipt: 1` and that the prover is healthy.",
                "trace_id": TRACE_ID,
            }),
        );
    }

    #[test]
    fn serializes_internal_envelope() {
        assert_envelope(
            &ViError::Internal {
                backtrace: "panic at boundary".to_owned(),
            },
            &json!({
                "error": true,
                "schema_version": 1,
                "subcommand": SUBCOMMAND,
                "category": "internal",
                "exit_code": 70,
                "message": "internal error",
                "detail": {
                    "backtrace": "panic at boundary",
                },
                "remediation": "Please file an issue with the trace_id.",
                "trace_id": TRACE_ID,
            }),
        );
    }
}
