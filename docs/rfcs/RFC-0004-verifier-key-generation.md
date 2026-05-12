# RFC-0004: Verifier key generation and binding

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

`vi keygen` is a single, deterministic command that fetches (or accepts) a model checkpoint, hashes it canonically, runs the CommitLLM key-generation routine at the pinned commit, and emits a `VIKY` envelope binding the resulting CommitLLM key to `(model_id, checkpoint_hash, commitllm_pin, seed)`. Two invocations with the same inputs produce byte-identical output. The key is public verification material, not a secret.

## Motivation

The verifier needs material derived from the model's weights to check receipts. CommitLLM provides this; we provide the developer surface that turns "read a paper, run a benchmark script" into one command that yields a single binary file with a known size budget and a stable on-disk layout. The binding header lets the verifier refuse a mismatched receipt before any cryptographic work happens.

## Goals

- Deterministic output in `(model_id, checkpoint_hash, commitllm_pin, seed)` ([FR-17](../../PRD.md)).
- A single artifact under 10 MB for Llama 3.2 1B W8A8 ([NFR-2](../../PRD.md)).
- No GPU required to run `vi keygen`. CPU-only or, optionally, GPU-accelerated CommitLLM internals if the user has one available.
- A clear failure when the model identifier is unknown to v1.
- A documented checkpoint mirror so that `vi keygen` does not depend on continued HF availability of any given upstream checkpoint ([R8](../../PRD.md)).

## Non-Goals

- No key rotation in v1. A key is bound to a pin; a new pin yields a new key.
- No multi-checkpoint keys (combined keys for several models). Out of scope per [NG6](../../PRD.md).
- No key encryption at rest. Keys are public.
- No key import from upstream's `verilm-keygen` tooling in v1; we run our own thin wrapper.

## Proposed Design

### Inputs

- `--model <id>`: canonical identifier; v1 supports exactly `llama-3.2-1b-w8a8`.
- `--checkpoint <path>`: optional local directory; defaults to the published mirror.
- `--output <path>`: optional, defaults to `./key.bin`.
- `--seed <u64>`: optional, defaults to a documented constant (e.g. `0`).

### Steps

1. **Resolve checkpoint.**
   - If `--checkpoint` is provided, validate that the directory contains the expected files (`config.json`, `safetensors`, tokenizer files); the file set is part of the canonical hash input.
   - Otherwise, download from the mirror `AbdelStark/Llama-3.2-1B-Instruct-quantized.w8a8` (or the resolved canonical mirror per [RFC-0012](./RFC-0012-w8a8-quantization.md)) via `huggingface_hub`'s HTTP interface. Resume on partial downloads.
2. **Compute checkpoint hash.**
   - Canonical hash is SHA-256 over the concatenation of:
     - `config.json` bytes.
     - Every `*.safetensors` file's bytes, in lexicographic filename order.
     - `tokenizer.json` (and `tokenizer.model` if present), in fixed order.
   - The exact file set and order is recorded in `docs/spec/03-data-model.md` (to be added under the keygen issue) so two callers compute the same hash.
3. **Compare to expected.**
   - For known `model_id`, the spec carries an `expected_checkpoint_hash` field. If the computed hash differs and `--allow-checkpoint-drift` is not set, fail with `hash_mismatch`.
4. **Generate CommitLLM key.**
   - Delegate to the CommitLLM key-generation routine at the pinned commit. Pass `seed` through. Capture the returned key blob.
5. **Assemble binding header.**
   - Fill in `model_id`, `checkpoint_hash`, `commitllm_pin`, `seed`, `keygen_schema_version`. Compute CRC32C.
6. **Emit envelope.**
   - Magic `VIKY`, `ver = 1`, `flags = 0`. Concatenate binding header + CommitLLM key bytes. Write to `--output`. Refuse to overwrite without `--force`.
7. **Print JSON output.**
   - As specified in [02-public-api.md](../spec/02-public-api.md).

### Determinism

The only non-deterministic inputs to v1's keygen are clock and entropy. Both are excluded:

- No timestamps in the key.
- No random material introduced beyond `seed`.
- CommitLLM's keygen routine is itself deterministic given seed; we re-test that property in CI.

CI re-runs `vi keygen` against a fixture checkpoint and asserts byte-identical output between two runs and between two CI agents.

### Failure modes

| Failure | Category | Exit code |
|---------|----------|-----------|
| Unknown `--model` | `input` | 2 |
| Local checkpoint directory missing files | `input` | 2 |
| Network failure during mirror download | `network` | 3 |
| Computed checkpoint hash does not match expected | `hash_mismatch` | 4 |
| Output path exists, `--force` not set | `input` | 2 |
| CommitLLM key generation panic / error (bug) | `internal` | 70 |

### Mirror policy

[RFC-0012](./RFC-0012-w8a8-quantization.md) covers the mirror. Once we publish a W8A8 checkpoint under our account, the `expected_checkpoint_hash` for `llama-3.2-1b-w8a8` is the SHA-256 of that mirror. If we later switch to an upstream-maintained W8A8 checkpoint, the hash updates as a documented MINOR-with-pin-change event.

## Alternatives Considered

**Treat the key as the CommitLLM key bytes directly, no envelope.** Rejected per [RFC-0003](./RFC-0003-receipt-format-pinning.md): no identity, no version, no place to record the pin.

**Allow user to skip checkpoint hashing for speed.** Rejected: hash is a few seconds for 1B; it is the only check that catches a substituted checkpoint at keygen time.

**Run keygen on GPU only.** Rejected: forces the integrating-developer persona to have a GPU before they can verify; defeats the project's UX premise.

**Multiple keys per file (multi-model bundle).** Rejected as out of scope; ergonomic for a hypothetical multi-model future, costs complexity now.

## Drawbacks

- The mirror is operational responsibility we take on. Mitigation: the mirror is a copy of the upstream W8A8 checkpoint when one exists; we are not the source of truth, we are a fixed snapshot.
- Canonical hash discipline is strict: any change to the file set requires careful version planning. Mitigation: file set is fixed by `config.json` content + safetensors shards + tokenizer files; the upstream tokenizer or config bumping triggers a deliberate revision.

## Migration / Rollout

- Initial implementation lands behind the keygen tracking issue. Mirror is uploaded first; the `expected_checkpoint_hash` value gets pinned in code once the upload is final.
- A pre-release dry-run on a fresh laptop confirms the full path from `cargo install` to a valid `key.bin`.

## Testing Strategy

- Determinism test in CI: run twice, diff bytes, must be identical.
- Drift test: a synthetic checkpoint with a flipped byte causes `hash_mismatch`.
- Network failure test: mock HTTP returns a 500; CLI exits `network`.
- Size-budget test: `key.bin` size compared against the 10 MB envelope ([08-performance-budget.md](../spec/08-performance-budget.md)).
- Round-trip test: `vi-receipt` decodes the emitted `VIKY` envelope and recovers the binding fields.

## Open Questions

None.

## References

- [RFC-0003](./RFC-0003-receipt-format-pinning.md)
- [RFC-0011](./RFC-0011-commitllm-upstream-pinning.md)
- [RFC-0012](./RFC-0012-w8a8-quantization.md)
- [03-data-model.md](../spec/03-data-model.md)
