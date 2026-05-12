# 03 — Data Model

This document specifies the on-the-wire and on-disk binary contracts the project owns or re-exports. The underlying receipt and audit payload schemas are inherited from CommitLLM at the pinned commit ([RFC-0011](../rfcs/RFC-0011-commitllm-upstream-pinning.md)); the envelope around them is owned here.

## 1. Envelope conventions

Every binary artifact this project produces is a typed envelope:

```
+--------+--------+--------+--------+--------+----------------+
|  magic (4 bytes)         | ver(1) | flags  | payload bytes  |
+--------+--------+--------+--------+--------+----------------+
```

- `magic`: 4 ASCII bytes identifying the artifact kind.
- `ver`: 1 byte envelope version. v1 ships `ver = 1` for all envelopes.
- `flags`: 1 byte. v1: all bits reserved (must be `0`). Future use: compression, encryption marker.
- `payload`: opaque to this project; consumed by CommitLLM crates.

The envelope is parsed by `vi-receipt`. Any artifact whose magic does not match the expected kind, whose version is unknown, or whose flags carry set bits the verifier does not understand, is rejected with a typed error ([RFC-0014](../rfcs/RFC-0014-error-taxonomy.md)).

## 2. Magic prefixes

| Artifact | Magic | Hex |
|----------|-------|-----|
| Verifier key | `VIKY` | `56 49 4B 59` |
| Receipt | `VIRC` | `56 49 52 43` |
| Audit payload | `VIAU` | `56 49 41 55` |

Magic prefixes are normative and never change without a major version bump.

## 3. Verifier key

### 3.1 Structure

```
envelope(magic=VIKY, ver=1, flags=0,
         payload = <BindingHeader><CommitLLMKeyBlob>)
```

`BindingHeader` (added by this project on top of the CommitLLM key blob):

| Field | Bytes | Notes |
|-------|-------|-------|
| `model_id_len` | 2 | u16 LE |
| `model_id` | `model_id_len` | UTF-8 |
| `checkpoint_hash` | 32 | SHA-256 of the model checkpoint, canonical form per [RFC-0004](../rfcs/RFC-0004-verifier-key-generation.md) |
| `commitllm_pin_len` | 1 | u8 |
| `commitllm_pin` | `commitllm_pin_len` | ASCII commit short SHA |
| `seed` | 8 | u64 LE |
| `keygen_schema_version` | 2 | u16 LE; v1 = `1` |
| `binding_crc32` | 4 | CRC32C of all the above (model_id_len..keygen_schema_version) |

The CommitLLM key blob follows. Its layout is owned upstream.

### 3.2 Determinism

`vi-keygen` MUST produce identical envelope bytes for identical `(model_id, checkpoint_hash, seed, commitllm_pin)`. Inputs other than these MUST NOT influence the output. CI re-runs `vi keygen` against a fixture and compares SHA-256 of the resulting envelope ([FR-17](../../PRD.md)).

### 3.3 Size envelope

- Target: < 10 MB ([NFR-2](../../PRD.md)).
- Measured in CI; a regression beyond 10 MB fails the build.
- If CommitLLM key size for 1B parameters exceeds 10 MB, the spec is updated to the measured value and the README is corrected. No silent envelope inflation.

## 4. Receipt

### 4.1 Structure

```
envelope(magic=VIRC, ver=1, flags=0,
         payload = <ReceiptHeader><CommitLLMReceiptBlob>)
```

`ReceiptHeader`:

| Field | Bytes | Notes |
|-------|-------|-------|
| `key_hash` | 32 | SHA-256 of the `VIKY` envelope this receipt is bound to |
| `model_id_len` | 2 | u16 LE |
| `model_id` | `model_id_len` | UTF-8; must match the key's `model_id` |
| `commitllm_pin_len` | 1 | u8 |
| `commitllm_pin` | `commitllm_pin_len` | ASCII; must match the key's `commitllm_pin` |
| `prompt_hash` | 32 | SHA-256 of the prompt UTF-8 bytes |
| `generated_token_count` | 4 | u32 LE |
| `wall_clock_unix_ms` | 8 | i64 LE; provider's view at request completion |
| `receipt_schema_version` | 2 | u16 LE; v1 = `1` |

The CommitLLM receipt blob follows.

### 4.2 Binding enforcement

The verifier MUST refuse a receipt where any of `key_hash`, `model_id`, or `commitllm_pin` do not match the loaded key. The error category is `IdentityMismatch` and the exit code is `7` ([RFC-0014](../rfcs/RFC-0014-error-taxonomy.md)).

### 4.3 Size envelope

- Target: < 100 KB for a 256-token response ([NFR-3](../../PRD.md)).
- Measured in CI; regression beyond 110 KB (10% margin) fails the build.
- If CommitLLM receipt size for 1B materially exceeds 100 KB, the spec is updated to the measured value and the README is corrected.

### 4.4 Receipt without a body

Future-proofing: a receipt with `generated_token_count = 0` is valid (refusal-to-generate, content-filtered response). The verifier still validates the binding and the empty payload structure. v1 does not depend on this case but does not break on it.

## 5. Audit payload

### 5.1 Structure

```
envelope(magic=VIAU, ver=1, flags=0,
         payload = <AuditHeader><CommitLLMAuditBlob>)
```

`AuditHeader`:

| Field | Bytes | Notes |
|-------|-------|-------|
| `receipt_hash` | 32 | SHA-256 of the `VIRC` envelope this audit responds to |
| `tier` | 1 | u8: `0=receipt-only`, `1=routine`, `2=deep`, `3=full` |
| `token_index` | 4 | u32 LE |
| `layer_count` | 2 | u16 LE |
| `layer_indices` | `layer_count * 2` | u16 LE each |
| `audit_schema_version` | 2 | u16 LE; v1 = `1` |

The CommitLLM audit blob follows.

### 5.2 Binding enforcement

The verifier MUST refuse an audit payload where `receipt_hash` does not match the receipt under verification, or where the requested `(tier, token_index, layer_indices)` does not match what the verifier asked for.

## 6. CLI JSON output schemas

JSON schemas for `vi keygen`, `vi chat`, `vi verify` outputs are defined in [02-public-api.md](./02-public-api.md) §1.2 and bound by `schema_version` fields. The JSON schemas are checked in to `schemas/` as JSON Schema documents; CI validates fixture outputs against them.

## 7. Schema version evolution

- Envelope `ver` byte: bumped on any binary layout change to the envelope itself. Verifier rejects unknown values.
- `keygen_schema_version`, `receipt_schema_version`, `audit_schema_version`, `schema_version` (JSON): bumped on additive changes. Verifier accepts any version it has explicit handling for; unknown values fail closed.

Compatibility matrix is maintained in [09-release-and-versioning.md](./09-release-and-versioning.md).

## 8. Endianness, alignment, encoding

- All integer fields are little-endian unless explicitly noted.
- No padding, no alignment requirements; the format is byte-packed.
- Strings are UTF-8, length-prefixed by the immediately preceding length field. No null terminators.
- SHA-256 is canonical 32-byte big-endian-as-bytes (i.e., the natural output of any standard implementation).
- CRC32C uses the Castagnoli polynomial.
