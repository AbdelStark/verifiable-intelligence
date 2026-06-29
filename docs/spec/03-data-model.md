# 03 - Data Model

This document specifies the proof bundle and the binary artifacts it wraps. CommitLLM receipt and audit schemas remain upstream-owned at the pinned commit.

## 1. `VIEX` proof bundle

The buyer-facing artifact is JSON:

```json
{
  "magic": "VIEX",
  "schema_version": 1,
  "created_unix_ms": 1782734400000,
  "quote": {},
  "request": {},
  "response": {},
  "receipt": {},
  "verifier": {},
  "audit": {},
  "report": {}
}
```

The normative JSON Schema lives at [`schemas/viex.schema.json`](../../schemas/viex.schema.json). Canonical fixtures live under [`fixtures/viex/`](../../fixtures/viex/).

### 1.1 Quote

| Field | Type | Notes |
|-------|------|-------|
| `quote_id` | string | Broker/provider quote ID |
| `provider_id` | string | Stable provider ID |
| `model_id` | string | Claimed model |
| `checkpoint_hash` | string | `sha256:<hex>` |
| `commitllm_pin` | string | Pinned upstream commit |
| `key_hash` | string | SHA-256 of verifier key envelope |
| `decode_policy_hash` | string | Hash of canonical decode policy JSON |
| `price` | object | Demo price fields |
| `expires_unix_ms` | integer | Quote expiry |
| `signature` | string | Demo signature or provider signature |

### 1.2 Request

| Field | Type | Notes |
|-------|------|-------|
| `request_id` | string | Provider request ID |
| `prompt_hash` | string | SHA-256 of canonical prompt bytes |
| `input_spec_hash` | string | Tokenizer/chat-template/system-prompt policy hash |
| `max_tokens` | integer | Requested generation cap |

Raw prompt text is optional and absent by default in shared bundles.

### 1.3 Response

| Field | Type | Notes |
|-------|------|-------|
| `answer_hash` | string | SHA-256 of delivered answer bytes |
| `answer_preview` | string | Optional truncated display text |
| `generated_token_count` | integer | Provider token count |
| `output_spec_hash` | string | Detokenization/post-processing policy hash |

### 1.4 Receipt

| Field | Type | Notes |
|-------|------|-------|
| `encoding` | string | `base64`, `external_sha256`, or `missing` for a red-path bundle where the provider returned no receipt |
| `content_type` | string | `application/vnd.verifiable-intelligence.receipt+binary` |
| `bytes_b64` | string | Present when embedded |
| `sha256` | string | Always present |
| `size_bytes` | integer | Receipt size |

### 1.5 Verifier

| Field | Type | Notes |
|-------|------|-------|
| `key_hash` | string | Must match quote |
| `key_ref` | string | URL, bundle-relative path, or embedded key marker |
| `commitllm_pin` | string | Must match quote and receipt |
| `verifier_version` | string | Project verifier version |
| `verification_mode` | string | `browser-wasm`, `server`, or `cli` |

### 1.6 Audit

| Field | Type | Notes |
|-------|------|-------|
| `audit_endpoint` | string | Provider audit URL |
| `tier` | string | `receipt-only`, `routine`, `deep`, `full` |
| `challenge` | object | Token/layer challenge |
| `payload_hash` | string | Present after audit opening |

### 1.7 Report

| Field | Type | Notes |
|-------|------|-------|
| `overall` | string | `pass`, `fail`, `unsupported`, `not_run` |
| `checked_at_unix_ms` | integer | Verification time |
| `checks` | array | Ordered check results |
| `warnings` | array | Non-fatal caveats |
| `unsupported` | array | Unsupported requested claims |

Each check result has:

```json
{
  "id": "model_binding",
  "class": "exact",
  "status": "pass",
  "field": "quote.key_hash",
  "detail": "quote key_hash matches receipt key_hash"
}
```

`field` is optional for passing checks and required by convention for failed checks in canonical fixtures. It names the first bundle field that made the check fail. `class` values: `exact`, `algebraic`, `statistical`, `audited`, `open`, `structural`.

## 2. Binary envelopes

The older binary envelopes remain valid inside bundles:

| Artifact | Magic | Purpose |
|----------|-------|---------|
| Verifier key | `VIKY` | Key bound to model/checkpoint/pin |
| Receipt | `VIRC` | CommitLLM receipt envelope |
| Audit payload | `VIAU` | CommitLLM audit opening |

## 3. Binding rules

The verifier must fail if any of these disagree:

- quote `model_id` vs receipt model ID,
- quote `checkpoint_hash` vs verifier key checkpoint hash,
- quote `key_hash` vs verifier key hash,
- quote `commitllm_pin` vs verifier and receipt pin,
- request `prompt_hash` vs receipt prompt hash,
- response `answer_hash` vs CommitLLM delivered-answer binding,
- audit payload hash vs verifier challenge.

## 4. Privacy default

Shared bundles omit raw prompt and raw answer by default. The local UI may hold them in memory to compute hashes and render the response. Deep audit openings may reveal trace data; the UI must label that before export.
