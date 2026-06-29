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

### `GET /providers`

Returns authorized demo providers:

```json
{
  "providers": [
    {
      "provider_id": "lab-l4-01",
      "display_name": "Lab L4 01",
      "model_id": "llama-3.1-8b-w8a8",
      "checkpoint_hash": "sha256:...",
      "key_hash": "sha256:...",
      "commitllm_pin": "abcdef0",
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
  "provider_id": "lab-l4-01",
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
  "provider_id": "lab-l4-01",
  "model_id": "llama-3.1-8b-w8a8",
  "checkpoint_hash": "sha256:...",
  "key_hash": "sha256:...",
  "commitllm_pin": "abcdef0",
  "expires_unix_ms": 1782735000000,
  "estimated_price_usd": "0.0048",
  "signature": "demo:..."
}
```

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

### `POST /verify`

Prototype fallback only. A production browser-first flow should verify locally when feasible.

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
  "commitllm_pin": "abcdef0",
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
