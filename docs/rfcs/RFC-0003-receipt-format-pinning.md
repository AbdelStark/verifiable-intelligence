# RFC-0003: Receipt format pinning and version handling

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

Every binary artifact this project produces is wrapped in a project-owned envelope (magic + version + flags + payload). The payload is the CommitLLM-defined blob at the pinned upstream commit. The envelope is owned by `vi-receipt`; version handshake, identity binding, and tamper-localizable parsing happen there. We accept exactly the envelope and CommitLLM versions documented in [09-release-and-versioning.md](../spec/09-release-and-versioning.md); anything else fails closed.

## Motivation

CommitLLM ships its own receipt schema and evolves it. We need a stable surface that downstream code (CLI, TUI, future WASM verifier, third-party tooling) can identify and route on without parsing the inner protocol layer. We also need to bind a receipt to a specific key, model, and pin so that a receipt from one deployment cannot be silently verified against a different one.

A magic prefix plus a binding header gives us:

- File-type identification at one byte without a heuristic.
- A small, project-owned point where we can reject incompatible-version artifacts before any complex parsing runs.
- A place to record `model_id`, `checkpoint_hash`, and `commitllm_pin` such that the verifier refuses a receipt that does not match.

## Goals

- A four-byte ASCII magic prefix per artifact kind.
- A one-byte envelope version that is bumped on any binary-layout change to the envelope itself.
- A binding header recording identity fields, integrity-checked with CRC32C.
- A clean separation between "this artifact is for me to parse" (envelope) and "this content is the protocol" (CommitLLM blob).

## Non-Goals

- We do not own the CommitLLM payload format. We pin a commit and accept its bytes.
- We do not introduce cryptographic signatures on receipts in v1 (see [06-security.md](../spec/06-security.md) §"Cryptographic primitives out of scope").
- We do not compress receipts in v1. Receipts are small enough that compression cost outweighs gain.

## Proposed Design

### Envelope layout

See [03-data-model.md §1](../spec/03-data-model.md) for the canonical byte-level layout. Recap:

```
[magic:4][ver:1][flags:1][payload:N]
```

- `magic`: `VIKY` | `VIRC` | `VIAU`.
- `ver`: 1 for v1.
- `flags`: 0 for v1; non-zero rejected.

### Binding header

For receipts and audit payloads, a project-owned binding header precedes the CommitLLM payload. Fields and byte layout are normative in [03-data-model.md §4](../spec/03-data-model.md). The header is integrity-checked with CRC32C — not for security, for early corruption detection.

The verifier:

1. Reads magic; rejects on mismatch with `corrupt_envelope`.
2. Reads `ver`; rejects unknown with `unknown_version`.
3. Reads `flags`; rejects non-zero with `corrupt_envelope`.
4. Reads binding header; verifies CRC32C; rejects mismatch with `corrupt_envelope`.
5. Compares binding fields to the loaded key; rejects mismatch with `identity_mismatch`.
6. Hands the remaining bytes to the CommitLLM verifier crate.

This sequence is the tamper-defense pipeline. The fuzz harness ([RFC-0009](./RFC-0009-tamper-fuzz-harness.md)) targets every byte in the artifact; the design above guarantees a typed error at each layer.

### CommitLLM pin

The upstream pin is recorded in `commitllm.lock` at the repo root, a single line:

```
commitllm = "lambdaclass/CommitLLM@<commit-sha>"
```

`Cargo.toml` references the pinned commit via a Git dependency with `rev = "<sha>"`. The `commitllm_pin` field in binding headers carries the short SHA (8 hex chars) and is checked at verify time.

A pin change is a structured event ([09-release-and-versioning.md](../spec/09-release-and-versioning.md) §"CommitLLM pin changes") with a CHANGELOG entry.

### Backward compatibility within v1.x

- We accept exactly `ver = 1` across all envelopes in v1.x.
- A `ver = 2` envelope cannot be verified by `vi` v1.x; users must upgrade.
- We refuse to silently parse v2 even with a fallback path; fail closed.

### Forward compatibility hooks

- The `flags` byte is reserved. Future uses (compression, signature presence) get distinct bits with documented semantics in v2.
- Adding fields to the binding header requires bumping `keygen_schema_version` (for keys) or `receipt_schema_version` (for receipts). Both are u16 fields inside the binding header itself; bumping is a MINOR if the field is optional, a MAJOR if it is required for verification.

## Alternatives Considered

**No project-owned envelope; ship CommitLLM bytes directly.** Rejected: no way to identify the artifact kind at a glance, no place to record `model_id` and `commitllm_pin`, no way to reject mismatched receipts before CommitLLM parsing starts.

**JSON envelope.** Rejected: CommitLLM payload is intrinsically binary; wrapping it in JSON requires base64 inflation (33% size hit) and a parser pass.

**TLV (type-length-value) envelope.** Rejected: full TLV is overkill for v1 with three artifact kinds. Magic + version + flags is enough.

**Cryptographic signature on the envelope.** Rejected: not needed in v1 because CommitLLM commitments already bind the content; adding our own signature requires a key distribution story we don't have. Revisit in v1.2 (compliance bundles do warrant a signature).

**Track CommitLLM `main` instead of a pinned commit.** Rejected: upstream rename (CommitLLM roadmap #49) is mid-flight; main is moving; we need a stable target for v1.

## Drawbacks

- Two layers of versioning (envelope `ver` and `*_schema_version` inside) is more complex than one. Worth it: the envelope layout itself rarely changes; the inner binding may evolve.
- CRC32C is not a security primitive; a reader who confuses it for one is wrong. The spec says so plainly.

## Migration / Rollout

- No prior format exists; this is the inception.
- The first `vi-receipt` PR lands the envelope codec; the second wires keygen and verify to it.
- The receipt schema and fixtures land together so that integration tests work from day one.

## Testing Strategy

- Round-trip property tests (`proptest`) on the envelope codec.
- Random-byte fuzz: any input either parses to a value or returns a typed error; never panics.
- Magic-mismatch, version-mismatch, flag-set, CRC-mismatch all produce the expected typed error.
- Identity-mismatch tests: receipts cross-pollinated between keys must fail.
- Tamper fuzz harness ([RFC-0009](./RFC-0009-tamper-fuzz-harness.md)) over the full byte range of a valid receipt.

## Open Questions

None at this layer. CommitLLM rename handling lives in [RFC-0011](./RFC-0011-commitllm-upstream-pinning.md).

## References

- [03-data-model.md](../spec/03-data-model.md)
- [RFC-0011](./RFC-0011-commitllm-upstream-pinning.md)
- CommitLLM upstream receipt schema (pinned commit; see `commitllm.lock`)
