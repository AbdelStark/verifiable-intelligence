# 07 — Testing Strategy

## Pyramid

```
                    ┌───────────────────────────┐
                    │ Comprehension gates       │  manual, gating
                    │  (SM-5, SM-6)             │
                    ├───────────────────────────┤
                    │ Corridor measurement      │  GPU, on-demand
                    │  (FR-13, FR-14, SM-4)     │
                    ├───────────────────────────┤
                    │ End-to-end (fresh-env)    │  CI per PR, gated
                    │  (SM-1)                   │
                    ├───────────────────────────┤
                    │ Tamper fuzz harness       │  CI per PR + nightly
                    │  (FR-15, SM-3)            │
                    ├───────────────────────────┤
                    │ Integration tests         │  CI per PR
                    │  (fixture-based)          │
                    ├───────────────────────────┤
                    │ Property tests            │  CI per PR
                    │  (envelope roundtrip etc) │
                    ├───────────────────────────┤
                    │ Unit tests                │  CI per PR
                    └───────────────────────────┘
```

Each layer specified below has concrete cases, not categories.

## 1. Unit tests

Per crate, owned by the implementing engineer.

### `vi-receipt`

- Magic prefix mismatch returns `corrupt_envelope`.
- Truncated length-prefixed string returns `corrupt_envelope` with the offending offset.
- CRC32C mismatch on binding header returns `corrupt_envelope`.
- Round-trip: `encode(decode(bytes)) == bytes` for valid envelopes.
- Unknown version byte returns `unknown_version`.
- Flag bits beyond v1's allowed set return `corrupt_envelope`.

### `vi-keygen`

- Determinism: two invocations with identical `(model_id, checkpoint_path, seed)` produce byte-identical outputs.
- Checkpoint hash mismatch (user supplies a checkpoint inconsistent with declared model) returns `hash_mismatch`.
- Output file refusal: existing path without `--force` returns `input` error.

### `vi-verifier`

- Receipt with `key_hash` mismatch fails with `identity_mismatch`.
- Receipt with `commitllm_pin` mismatch fails with `identity_mismatch`.
- Receipt with `model_id` mismatch fails with `identity_mismatch`.
- Unsupported tier requested without required data fails with `unsupported_tier`.
- Phase failure surfaces correct phase name and measured-vs-tolerance values.

### `vi-client`

- HTTP URL rejected with `input` error.
- 4xx response surfaces `http_status` and does not retry.
- Missing receipt part on a `X-Verifiable-Receipt: 1` request returns `receipt_missing`.
- Malformed multipart returns `corrupt_envelope`.

### `vi-cli`

- Each subcommand's `--help` is captured as a fixture and snapshot-tested. `--help` is public API.
- Exit code table is exhaustively covered: every category has at least one path that produces it.

### `vi-tui`

- Phase-walk rendering with mock verifier: green sequence for clean receipt, red interruption for tampered receipt.
- `--phase-delay` actually inserts delay (measured deterministically via mock clock).
- `--tamper byte-flip` produces a `verification_failed` or `corrupt_envelope`, never a success.

### `vi-log`

- Redaction: a prompt or generated text never appears in the structured log output at any verbosity below `TRACE`.
- `TRACE` may include a hex-prefix of receipt bytes but never the full payload.
- `trace_id` is consistent across all events in a single process.

## 2. Property tests

Implemented with `proptest`.

- **Envelope round-trip**: for any well-formed envelope payload, `decode(encode(x)) == x`.
- **Envelope fuzz**: for any random byte string, `decode` either returns a typed error or a value, never panics.
- **Verifier byte-level robustness**: for any random mutation of a valid receipt, the verifier returns either `verification_failed`, `corrupt_envelope`, or `identity_mismatch`; never a success, never a panic.
- **JSON output schema conformance**: for any valid execution, the JSON output validates against the published schema.

## 3. Integration tests

Run in CI on every PR with fixtures checked in to `tests/fixtures/`:

