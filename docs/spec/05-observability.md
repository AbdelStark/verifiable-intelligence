# 05 - Observability

The pivot adds browser, broker, and proof-bundle events to the original provider and verifier logs. Logs are for debugging and reproducibility. They are not proof material.

## Principles

- Raw prompts and raw answers are not logged by default.
- Prompt hashes, answer hashes, quote IDs, request IDs, key hashes, and receipt hashes are allowed.
- Browser demo telemetry is local-only unless a hosted demo explicitly adds analytics.
- Broker logs must be enough to correlate a quote, provider request, receipt, and verification report.

## Event namespaces

| Event | Emitter | Fields |
|-------|---------|--------|
| `demo.provider.selected` | browser | `provider_id`, `model_id`, `key_hash` |
| `demo.quote.created` | browser/broker | `quote_id`, `provider_id`, `model_id`, `expires_unix_ms` |
| `demo.prompt.submitted` | browser | `quote_id`, `prompt_hash`, `max_tokens` |
| `demo.response.received` | browser/broker | `request_id`, `answer_hash`, `receipt_hash`, `elapsed_ms` |
| `demo.verify.started` | browser/verifier | `bundle_hash`, `tier`, `verification_mode` |
| `demo.verify.check` | browser/verifier | `check_id`, `class`, `status`, `elapsed_ms` |
| `demo.verify.finished` | browser/verifier | `overall`, `elapsed_ms`, `warning_count` |
| `broker.provider.forward` | broker | `quote_id`, `provider_id`, `request_id`, `trace_id` |
| `provider.boot` | provider | `commitllm_pin`, `model_id`, `checkpoint_hash`, `key_hash` |
| `provider.audit` | provider | `request_id`, `tier`, `token_index`, `layer_count`, `duration_ms` |

## Redaction

| Field | Default | Debug |
|-------|---------|-------|
| `prompt` | dropped | dropped |
| `answer` | dropped | dropped |
| `prompt_hash` | kept | kept |
| `answer_hash` | kept | kept |
| `api_key` | dropped | dropped |
| `receipt_bytes` | dropped | prefix only |
| `bundle_json` | dropped | local-only test fixtures |

## Trace correlation

The browser creates a `trace_id` per verification run. Broker and provider propagate it with `X-Verifiable-Intelligence-Trace`. The trace ID is not a security signal; it only helps connect logs.

## Metrics

Minimum local metrics:

- static demo render time,
- quote-to-response elapsed time,
- verification elapsed time,
- proof bundle byte size,
- receipt byte size,
- first failing check ID.

Hosted demo metrics must not include raw prompt or answer text.
