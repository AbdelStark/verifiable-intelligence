//! Verification pipeline wrapping the upstream `CommitLLM` verifier.
//!
//! Filled in by RFC-0003 / RFC-0008 implementation issues.

// Verification failures must flow through the shared `ViError` taxonomy.
#![allow(clippy::result_large_err)]

use std::time::{Duration, Instant};

use commitllm_core::{
    serialize,
    types::{AuditChallenge as CommitllmAuditChallenge, AuditTier as CommitllmAuditTier},
};
use commitllm_verifier::{canonical, client, FailureCode, V4VerifyReport as CommitllmReport};
use sha2::{Digest, Sha256};
use vi_errors::{PhaseId, ViError};
use vi_receipt::{
    check_audit_receipt_hash, check_receipt_identity, decode_viau_payload, decode_viky_payload,
    decode_virc_payload, AuditBindingHeader, AuditTier, Envelope, Magic, ReceiptBindingHeader,
};

/// Full upstream `CommitLLM` commit SHA used by this verifier crate.
pub const COMMITLLM_PIN: &str = "25541e83347655e44ad6e84eb901e1e7ae392a66";

/// Buyer-facing short `CommitLLM` pin carried in demo proof bundles.
pub const COMMITLLM_SHORT_PIN: &str = "25541e83";

/// Stable project-level verification phases.
pub const VERIFY_PHASES: [PhaseId; 7] = [
    PhaseId::EmbeddingMerkle,
    PhaseId::ShellFreivalds,
    PhaseId::BridgeReplay,
    PhaseId::AttentionCorridor,
    PhaseId::KvProvenance,
    PhaseId::LmHead,
    PhaseId::DecodePolicy,
];

/// Project-level verification verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyVerdict {
    /// All requested checks passed.
    Pass,
    /// At least one requested check failed.
    Fail,
}

/// Verification report returned when the requested tier completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// Requested project audit tier.
    pub tier: AuditTier,
    /// Project-level verdict.
    pub verdict: VerifyVerdict,
    /// Stable project phases represented by this verifier.
    pub phases: Vec<PhaseId>,
    /// Number of checks reported by the upstream verifier.
    pub checks_run: usize,
    /// Number of checks passed by the upstream verifier.
    pub checks_passed: usize,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
    /// Milliseconds elapsed inside the upstream verifier.
    pub elapsed_ms: u64,
}

/// Structured verifier phase event for logs, TUI, and browser demo consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseEvent {
    /// A project-level verification phase started.
    Started {
        /// Stable project phase identifier.
        phase: PhaseId,
    },
    /// A project-level verification phase ended.
    Ended {
        /// Stable project phase identifier.
        phase: PhaseId,
        /// Whether the phase passed.
        passed: bool,
        /// Failure detail suitable for demo/UI rendering.
        detail: Option<String>,
        /// Milliseconds elapsed since the verification run started.
        elapsed_ms: u64,
    },
}

/// Provider abstraction used for tiers that need a `VIAU` audit opening.
pub trait AuditProvider {
    /// Fetch a `VIAU` audit opening for the given receipt and tier.
    fn fetch_audit(
        &mut self,
        receipt: &ReceiptBindingHeader,
        tier: AuditTier,
    ) -> Result<Vec<u8>, ViError>;
}

/// Verify a project-wrapped receipt/key pair at the requested tier.
pub fn verify(
    receipt_bytes: &[u8],
    key_bytes: &[u8],
    tier: AuditTier,
    audit_provider: &mut dyn AuditProvider,
) -> Result<VerifyReport, ViError> {
    verify_inner(receipt_bytes, key_bytes, tier, audit_provider, |_| {})
}

/// Verify a project-wrapped receipt/key pair and emit phase-boundary events.
pub fn verify_with_callback(
    receipt_bytes: &[u8],
    key_bytes: &[u8],
    tier: AuditTier,
    audit_provider: &mut dyn AuditProvider,
    phase_callback: impl FnMut(PhaseEvent),
) -> Result<VerifyReport, ViError> {
    verify_inner(
        receipt_bytes,
        key_bytes,
        tier,
        audit_provider,
        phase_callback,
    )
}

