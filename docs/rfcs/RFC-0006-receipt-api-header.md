# RFC-0006: Receipt API header convention

- Status: Accepted (resolves PRD OQ-3)
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

Receipt opt-in is signaled by the request header `X-Verifiable-Receipt: 1`. When set, the provider returns `multipart/mixed` with the chat-completion JSON and a binary part carrying the receipt envelope. When unset, the response is the standard OpenAI-compatible chat-completion JSON. No query parameter form. No content-negotiation via `Accept`.

## Motivation

The provider exposes an OpenAI-compatible chat-completion endpoint to maximize compatibility with existing clients. The receipt is an extension that must be discoverable without breaking those clients. Three options were on the table per [PRD OQ-3](../../PRD.md):

1. Content negotiation: `Accept: application/json+receipt-v1`.
2. Explicit opt-in header: `X-Verifiable-Receipt: 1`.
3. Query parameter: `?receipt=1`.

This RFC picks (2) and locks it.

## Goals

- Discoverable via documentation, not via guessing.
- Opt-in: existing OpenAI-compatible clients see no change in behavior unless they ask for the receipt.
- Carries no URL-state pollution (no query param affecting telemetry, CDN cache keys, logs).
- Easy to set in any HTTP client.

## Non-Goals

- No version negotiation across header values. The receipt schema version is governed by `commitllm_pin` and the envelope `ver` byte, not by the request header.
- No fine-grained tier opt-in on the chat request. The client always receives "the receipt the provider produced"; tier selection happens at verification time via `vi verify --tier`.

## Proposed Design

### Request

- Header: `X-Verifiable-Receipt: 1`
- Body: standard OpenAI chat-completion JSON.
- Optional: `X-Verifiable-Intelligence-Trace: <ulid>` for client→server correlation.

If `X-Verifiable-Receipt` is set to any value other than `1`, the provider returns `400 Bad Request` with an `input` error envelope. This is strict; any reserved future values get distinct names, not new values of the same header.

### Response (opt-in case)

- Status: `200 OK`.
- Content-Type: `multipart/mixed; boundary=<random>`.
- Parts:
  1. `application/json` — the OpenAI chat-completion body.
  2. `application/vnd.verifiable-intelligence.receipt+binary` — the `VIRC` envelope bytes.

### Response (opt-out case, header absent)

- Status: `200 OK`.
- Content-Type: `application/json`.
- Body: standard OpenAI chat-completion shape.
- Behavior is bit-for-bit compatible with upstream vLLM.

### Errors

- `400` if the header value is invalid.
- `415` is not used; the receipt is provider-initiated, not client-content-typed.
- `503` if the provider cannot emit a receipt (CommitLLM prover unhealthy); the body is the standard chat-completion JSON with no receipt, plus a `Warning` header `Warning: 199 - "Receipt unavailable"`. Clients can decide to retry or proceed receipt-less.

Note: the `503` choice is deliberate so that a CommitLLM-degraded provider still serves text-only clients normally and surfaces the degradation only to receipt-opt-in clients.

### CLI behavior

- `vi chat` sets the header. If the response is single-part JSON without a receipt, the CLI emits `receipt_missing` and exits with code 5.
- `vi chat --no-receipt` does not set the header. Used when the CLI is being repurposed as a thin chat client.

## Alternatives Considered

**Content negotiation via `Accept: application/json+receipt-v1`.** Rejected on three grounds:
- vLLM's request parsing is opinionated and may not honor a custom `Accept`; we would have to patch.
- Content negotiation invites a fallback chain (`q=0.9` etc.) that complicates server logic.
- Most HTTP clients in scripting languages do not surface `Accept` cleanly via convenience helpers; the developer experience is worse.

**Query parameter `?receipt=1`.** Rejected because:
- It pollutes URL telemetry, CDN cache keys, and proxy logs.
- It conflates content semantics with addressability.
- It is easily clobbered by clients that rewrite URLs (analytics middlewares).

**No opt-in; always return a receipt.** Rejected: not OpenAI-compatible; existing clients break.

**Use `Prefer: receipt` (RFC 7240).** Rejected: too clever, low recognition in the developer audience; `X-Verifiable-Receipt` is self-documenting.

## Drawbacks

- `multipart/mixed` parsing is not universal in HTTP client libraries. Mitigation: the CLI implements it; the WASM verifier (v1.1) implements it; third-party integrations can fall back to a "receipt-only" endpoint (future). For v1, `vi chat` is the recommended client.
- The `X-` prefix is deprecated by RFC 6648, but in practice every meaningful header in operations still uses it; the alternative ("`Vi-Receipt`") looks like an internal codename. We accept the deprecation in exchange for legibility.

## Migration / Rollout

- Lands with the first version of the provider entrypoint.
- The CLI's `vi chat` lands paired so end-to-end works on day one.
- The documented `curl` example for receipt opt-in is part of the README's quickstart.

## Testing Strategy

- Integration test: send a request without the header, get a standard JSON response.
- Integration test: send with `X-Verifiable-Receipt: 1`, get a multipart response with both parts; both parts parse.
- Integration test: send with `X-Verifiable-Receipt: 0`, get a 400 with `input` envelope.
- Integration test: simulate a degraded prover, send with the header, receive 503 with the `Warning` header.
- CLI test: `vi chat` correctly detects `receipt_missing` if the server omits the receipt part.

## Open Questions

None.

## References

- [02-public-api.md §2](../spec/02-public-api.md)
- [PRD OQ-3](../../PRD.md)
- RFC 6648 (deprecation of `X-` prefix) — acknowledged, accepted.
- RFC 2046 (`multipart/mixed`).
