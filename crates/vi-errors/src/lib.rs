//! Error taxonomy and exit-code map for the verifiable-intelligence CLI.
//!
//! Filled in by RFC-0014 implementation issues.

use std::fmt;

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
        }
    }
}

impl fmt::Display for IdentityFields {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "model_id={}, checkpoint_hash={}, commitllm_pin={}",
            self.model_id, self.checkpoint_hash, self.commitllm_pin
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IdentityFields, NetworkErrorKind, PhaseId, ViError, CATEGORY_STRINGS, SIGINT_EXIT_CODE,
        USAGE_EXIT_CODE,
    };

    fn sample_identity() -> IdentityFields {
        IdentityFields::new("model", "sha256:abc", "25541e83")
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
}