fn verify_inner(
    receipt_bytes: &[u8],
    key_bytes: &[u8],
    tier: AuditTier,
    audit_provider: &mut dyn AuditProvider,
    mut phase_callback: impl FnMut(PhaseEvent),
) -> Result<VerifyReport, ViError> {
    let started_at = Instant::now();
    let result = run_verification_core(receipt_bytes, key_bytes, tier, audit_provider);
    emit_phase_events(&result, &mut phase_callback, started_at);
    result
}

fn run_verification_core(
    receipt_bytes: &[u8],
    key_bytes: &[u8],
    tier: AuditTier,
    audit_provider: &mut dyn AuditProvider,
) -> Result<VerifyReport, ViError> {
    let (key_header, commitllm_key_bytes) = decode_project_key(key_bytes)?;
    let (receipt_header, commitllm_receipt_bytes) = decode_project_receipt(receipt_bytes)?;
    check_receipt_identity(&key_header, &receipt_header)?;
    check_receipt_key_hash(&key_header, &receipt_header, key_bytes)?;

    let key = serialize::deserialize_key(&commitllm_key_bytes)
        .map_err(|_| corrupt_commitllm("VIKY", "CommitLLM key payload is malformed"))?;

    let upstream_report = if tier_requires_audit(tier) {
        let audit_bytes = audit_provider.fetch_audit(&receipt_header, tier)?;
        let (audit_header, commitllm_audit_bytes) = decode_project_audit(&audit_bytes)?;
        check_audit_receipt_hash(&audit_header, &sha256_hex(receipt_bytes))?;
        check_audit_tier(&audit_header, tier)?;
        let challenge = commitllm_challenge(&audit_header)?;
        client::verify_challenged_binary(&challenge, &key, &commitllm_audit_bytes, None, None)
            .map_err(|_| corrupt_commitllm("VIAU", "CommitLLM audit payload is malformed"))?
    } else {
        canonical::verify_binary(&key, &commitllm_receipt_bytes, None, None, None)
            .map_err(|_| corrupt_commitllm("VIRC", "CommitLLM receipt payload is malformed"))?
    };

    report_from_upstream(tier, upstream_report)
}

pub fn placeholder() {}

fn emit_phase_events(
    result: &Result<VerifyReport, ViError>,
    callback: &mut dyn FnMut(PhaseEvent),
    started_at: Instant,
) {
    let failure = result
        .as_ref()
        .err()
        .map(|error| (phase_for_error(error), error.to_string()));

    for phase in VERIFY_PHASES {
        callback(PhaseEvent::Started { phase });

        let Some((failed_phase, detail)) = &failure else {
            callback(PhaseEvent::Ended {
                phase,
                passed: true,
                detail: None,
                elapsed_ms: elapsed_millis(started_at),
            });
            continue;
        };

        let passed = phase != *failed_phase;
        callback(PhaseEvent::Ended {
            phase,
            passed,
            detail: if passed { None } else { Some(detail.clone()) },
            elapsed_ms: elapsed_millis(started_at),
        });

        if !passed {
            break;
        }
    }
}

fn phase_for_error(error: &ViError) -> PhaseId {
    match error {
        ViError::VerificationFailed { phase, .. } => *phase,
        _ => VERIFY_PHASES[0],
    }
}

fn elapsed_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn decode_project_key(bytes: &[u8]) -> Result<(vi_receipt::KeyBindingHeader, Vec<u8>), ViError> {
    let envelope = Envelope::decode(bytes)?;
    if envelope.magic != Magic::VIKY {
        return Err(corrupt_envelope_at_magic("VIKY"));
    }
    let (header, payload) = decode_viky_payload(&envelope.payload)?;
    Ok((header, payload.to_vec()))
}

