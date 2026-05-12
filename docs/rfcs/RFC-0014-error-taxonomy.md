# RFC-0014: Error taxonomy and exit codes

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

Errors are typed at the Rust layer via a single `ViError` sum with one variant per category in [04-error-model.md](../spec/04-error-model.md). Conversion from `ViError` to an exit code and to a JSON error envelope is exhaustive and centralized in `vi-errors`. No subcommand emits an error that does not pass through this taxonomy. Exit codes are stable; adding a category is non-breaking, renaming or removing is breaking.

## Motivation

Without a centralized taxonomy, ad-hoc `eyre`/`anyhow` chains decay into generic "operation failed" messages. The integrating-developer persona relies on exit codes for CI integration; the demo-audience persona relies on phase-level detail to see verification failure. The spec already names the categories; this RFC fixes the implementation contract.

## Goals

- One `ViError` enum, one place where the exit code is decided, one place where the JSON envelope is shaped.
- Every error reaching the binary boundary is mapped to a category.
- `clap` parse errors land in `usage` (code 64).
- Panics land in `internal` (code 70) at the boundary; the actual panic does not leak through.
- The error path never silently swallows an upstream typed error.

## Non-Goals

- No internationalization of error messages in v1. English only.
- No structured "error chain" introspection in the JSON envelope beyond `category` + `detail`. Causes are rendered into `message` but not exposed as a separate tree.

## Proposed Design

### `ViError` enum

```rust
#[derive(Debug)]
pub enum ViError {
    Input { arg: String, reason: String, detail: Option<serde_json::Value> },
    Network { endpoint: String, kind: NetworkErrorKind, http_status: Option<u16> },
    VerificationFailed { phase: PhaseId, measured: Option<f64>, tolerance: Option<f64>, extra: Option<serde_json::Value> },
    IdentityMismatch { expected: IdentityFields, actual: IdentityFields },
    UnknownVersion { envelope: &'static str, field: &'static str, value: u32, supported: Vec<u32> },
    UnsupportedTier { requested: String, reason: String },
    CorruptEnvelope { envelope: &'static str, offset: usize, reason: &'static str },
    HashMismatch { expected: String, actual: String },
    ReceiptMissing { endpoint: String, content_type: String },
    Internal { backtrace: String },
}
```

Field shapes deliberately match the JSON envelope shape so the conversion is mechanical.

### Exit code mapping

```rust
impl ViError {
    pub fn exit_code(&self) -> i32 {
        match self {
            ViError::VerificationFailed { .. } => 1,
            ViError::Input { .. } => 2,
            ViError::Network { .. } => 3,
            ViError::HashMismatch { .. } => 4,
            ViError::ReceiptMissing { .. } => 5,
            ViError::UnknownVersion { .. } => 6,
            ViError::IdentityMismatch { .. } => 7,
            ViError::UnsupportedTier { .. } => 8,
            ViError::CorruptEnvelope { .. } => 9,
            ViError::Internal { .. } => 70,
        }
    }
}
```

`clap` parse failure is intercepted at `main` before `ViError` construction; it maps to exit code 64. SIGINT maps to 130 at the signal handler.

### JSON envelope serialization

```rust
#[derive(Serialize)]
pub struct ErrorEnvelope<'a> {
    error: bool, // always true
    schema_version: u16,
    subcommand: &'a str,
    category: &'static str, // stable strings
    exit_code: i32,
    message: String,
    detail: serde_json::Value,
    remediation: Option<&'static str>,
    trace_id: &'a str,
}
```

The `category` strings are stable enum values matching [04-error-model.md](../spec/04-error-model.md). They are unit-tested for round-trip stability.

### Boundary handling

`main` (in `vi-cli`) wraps the dispatch in:

```rust
fn main() {
    let result = run();
    match result {
        Ok(out) => { print_stdout(out); std::process::exit(0); }
        Err(e) => {
            print_stderr_error_envelope(&e);
            std::process::exit(e.exit_code());
        }
    }
}
```

Panics are caught via `std::panic::set_hook` early in `main`; the hook formats a JSON envelope with `category = "internal"` and exits 70. This is the only place we paper over a panic; library code may not.

### Error conversion

Each leaf crate exposes its own typed errors (`ReceiptError`, `NetworkError`, etc.). `vi-errors` provides `From` impls that fan in to `ViError`. The conversion site is in `vi-cli` or `vi-tui`, never in the leaf crates. This keeps leaf crates testable without dragging the full taxonomy.

### Phase enumeration

```rust
pub enum PhaseId {
    EmbeddingMerkle,
    ShellFreivalds,
    BridgeReplay,
    AttentionCorridor,
    KvProvenance,
    LmHead,
    DecodePolicy,
}
```

Stable string representations match the glossary ([10-glossary.md](../spec/10-glossary.md)). Adding a phase is non-breaking (the report carries a new entry); removing is breaking.

### Remediation hints

Each category has one canonical, static remediation string declared once in `vi-errors`. They are intentionally short:

- `Input`: "Check the argument value or file path."
- `Network`: "Check the endpoint URL and network connectivity; retry on transient failures."
- `VerificationFailed`: "Re-fetch the receipt; if the failure persists, the provider's deployment may have drifted from the pinned model."
- `IdentityMismatch`: "Use the verifier key that matches this receipt's model and CommitLLM pin."
- `UnknownVersion`: "Upgrade `vi` or downgrade the provider so versions align."
- `UnsupportedTier`: "Use a supported tier or provide the missing inputs (e.g., `--audit-endpoint`)."
- `CorruptEnvelope`: "The artifact has been damaged. Re-fetch the original."
- `HashMismatch`: "The checkpoint at the source has changed. Re-download or use `--allow-checkpoint-drift` if you are intentionally pinning."
- `ReceiptMissing`: "The provider did not emit a receipt. Check that the endpoint supports `X-Verifiable-Receipt: 1` and that the prover is healthy."
- `Internal`: "Please file an issue with the trace_id."

Remediations are scoped to be true regardless of the specific failure; details go in the `detail` field, not the remediation.

## Alternatives Considered

**Use `anyhow`/`eyre` with string-tag categorization.** Rejected: string-tag taxonomies decay; centralized enum is the durable shape.

**One exit code for all errors.** Rejected: defeats the primary persona's ability to script.

**Per-subcommand error enums.** Rejected: every subcommand can hit network, parse, identity errors; one shared taxonomy fits the data.

**Hide `internal` errors behind a generic "operation failed".** Rejected: surface the trace_id and prompt for a bug report; opacity is worse than honest internal markers.

## Drawbacks

- Adding a category requires updating the spec, the enum, the exit-code map, and tests in one PR. By design.
- Some errors don't fit one category cleanly; the rule is "pick the most actionable one for the caller", documented in the issue template for error contributions.

## Migration / Rollout

- `vi-errors` lands in the workspace bootstrap PR.
- Each leaf crate's typed errors land with it; `From` impls are added incrementally.
- Snapshot tests for the JSON envelope per category land in the first CLI subcommand's PR.

## Testing Strategy

- Every category has at least one test that produces it end-to-end through a subcommand.
- Exit-code mapping is unit-tested exhaustively.
- JSON envelope serialization is snapshot-tested per category.
- A "category strings stable" test asserts the wire strings have not changed (rename detection).
- The `internal` panic-catch is verified by a deliberately-panicking test binary.

## Open Questions

None.

## References

- [04-error-model.md](../spec/04-error-model.md)
- [02-public-api.md](../spec/02-public-api.md)
- [10-glossary.md](../spec/10-glossary.md)