- A known-good receipt + key + expected JSON report. `vi verify` must produce byte-identical JSON (modulo `elapsed_ms`).
- A known-bad receipt (single-byte flip) + key. `vi verify` must produce `verification_failed` or `corrupt_envelope`.
- A receipt for model A + key for model B. `vi verify` must produce `identity_mismatch`.
- A receipt at an unknown version. `vi verify` must produce `unknown_version`.

Fixtures are versioned alongside the receipt schema; the README documents how to regenerate them when the pin changes.

## 4. Tamper fuzz harness

Detailed in [RFC-0009](../rfcs/RFC-0009-tamper-fuzz-harness.md).

- Per PR: 100 random single-byte flips against the canonical fixture. 100% rejection required.
- Nightly: 1000 random flips. 100% rejection required.
- Failures are surfaced as a structured artifact: the flipped offset, the response category, the response detail.
- A "lazy" failure (returns success despite tamper) is the highest-severity regression and blocks releases.

## 5. End-to-end (fresh-environment)

Run on a clean Docker container, no project caches, no pre-installed Rust.

Sequence:

1. Install `vi` via `cargo install` (or download the binary appropriate for the platform).
2. `vi keygen --model llama-3.2-1b-w8a8`.
3. `vi chat --endpoint <ci-endpoint> --prompt "<fixed>"`.
4. `vi verify --receipt receipt.bin --key key.bin --tier routine`.

Acceptance:

- Total wall-clock time under 10 minutes ([SM-1](../../PRD.md)).
- Exit code 0 at every step.
- JSON output validates against schemas.

The CI endpoint is a long-running provider container in the CI runner, not the public demo. The model checkpoint is mirrored to avoid HF rate limits during CI.

## 6. Corridor measurement

GPU-backed, on-demand (not per PR).

Workloads ([FR-13](../../PRD.md)):

- **Short-answer factual.** 200 prompts. "What causes rainbows", "Capital of Paraguay", etc.
- **Multi-turn reasoning.** 100 conversations of 3 turns each. Arithmetic, simple logic.
- **Long-context code.** 50 prompts with 4–8 KB context windows of code, asking for an extension.

Metrics:

- Global `L_inf` (max absolute deviation from teacher in the attention output).
- First-generated-token max.
- Decode max.
- `frac_eq` (fraction of teacher-prover comparisons exactly equal).
- `frac<=1` (fraction within ±1).
- Growth of `L_inf` with context length.

Output:

- JSON report under `reports/corridor/<date>-<commit>.json`.
- Markdown summary appended to `docs/measurements/corridor.md`.
- README badges updated automatically when a measurement is accepted to main.

Acceptance ([SM-4](../../PRD.md)):

- `frac<=1 >= 99.5%` across all three workloads.
- If outside CommitLLM's 7B/8B envelope, the gap is documented in the same PR and the published tolerance is tightened or upstream is escalated per [RFC-0010](../rfcs/RFC-0010-corridor-measurement.md).

## 7. Comprehension gates (manual, pre-release)

- **SM-5**: README reviewed by 5 external readers; 5/5 correctly identify (a) open-weights only, (b) interactive challenge required, (c) attention corridor is empirical not exact. Tracked in a release-gate issue.
- **SM-6**: TUI shown to 3 non-cryptographers; 3/3 correctly describe the green-then-red transition. Tracked in a release-gate issue.

Failure to pass either gate blocks v1.0 release.

## 8. Performance benchmarks

`cargo bench`-driven, run nightly:

- `verify` full-tier on the canonical fixture: target p95 < 1 s on the reference runner.
- `verify` routine-tier: target p95 < 200 ms.
- Key load time and parse cost broken out separately.

Benchmark results are published to `reports/perf/` and the README's perf table is regenerated on release.

## 9. CI determinism guards

- Build is reproducible per `Cargo.lock` and `rust-toolchain.toml`.
- Docker image is built with `--no-cache` periodically (weekly) to catch silent base-image drift.
- `cargo deny` runs on every PR for license and vulnerability advisories.

## 10. Test inventory document

A test inventory at `docs/testing/inventory.md` lists every test fixture, every benchmark, and every gate, with owners. Generated rather than hand-maintained where feasible.