fn decode_project_receipt(bytes: &[u8]) -> Result<(ReceiptBindingHeader, Vec<u8>), ViError> {
    let envelope = Envelope::decode(bytes)?;
    if envelope.magic != Magic::VIRC {
        return Err(corrupt_envelope_at_magic("VIRC"));
    }
    let (header, payload) = decode_virc_payload(&envelope.payload)?;
    Ok((header, payload.to_vec()))
}

fn decode_project_audit(bytes: &[u8]) -> Result<(AuditBindingHeader, Vec<u8>), ViError> {
    let envelope = Envelope::decode(bytes)?;
    if envelope.magic != Magic::VIAU {
        return Err(corrupt_envelope_at_magic("VIAU"));
    }
    let (header, payload) = decode_viau_payload(&envelope.payload)?;
    Ok((header, payload.to_vec()))
}

fn check_receipt_key_hash(
    key: &vi_receipt::KeyBindingHeader,
    receipt: &ReceiptBindingHeader,
    key_bytes: &[u8],
) -> Result<(), ViError> {
    let actual = sha256_hex(key_bytes);
    if receipt.key_hash == actual {
        Ok(())
    } else {
        Err(ViError::IdentityMismatch {
            expected: key.identity_fields().with_key_hash(actual),
            actual: receipt
                .identity_fields()
                .with_key_hash(receipt.key_hash.clone()),
        })
    }
}

fn check_audit_tier(audit: &AuditBindingHeader, requested: AuditTier) -> Result<(), ViError> {
    if audit.tier == requested {
        Ok(())
    } else {
        Err(ViError::UnsupportedTier {
            requested: requested.to_string(),
            reason: format!("audit opening tier was {}", audit.tier),
        })
    }
}

fn tier_requires_audit(tier: AuditTier) -> bool {
    matches!(tier, AuditTier::Deep | AuditTier::Full)
}

fn commitllm_challenge(audit: &AuditBindingHeader) -> Result<CommitllmAuditChallenge, ViError> {
    let token_index = u32::try_from(audit.token_index).map_err(|_| ViError::Input {
        arg: "token_index".to_owned(),
        reason: "audit token_index exceeds u32".to_owned(),
        detail: None,
    })?;
    let layer_indices = audit
        .layer_indices
        .iter()
        .map(|layer| usize::try_from(*layer))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ViError::Input {
            arg: "layer_indices".to_owned(),
            reason: "audit layer index exceeds usize".to_owned(),
            detail: None,
        })?;
    let tier = match audit.tier {
        AuditTier::Full => CommitllmAuditTier::Full,
        AuditTier::ReceiptOnly | AuditTier::Routine | AuditTier::Deep => {
            CommitllmAuditTier::Routine
        }
    };

    Ok(CommitllmAuditChallenge {
        token_index,
        layer_indices,
        tier,
    })
}

fn report_from_upstream(
    tier: AuditTier,
    upstream: CommitllmReport,
) -> Result<VerifyReport, ViError> {
    if let Some(failure) = upstream.failures.first() {
        return Err(ViError::VerificationFailed {
            phase: phase_for_failure(failure.code),
            measured: None,
            tolerance: None,
            extra: Some(serde_json::json!({
                "code": failure.code.to_string(),
                "message": failure.message.as_str(),
            })),
        });
    }

    Ok(VerifyReport {
        tier,
        verdict: VerifyVerdict::Pass,
        phases: VERIFY_PHASES.to_vec(),
        checks_run: upstream.checks_run,
        checks_passed: upstream.checks_passed,
        warnings: upstream.skipped,
        elapsed_ms: duration_millis(upstream.duration),
    })
}

