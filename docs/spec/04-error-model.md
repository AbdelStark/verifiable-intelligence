# 04 — Error Model

This document enumerates every category of failure the system can produce, the user-facing exit code, the user-facing error envelope, and the recovery action. Implementation detail in [RFC-0014](../rfcs/RFC-0014-error-taxonomy.md).

## Principles

1. **Fail closed.** Unknown receipt versions, unknown model identities, unsupported tier requests, partial data, and any structural validation failure produce an error, never a success. ([NFR-6](../../PRD.md))
2. **Distinguish layers.** Network failures, parse failures, identity failures, and verification failures are different categories with different exit codes. They are never collapsed into a generic "failed."
3. **Surface phase detail.** A verification failure names the failing phase and the measured value vs the tolerance. ([FR-9](../../PRD.md))
4. **Errors are JSON.** The error envelope shape is part of the public API.

## Exit code map

| Code | Category | Meaning | Scope |
|------|----------|---------|-------|
| 0 | success | Operation completed successfully | All subcommands |
| 1 | verification_failed | Receipt was structurally valid but verification did not pass | `vi verify`, `vi tui` |
| 2 | input | Bad arguments, malformed input file, missing required flag | All |
| 3 | network | Transport error talking to provider or audit endpoint | `vi chat`, `vi verify --tier=full|deep`, `vi tui` |
| 4 | hash_mismatch | Checkpoint hash does not match the expected canonical hash | `vi keygen` |
| 5 | receipt_missing | Provider did not return a receipt despite opt-in header | `vi chat`, `vi tui` |
| 6 | unknown_version | Receipt, audit, or key envelope has an unknown version byte | `vi verify`, `vi tui` |
| 7 | identity_mismatch | Receipt does not bind to the loaded key (model id, checkpoint, pin, key hash) | `vi verify`, `vi tui` |
| 8 | unsupported_tier | Requested tier requires data not present (e.g. `full` without `--audit-endpoint`) | `vi verify`, `vi tui` |
| 9 | corrupt_envelope | Magic prefix wrong, flags carry unsupported bits, length fields invalid | `vi verify`, `vi keygen` |
| 64 | usage | `--help` requested or argument parsing top-level failure (clap convention) | All |
| 70 | internal | Panic or programmer error caught at the boundary | All |
| 130 | sigint | Interrupted by SIGINT | `vi tui` primarily; all subcommands fall through |

Exit codes outside this table MUST NOT be emitted by `vi`. CI gates this.

## Error envelope (stderr JSON)

```json
{
  "error": true,
  "schema_version": 1,
  "subcommand": "verify",
  "category": "verification_failed",
  "exit_code": 1,
  "message": "Phase bridge_replay failed: L_inf=47 exceeds tolerance=10",
  "detail": {
    "phase": "bridge_replay",
    "measured": 47,
    "tolerance": 10
  },
  "remediation": "Re-fetch the receipt; if the failure persists, the provider's deployment may have drifted from the pinned model.",
  "trace_id": "01J..."
}
```

- `error: true` is always present on error envelopes.
- `category` is a stable enum string. Adding a category is non-breaking; renaming or removing one is breaking.
- `detail` shape is category-specific.
- `remediation` is human-readable, optional but present where there is a real action.
- `trace_id` is a ULID set at process start; correlates with structured logs ([05-observability.md](./05-observability.md)).

## Category catalog

### `input`

Symptoms: argument missing, file not found, unparseable, mutually exclusive flags.

Detail shape:

```json
{ "arg": "--receipt", "reason": "file not found", "path": "./receipt.bin" }
```

Recovery: caller corrects invocation.

### `network`

Symptoms: DNS failure, connection refused, TLS error, HTTP non-2xx, timeout.

Detail shape:

```json
{ "endpoint": "https://...", "kind": "timeout", "after_ms": 30000, "http_status": null }
```

Recovery: retry on `kind=timeout` and `kind=tls_handshake_eof`; do not retry on `http_status` in `4xx`. `vi` does not auto-retry in v1; the integrator's CI loop decides.

### `verification_failed`

Symptoms: at least one phase failed during a structurally-valid verification run.

Detail shape:

```json
{ "phase": "bridge_replay", "measured": <number>, "tolerance": <number>, "extra": {...} }
```

Recovery: investigate the failing phase. Do NOT retry blindly; verification is deterministic given inputs.

### `identity_mismatch`

Symptoms: a binding field on the receipt does not match the key. Examples: receipt was produced against a different model id, checkpoint hash, CommitLLM pin, or the receipt's `key_hash` does not equal the loaded key's hash.

Detail shape:

```json
{
  "expected": { "model_id": "...", "checkpoint_hash": "sha256:...", "commitllm_pin": "..." },
  "actual":   { "model_id": "...", "checkpoint_hash": "sha256:...", "commitllm_pin": "..." }
}
```

Recovery: use the correct key, or accept that the receipt is from a different model.

### `unknown_version`

Symptoms: envelope `ver` byte or `*_schema_version` field is not in the verifier's supported set.

Detail shape:

```json
{ "envelope": "VIRC", "field": "ver", "value": 9, "supported": [1] }
```

Recovery: upgrade `vi` to a version that handles the newer schema, or downgrade the provider.

### `unsupported_tier`

Symptoms: `--tier full` without `--audit-endpoint`; `--tier full` requested but provider is at a CommitLLM pin without full-tier support; tier name unknown.

Detail shape:

```json
{ "requested_tier": "full", "reason": "missing --audit-endpoint" }
```

### `corrupt_envelope`

Symptoms: magic mismatch, length-prefixed string overruns the buffer, CRC mismatch on the binding header, flags carry unsupported bits.

Detail shape:

```json
{ "envelope": "VIRC", "offset": 7, "reason": "binding_crc32 mismatch" }
```

This category is the primary success criterion for the tamper-fuzz harness ([RFC-0009](../rfcs/RFC-0009-tamper-fuzz-harness.md)): every random single-byte flip must produce either `corrupt_envelope` or `verification_failed`, never a success.

### `receipt_missing`

Symptoms: provider responded 2xx to a chat request with `X-Verifiable-Receipt: 1` set, but the response did not carry a receipt part.

Detail shape:

```json
{ "endpoint": "https://...", "content_type": "application/json", "expected": "multipart/mixed" }
```

Recovery: check the endpoint version, the provider's `commitllm_pin`, the `/healthz` advertisement.

### `internal`

Symptoms: a `panic!` caught at the binary boundary; an assertion violated. These are bugs. The trace_id is critical; the bug report procedure is documented in CONTRIBUTING.

## Phase failure detail map

For `verification_failed`, the `detail.phase` value is one of the stable enum strings:

- `embedding_merkle`
- `shell_freivalds`
- `bridge_replay`
- `attention_corridor`
- `kv_provenance`
- `lm_head`
- `decode_policy`

Each phase's `detail` payload includes phase-specific fields. The full per-phase schema is in [`schemas/verify-report.schema.json`](../../schemas/verify-report.schema.json) (to be authored under the tracking issue for schemas).

## Logging vs error envelope

The error envelope on stderr is a single object. Structured logs ([05-observability.md](./05-observability.md)) may include additional events with the same `trace_id`. The envelope is the contract; logs are debugging detail.
