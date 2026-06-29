# Provider Integration Guide

This guide describes the minimum provider behavior for the v1 marketplace proof demo.

## Provider Eligibility

An authorized provider must operate an open-weight model it is allowed to serve. v1 does not support closed-weight frontier models unless the model owner publishes compatible verifier material or a signed attestation that this project can verify independently.

The current v1 reference target is:

| Field | Value |
|-------|-------|
| Model ID | `llama-3.1-8b-w8a8` |
| CommitLLM profile | `llama-w8a8-audited` |
| CommitLLM pin | `25541e83` in buyer-facing short form; full pinned commit where artifacts require it |
| First live hosting target | A100-class GPU host |

## Required Provider Metadata

Expose stable metadata through the provider catalog, quote response, and `/healthz`:

| Field | Purpose |
|-------|---------|
| `provider_id` | Stable provider identity shown to the buyer. |
| `model_id` | Human-readable model identity. |
| `checkpoint_hash` | Hash of the exact checkpoint snapshot used by the CommitLLM prover path. |
| `key_hash` | Hash of the verifier key buyers use. |
| `commitllm_pin` | Exact CommitLLM commit used by prover and verifier. |
| `proof_modes` | Truthful list of supported proof modes such as `routine` or `deep`. |
| `price_per_1k_tokens_usd` | Demo quote pricing. |

Do not advertise unsupported proof modes. If receipts or audit responses are unavailable, the provider must render as `fail` or `unsupported`, not as a degraded pass.

## Request Flow

1. The broker or app requests a short-lived quote.
2. The provider quote binds model identity, checkpoint hash, key hash, CommitLLM pin, decode policy hash, price, expiry, and signature.
3. The buyer submits a prompt under that quote.
4. The provider returns the delivered answer plus a CommitLLM receipt when `X-Verifiable-Receipt: 1` is requested.
5. The app assembles a `VIEX` proof bundle.
6. Browser verification checks the quote, prompt hash, decode policy, delivered answer hash, receipt bytes, verifier key, CommitLLM pin, and supported CommitLLM report.

## Provider API Surface

The provider should keep the OpenAI-compatible chat shape and add verifiable-inference behavior:

- `POST /v1/chat/completions`
- request header `X-Verifiable-Receipt: 1`
- response body or multipart part containing the answer and CommitLLM receipt
- `POST /v1/audit` for challenge responses when the selected proof mode needs an audit endpoint
- `GET /healthz` with `model_id`, `checkpoint_hash`, `commitllm_pin`, and `key_hash`

The provider must not ask buyers for third-party API keys. The hosted demo must use project-owned credentials only, with rate limits.

## Lawful-Use Boundary

The integration is for authorized open-weight providers and research fixtures.

Providers must not use this project to:

- pool or resell third-party API credentials,
- bypass provider terms, billing controls, rate limits, identity checks, or access controls,
- claim proof support for closed-weight models without compatible public verifier material,
- represent fixture simulation as live CommitLLM verification,
- claim answer correctness, safety, or endorsement from a passing receipt.

## Operational Checklist

- Pin CommitLLM by full commit SHA.
- Publish verifier key material and key hash.
- Pin model weights by immutable snapshot hash.
- Keep `commitllm_pin` consistent across `/healthz`, quotes, `VIEX` bundles, and verifier packages.
- Return explicit failures for unsupported proof modes.
- Log prompt hashes and answer hashes by default, not raw prompt or raw answer text.
- Rate-limit chat and audit endpoints.
- Keep server-side verification labeled as a fallback when browser-WASM verification is not being used.