#[allow(clippy::too_many_lines)]
fn phase_for_failure(code: FailureCode) -> PhaseId {
    use FailureCode::{
        AttentionCertificationFailed, AttentionExactMismatch, AttentionKvCoverageIncomplete,
        AttentionReplayMismatch, AttentionWiringMismatch, BridgeScaleMismatch, BridgeXAttnMismatch,
        ChallengeLayerMismatch, ChallengeTokenMismatch, DecodeArtifactHashMismatch,
        DecodeModeTempInconsistent, DetokenizationMismatch, DetokenizerError,
        EmbeddingLeafMismatch, EmbeddingProofFailed, EosPolicyViolated, ExceedsMaxTokens,
        FreivaldsFailed, IgnoreEosViolated, IoChainMismatch, IoChainProofFailed,
        KvEntriesCountMismatch, KvProofCountMismatch, KvProofInvalid, KvRootsCountMismatch,
        LmHeadFreivaldsFailed, ManifestHashMismatch, MerkleProofFailed, MinTokensViolated,
        MissingEmbeddingProof, MissingEosTokenId, MissingFinalHidden, MissingFinalResidual,
        MissingInitialResidual, MissingLogits, MissingManifestHash, MissingNPromptTokens,
        MissingOutputText, MissingPromptBytes, MissingPromptHash, MissingQkv,
        MissingSeedCommitment, MissingShellOpening, MissingSpecHash, NPromptTokensBound,
        NPromptTokensMismatch, NonContiguousLayerIndices, PrefixCountMismatch,
        PrefixTokenCountMismatch, PromptHashMismatch, PromptTokenCountMismatch,
        PromptTokenMismatch, RetainedHashMismatch, ScoreAnchorMismatch, SeedMismatch,
        ShellLayerCountMismatch, SpecFieldMismatch, SpecHashMismatch, TokenSelectionMismatch,
        TokenizerError, UnboundInitialResidual, UncommittedPrompt, UnknownEosPolicy,
        UnsupportedDecodeFeature, UnsupportedDecodeMode, UnsupportedSamplerVersion,
        WitnessedScoreStructuralError, WrongCommitmentVersion,
    };

    match code {
        MissingEmbeddingProof
        | EmbeddingProofFailed
        | EmbeddingLeafMismatch
        | MissingInitialResidual
        | UnboundInitialResidual => PhaseId::EmbeddingMerkle,

        MissingShellOpening
        | MissingQkv
        | ShellLayerCountMismatch
        | NonContiguousLayerIndices
        | FreivaldsFailed
        | MerkleProofFailed
        | WrongCommitmentVersion
        | PrefixCountMismatch => PhaseId::ShellFreivalds,

        BridgeXAttnMismatch
        | BridgeScaleMismatch
        | IoChainMismatch
        | IoChainProofFailed
        | MissingFinalResidual
        | MissingSeedCommitment
        | SeedMismatch => PhaseId::BridgeReplay,

        AttentionReplayMismatch
        | AttentionExactMismatch
        | AttentionKvCoverageIncomplete
        | ScoreAnchorMismatch
        | WitnessedScoreStructuralError
        | AttentionWiringMismatch
        | AttentionCertificationFailed
        | ChallengeTokenMismatch
        | ChallengeLayerMismatch => PhaseId::AttentionCorridor,

        KvRootsCountMismatch
        | KvEntriesCountMismatch
        | KvProofInvalid
        | KvProofCountMismatch
        | RetainedHashMismatch => PhaseId::KvProvenance,

        LmHeadFreivaldsFailed
        | MissingLogits
        | MissingFinalHidden
        | DecodeArtifactHashMismatch
        | TokenSelectionMismatch => PhaseId::LmHead,

        MissingPromptHash
        | MissingPromptBytes
        | UncommittedPrompt
        | MissingNPromptTokens
        | MissingSpecHash
        | MissingManifestHash
        | MissingOutputText
        | MissingEosTokenId
        | ManifestHashMismatch
        | SpecHashMismatch
        | SpecFieldMismatch
        | UnsupportedSamplerVersion
        | UnsupportedDecodeMode
        | UnsupportedDecodeFeature
        | UnknownEosPolicy
        | ExceedsMaxTokens
        | MinTokensViolated
        | EosPolicyViolated
        | IgnoreEosViolated
        | DecodeModeTempInconsistent
        | PromptTokenMismatch
        | PromptTokenCountMismatch
        | NPromptTokensMismatch
        | NPromptTokensBound
        | PrefixTokenCountMismatch
        | DetokenizationMismatch
        | TokenizerError
        | DetokenizerError
        | PromptHashMismatch => PhaseId::DecodePolicy,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn corrupt_envelope_at_magic(expected: &'static str) -> ViError {
    ViError::CorruptEnvelope {
        envelope: expected,
        offset: 0,
        reason: "unexpected envelope magic",
    }
}

fn corrupt_commitllm(envelope: &'static str, reason: &'static str) -> ViError {
    ViError::CorruptEnvelope {
        envelope,
        offset: vi_receipt::HEADER_LEN,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use vi_errors::{ErrorEnvelope, IdentityFields};
    use vi_receipt::{
        encode_viau_payload, encode_viky_payload, encode_virc_payload, AuditBindingHeader,
        KeyBindingHeader, ReceiptBindingHeader,
    };

    use super::*;

    #[test]
    fn project_verifier_represents_all_seven_phases() {
        assert_eq!(VERIFY_PHASES.len(), 7);
        assert!(VERIFY_PHASES.contains(&PhaseId::EmbeddingMerkle));
        assert!(VERIFY_PHASES.contains(&PhaseId::ShellFreivalds));
        assert!(VERIFY_PHASES.contains(&PhaseId::BridgeReplay));
        assert!(VERIFY_PHASES.contains(&PhaseId::AttentionCorridor));
        assert!(VERIFY_PHASES.contains(&PhaseId::KvProvenance));
        assert!(VERIFY_PHASES.contains(&PhaseId::LmHead));
        assert!(VERIFY_PHASES.contains(&PhaseId::DecodePolicy));
    }

    #[test]
    fn upstream_failure_mapping_can_reach_every_project_phase() {
        let phases = [
            phase_for_failure(FailureCode::EmbeddingProofFailed),
            phase_for_failure(FailureCode::FreivaldsFailed),
            phase_for_failure(FailureCode::BridgeXAttnMismatch),
            phase_for_failure(FailureCode::AttentionExactMismatch),
            phase_for_failure(FailureCode::KvProofInvalid),
            phase_for_failure(FailureCode::LmHeadFreivaldsFailed),
            phase_for_failure(FailureCode::UnsupportedDecodeMode),
        ];

        let actual = BTreeSet::from(phases.map(PhaseId::as_str));
        let expected = BTreeSet::from(VERIFY_PHASES.map(PhaseId::as_str));

        assert_eq!(actual, expected);
    }

    #[test]
    fn receipt_model_identity_mismatch_is_rejected_before_upstream_verify() {
        let key = project_key("model-a");
        let receipt = project_receipt("model-b", &sha256_hex(&key));
        let mut audit_provider = RecordingAuditProvider::default();
        let expected = IdentityFields::new("model-a", "sha256:checkpoint", COMMITLLM_SHORT_PIN);
        let actual = IdentityFields::new("model-b", "sha256:checkpoint", COMMITLLM_SHORT_PIN);

        let error = verify(&receipt, &key, AuditTier::Routine, &mut audit_provider)
            .expect_err("identity mismatch should fail");

        assert_eq!(
            error,
            ViError::IdentityMismatch {
                expected: expected.clone(),
                actual: actual.clone(),
            }
        );
        assert_identity_envelope(&error, &expected, &actual);
        assert_eq!(audit_provider.calls, 0);
    }

    #[test]
    fn receipt_commitllm_pin_mismatch_is_rejected_before_upstream_verify() {
        let key = project_key("model-a");
        let receipt = project_receipt_with_identity(
            "model-a",
            "sha256:checkpoint",
            "ffffffff",
            &sha256_hex(&key),
        );
        let mut audit_provider = RecordingAuditProvider::default();
        let expected = IdentityFields::new("model-a", "sha256:checkpoint", COMMITLLM_SHORT_PIN);
        let actual = IdentityFields::new("model-a", "sha256:checkpoint", "ffffffff");

        let error = verify(&receipt, &key, AuditTier::Routine, &mut audit_provider)
            .expect_err("pin mismatch should fail");

        assert_eq!(
            error,
            ViError::IdentityMismatch {
                expected: expected.clone(),
                actual: actual.clone(),
            }
        );
        assert_identity_envelope(&error, &expected, &actual);
        assert_eq!(audit_provider.calls, 0);
    }

    #[test]
    fn receipt_key_hash_mismatch_is_identity_mismatch_before_upstream_verify() {
        let key = project_key("model-a");
        let wrong_key_hash = "sha256:wrong-key";
        let receipt = project_receipt("model-a", wrong_key_hash);
        let mut audit_provider = RecordingAuditProvider::default();
        let expected = IdentityFields::new("model-a", "sha256:checkpoint", COMMITLLM_SHORT_PIN)
            .with_key_hash(sha256_hex(&key));
        let actual = IdentityFields::new("model-a", "sha256:checkpoint", COMMITLLM_SHORT_PIN)
            .with_key_hash(wrong_key_hash);

        let error = verify(&receipt, &key, AuditTier::Routine, &mut audit_provider)
            .expect_err("key-hash mismatch should fail");

        assert_eq!(
            error,
            ViError::IdentityMismatch {
                expected: expected.clone(),
                actual: actual.clone(),
            }
        );
        assert_identity_envelope(&error, &expected, &actual);
        assert_eq!(audit_provider.calls, 0);
    }

    #[test]
    fn verify_with_callback_emits_mock_consumer_sequence_for_early_failure() {
        let key = project_key("model-a");
        let receipt = project_receipt("model-b", &sha256_hex(&key));
        let mut direct_audit_provider = RecordingAuditProvider::default();
        let mut callback_audit_provider = RecordingAuditProvider::default();
        let mut events = Vec::new();

        let direct_error = verify(
            &receipt,
            &key,
            AuditTier::Routine,
            &mut direct_audit_provider,
        )
        .expect_err("identity mismatch should fail");
        let callback_error = verify_with_callback(
            &receipt,
            &key,
            AuditTier::Routine,
            &mut callback_audit_provider,
            |event| events.push(event),
        )
        .expect_err("identity mismatch should fail");

        assert_eq!(callback_error, direct_error);
        assert_eq!(
            event_markers(&events),
            vec![
                ("started", PhaseId::EmbeddingMerkle, None),
                ("ended", PhaseId::EmbeddingMerkle, Some(false)),
            ]
        );
        assert_eq!(direct_audit_provider.calls, 0);
        assert_eq!(callback_audit_provider.calls, 0);

        let PhaseEvent::Ended {
            detail: Some(detail),
            ..
        } = &events[1]
        else {
            panic!("failed phase should carry detail");
        };
        assert!(detail.contains("identity mismatch"));
    }

    #[test]
    fn phase_event_walk_marks_all_phases_on_success() {
        let result = Ok(VerifyReport {
            tier: AuditTier::Routine,
            verdict: VerifyVerdict::Pass,
            phases: VERIFY_PHASES.to_vec(),
            checks_run: VERIFY_PHASES.len(),
            checks_passed: VERIFY_PHASES.len(),
            warnings: Vec::new(),
            elapsed_ms: 3,
        });
        let mut events = Vec::new();

        emit_phase_events(&result, &mut |event| events.push(event), Instant::now());

        let expected = VERIFY_PHASES
            .into_iter()
            .flat_map(|phase| [("started", phase, None), ("ended", phase, Some(true))])
            .collect::<Vec<_>>();
        assert_eq!(event_markers(&events), expected);
    }

    #[test]
    fn verify_report_json_is_byte_deterministic_modulo_elapsed_time() {
        let (receipt, key, audit) = full_audit_fixture_artifacts();
        let mut first_audit_provider = StaticAuditProvider::new(audit.clone());
        let mut second_audit_provider = StaticAuditProvider::new(audit);

        let first = verify(&receipt, &key, AuditTier::Full, &mut first_audit_provider)
            .expect("full-tier fixture verifies");
        let second = verify(&receipt, &key, AuditTier::Full, &mut second_audit_provider)
            .expect("full-tier fixture verifies");

        let mut first_json = verify_report_json(&first);
        let mut second_json = verify_report_json(&second);
        strip_timing_fields(&mut first_json);
        strip_timing_fields(&mut second_json);

        let first_bytes = serde_json::to_vec(&first_json).expect("report serializes");
        let second_bytes = serde_json::to_vec(&second_json).expect("report serializes");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            std::str::from_utf8(&first_bytes).expect("report JSON is UTF-8"),
            "{\"checks_passed\":39,\"checks_run\":39,\"phases\":[\"embedding_merkle\",\"shell_freivalds\",\"bridge_replay\",\"attention_corridor\",\"kv_provenance\",\"lm_head\",\"decode_policy\"],\"tier\":\"full\",\"verdict\":\"pass\",\"warnings\":[]}"
        );
        assert_eq!(first_audit_provider.calls, 1);
        assert_eq!(second_audit_provider.calls, 1);
    }

    #[test]
    fn routine_tier_never_fetches_audit_payload() {
        let key = project_key("model-a");
        let receipt = project_receipt("model-a", &sha256_hex(&key));
        let mut audit_provider = RecordingAuditProvider::default();

        let error = verify(&receipt, &key, AuditTier::Routine, &mut audit_provider)
            .expect_err("dummy CommitLLM key should not deserialize");

        assert_eq!(audit_provider.calls, 0);
        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "VIKY",
                offset: vi_receipt::HEADER_LEN,
                reason: "CommitLLM key payload is malformed",
            }
        );
    }

    #[derive(Debug, Default)]
    struct RecordingAuditProvider {
        calls: usize,
    }

    impl AuditProvider for RecordingAuditProvider {
        fn fetch_audit(
            &mut self,
            _receipt: &ReceiptBindingHeader,
            _tier: AuditTier,
        ) -> Result<Vec<u8>, ViError> {
            self.calls += 1;
            Ok(Vec::new())
        }
    }

    #[derive(Debug)]
    struct StaticAuditProvider {
        audit: Vec<u8>,
        calls: usize,
    }

    impl StaticAuditProvider {
        fn new(audit: Vec<u8>) -> Self {
            Self { audit, calls: 0 }
        }
    }

    impl AuditProvider for StaticAuditProvider {
        fn fetch_audit(
            &mut self,
            _receipt: &ReceiptBindingHeader,
            _tier: AuditTier,
        ) -> Result<Vec<u8>, ViError> {
            self.calls += 1;
            Ok(self.audit.clone())
        }
    }

    fn project_key(model_id: &str) -> Vec<u8> {
        let header = KeyBindingHeader::new(model_id, "sha256:checkpoint", COMMITLLM_SHORT_PIN, 7);
        let payload = encode_viky_payload(&header, b"not-a-commitllm-key").expect("VIKY encodes");
        Envelope::new(Magic::VIKY, payload)
            .encode()
            .expect("VIKY envelope encodes")
    }

    fn project_receipt(model_id: &str, key_hash: &str) -> Vec<u8> {
        project_receipt_with_identity(model_id, "sha256:checkpoint", COMMITLLM_SHORT_PIN, key_hash)
    }

    fn project_receipt_with_identity(
        model_id: &str,
        checkpoint_hash: &str,
        commitllm_pin: &str,
        key_hash: &str,
    ) -> Vec<u8> {
        let header = ReceiptBindingHeader::new(
            model_id,
            checkpoint_hash,
            commitllm_pin,
            key_hash,
            "sha256:prompt",
            "sha256:answer",
            1,
        );
        let payload =
            encode_virc_payload(&header, b"not-a-commitllm-receipt").expect("VIRC encodes");
        Envelope::new(Magic::VIRC, payload)
            .encode()
            .expect("VIRC envelope encodes")
    }

    fn full_audit_fixture_artifacts() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let raw_key = include_bytes!("../../../verifier/wasm/fixtures/v4_key_fullbridge.bin");
        let raw_audit = include_bytes!("../../../verifier/wasm/fixtures/v4_audit_fullbridge.bin");
        let model_id = "commitllm-fullbridge-fixture";
        let checkpoint_hash = "sha256:fixture-checkpoint";

        let key_header = KeyBindingHeader::new(model_id, checkpoint_hash, COMMITLLM_SHORT_PIN, 0);
        let key_payload = encode_viky_payload(&key_header, raw_key).expect("fixture VIKY encodes");
        let key = Envelope::new(Magic::VIKY, key_payload)
            .encode()
            .expect("fixture VIKY envelope encodes");

        let receipt_header = ReceiptBindingHeader::new(
            model_id,
            checkpoint_hash,
            COMMITLLM_SHORT_PIN,
            sha256_hex(&key),
            "sha256:fixture-prompt",
            "sha256:fixture-answer",
            1,
        );
        let receipt_payload = encode_virc_payload(&receipt_header, b"unused-full-audit-receipt")
            .expect("fixture VIRC encodes");
        let receipt = Envelope::new(Magic::VIRC, receipt_payload)
            .encode()
            .expect("fixture VIRC envelope encodes");

        let audit_header =
            AuditBindingHeader::new(sha256_hex(&receipt), AuditTier::Full, 0, vec![0, 1]);
        let audit_payload =
            encode_viau_payload(&audit_header, raw_audit).expect("fixture VIAU encodes");
        let audit = Envelope::new(Magic::VIAU, audit_payload)
            .encode()
            .expect("fixture VIAU envelope encodes");

        (receipt, key, audit)
    }

    fn event_markers(events: &[PhaseEvent]) -> Vec<(&'static str, PhaseId, Option<bool>)> {
        events
            .iter()
            .map(|event| match event {
                PhaseEvent::Started { phase } => ("started", *phase, None),
                PhaseEvent::Ended { phase, passed, .. } => ("ended", *phase, Some(*passed)),
            })
            .collect()
    }

    fn verify_report_json(report: &VerifyReport) -> serde_json::Value {
        serde_json::json!({
            "tier": report.tier.as_str(),
            "verdict": match report.verdict {
                VerifyVerdict::Pass => "pass",
                VerifyVerdict::Fail => "fail",
            },
            "phases": report
                .phases
                .iter()
                .map(|phase| phase.as_str())
                .collect::<Vec<_>>(),
            "checks_run": report.checks_run,
            "checks_passed": report.checks_passed,
            "warnings": report.warnings,
            "elapsed_ms": report.elapsed_ms,
        })
    }

    fn strip_timing_fields(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    strip_timing_fields(item);
                }
            }
            serde_json::Value::Object(fields) => {
                fields.remove("elapsed_ms");
                for field in fields.values_mut() {
                    strip_timing_fields(field);
                }
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
    }

    fn assert_identity_envelope(
        error: &ViError,
        expected: &IdentityFields,
        actual: &IdentityFields,
    ) {
        let envelope = serde_json::to_value(ErrorEnvelope::new("verify", "trace-identity", error))
            .expect("identity envelope serializes");

        assert_eq!(envelope["category"], "identity_mismatch");
        assert_eq!(envelope["exit_code"], 7);
        assert_eq!(envelope["detail"]["expected"], identity_json(expected));
        assert_eq!(envelope["detail"]["actual"], identity_json(actual));
    }

    fn identity_json(identity: &IdentityFields) -> serde_json::Value {
        let mut value = serde_json::json!({
            "model_id": identity.model_id,
            "checkpoint_hash": identity.checkpoint_hash,
            "commitllm_pin": identity.commitllm_pin,
        });
        if let Some(key_hash) = &identity.key_hash {
            value["key_hash"] = serde_json::Value::String(key_hash.clone());
        }
        value
    }
}
