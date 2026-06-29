//! Verification pipeline wrapping the upstream `CommitLLM` verifier.
//!
//! Filled in by RFC-0003 / RFC-0008 implementation issues.

/// Full upstream `CommitLLM` commit SHA used by this verifier crate.
pub const COMMITLLM_PIN: &str = "25541e83347655e44ad6e84eb901e1e7ae392a66";

/// Buyer-facing short `CommitLLM` pin carried in demo proof bundles.
pub const COMMITLLM_SHORT_PIN: &str = "25541e83";

pub fn placeholder() {}
