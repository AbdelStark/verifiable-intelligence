# 02 - Public API

This document defines the public surfaces after the marketplace-demo pivot: browser artifact, proof bundle, optional broker API, provider API, and utility CLI.

## 1. Browser app

- Path: `demo/index.html`
- Build: none for the prototype.
- Required states: honest provider, fake model, prompt mismatch, answer rewrite, receipt tamper, unsupported closed-weight model.
- Required export: proof bundle JSON download/copy.

The app must not ask for third-party API keys. Live hosted mode may use a project-owned provider endpoint with rate limits.

## 2. Proof bundle

Artifact: `application/vnd.verifiable-intelligence.exchange+json`

The bundle starts with:

```json
{
  "magic": "VIEX",
  "schema_version": 1,
  "quote": {},
  "request": {},
  "response": {},
  "receipt": {},
  "verifier": {},
  "audit": {},
  "report": {}
}
```

The normative field list is in [03-data-model.md](./03-data-model.md).

## 3. Broker API (optional)

The static demo may simulate these endpoints. A live broker must use the same shapes.

The broker is an ordering and convenience layer, not a trust root. It must not accept buyer-supplied third-party API keys, credential fields, or credential headers. Quote signatures are useful for replay and UI integrity, but proof validity comes from verifier checks over the bundle fields and CommitLLM receipt bindings.

### `GET /providers`

Returns authorized demo providers:

```json
{
  "providers": [
    {
      "provider_id": "lab-a100-01",
      "display_name": "Lab A100 01",
      "model_id": "llama-3.1-8b-w8a8",
      "checkpoint_hash": "sha256:...",
      "key_hash": "sha256:...",
      "commitllm_pin": "25541e83",
      "proof_modes": ["routine", "deep"],
      "price_per_1k_tokens_usd": "0.012"
    }
  ]
}
```

### `POST /quotes`

Request:

```json
{
  "provider_id": "lab-a100-01",
  "model_id": "llama-3.1-8b-w8a8",
  "max_tokens": 256,
  "decode_policy": {
    "temperature": 0.2,
    "top_p": 0.95
  }
}
```

Response:

```json
{
  "quote_id": "qt_...",
  "provider_id": "lab-a100-01",
  "model_id": "llama-3.1-8b-w8a8",
  "checkpoint_hash": "sha256:...",
  "key_hash": "sha256:...",
  "commitllm_pin": "25541e83",
  "decode_policy": {
    "temperature": 0.2,
    "top_p": 0.95,
    "max_tokens": 256
  },
  "decode_policy_hash": "sha256:...",
  "expires_unix_ms": 1782735000000,
  "estimated_price_usd": "0.0048",
  "signature": "demo:..."
}
```

Requests containing fields or headers such as `api_key`, `x-api-key`, `access_token`, `authorization`, `credentials`, `password`, or `secret` must be rejected before a quote is created.

### `POST /chat`

Request:

```json
{
  "quote_id": "qt_...",
  "prompt": "What causes rainbows?"
}
```

Response:

```json
{
  "quote_id": "qt_...",
  "request_id": "req_...",
  "text": "...",
  "proof_bundle": {}
}
```

The initial proof bundle may carry `report.overall = "not_run"` when the browser verifier is unavailable. The bundle still must include quote, request, response, receipt, verifier, audit, and report sections that validate against the VIEX schema.

### `POST /verify`

Prototype fallback only. A production browser-first flow should verify locally when feasible.

Request:

```json
{
  "proof_bundle": {}
}
```

Response:

```json
{
  "report": {}
}
```

The fallback verifier must fail closed on model, checkpoint, CommitLLM pin, verifier key, decode policy, prompt hash, answer hash, receipt hash, and quote-expiry mismatches. A broker quote signature alone is not sufficient evidence.

## 4. Provider API

The provider API inherits the old receipt surface:

- `POST /v1/chat/completions`
- request header `X-Verifiable-Receipt: 1`
- `POST /v1/audit`
- `GET /healthz`

`/healthz` must include:

```json
{
  "status": "ok",
  "model_id": "llama-3.1-8b-w8a8",
  "checkpoint_hash": "sha256:...",
  "commitllm_pin": "25541e83",
  "key_hash": "sha256:..."
}
```

## 5. Utility CLI

The old `vi` CLI remains an implementation utility:

- `vi keygen`
- `vi verify`
- `vi receipt inspect`
- `vi bundle inspect` (new)
- `vi bundle verify` (new)

`vi tui` is deferred. CLI JSON discipline and exit codes from the old spec still apply where the CLI exists.
