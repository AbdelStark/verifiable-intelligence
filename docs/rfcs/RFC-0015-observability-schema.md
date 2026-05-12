# RFC-0015: Observability schema

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

The CLI emits structured JSON log lines via `tracing` with a stable event-name namespace and a documented redaction layer. The provider container emits a small set of CommitLLM-adjacent log lines on top of vLLM's. A `trace_id` (ULID) generated per CLI invocation flows through logs, the error envelope, and (optionally) request headers to enable client→server correlation. No backends are integrated in v1; this is a developer-debugging tool, not an SRE platform.

## Motivation

[05-observability.md](../spec/05-observability.md) describes the user-facing behavior. This RFC fixes the implementation contract: subscriber configuration, event naming, redaction enforcement, and provider-side event hygiene. The redaction layer in particular is a correctness-critical surface that must not regress.

## Goals

- One subscriber, one place where formatting happens.
- Event names are stable; adding events is non-breaking.
- Redaction is enforced at the subscriber level, not at call sites.
- Default verbosity is "silent on success, JSON on error"; opt-in verbosity is documented.
- Provider-side logs are sufficient to correlate with a client trace.

## Non-Goals

- No OTLP, no Jaeger, no Tempo, no Loki, no Honeycomb in v1.
- No structured metric endpoint on the CLI. The provider passes through vLLM metrics; we add nothing.
- No log aggregation tooling. Operators of self-hosted provider deployments aggregate however they want.

## Proposed Design

### Subscriber

```rust
// vi-log/src/lib.rs (sketch)
pub fn init(subcommand: &str, trace_id: &str) {
    let layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(false)
        .with_current_span(false)
        .with_span_list(false)
        .event_format(VIEventFormatter::new(subcommand, trace_id))
        .with_filter(EnvFilter::from_env("VI_LOG").or(EnvFilter::new("error")));

    let redactor = RedactionLayer::default();

    tracing_subscriber::registry()
        .with(redactor)
        .with(layer)
        .init();
}
```

- `RedactionLayer` is a `tracing` layer that intercepts events and rewrites field values per the rules below. Tests verify that values that would be redacted at INFO never appear in INFO output regardless of how the call site spelled them.

### Default verbosity

- `VI_LOG` unset and `RUST_LOG` unset: filter at `ERROR`. The only log line on a successful run is the optional `process.end` at INFO (which is suppressed by the filter). Net effect: silent on success.
- `--log` or `VI_LOG=info`: INFO and above. Phase boundaries appear at DEBUG, so they are still hidden.
- `VI_LOG=debug`: all phase boundaries, all network events.
- `VI_LOG=trace`: byte-level prefixes for diagnostic purposes.

### Event naming

Dotted namespace. Stable identifiers. The full list is in [05-observability.md](../spec/05-observability.md) §"Events the project owns". New events go in that table first; RFC review is not required for additions.

### Redaction rules (enforced)

The `RedactionLayer` knows the following field-name → handling map at INFO and below:

| Field | INFO/DEBUG | TRACE |
|-------|------------|-------|
| `prompt` | dropped | dropped |
| `prompt_hash` | kept | kept |
| `generated_text` | dropped | dropped |
| `text_chars` | kept | kept |
| `key_bytes` | dropped | dropped |
| `receipt_bytes` | dropped | first 32 bytes hex prefix |
| `audit_bytes` | dropped | first 32 bytes hex prefix |
| `authorization`, `cookie`, `set-cookie` | dropped | dropped |
| `api_key`, `VI_API_KEY` | dropped | dropped |
| `args` | redacted (api key removed) | redacted |

The map is exhaustive: any field not in it passes through. The map is sourced from a single `const` table for auditability.

The redactor is unit-tested against deliberate misuse (someone tries to log `prompt = ...` at INFO; the prompt must not appear in the output).

### Trace ID

- ULID generated at process start.
- Stored in a `Once` and made available as a `tracing` field on every event.
- Surfaced in the error envelope ([RFC-0014](./RFC-0014-error-taxonomy.md)).
- Sent on the wire as `X-Verifiable-Intelligence-Trace: <ulid>` for chat and audit requests.

The provider echoes the header into its log lines for chat and audit requests, so a developer can grep their CI logs for a `trace_id` and find the corresponding server-side request in the provider's logs.

### Provider-side events

The provider entrypoint adds three log lines beyond vLLM's:

- `provider.boot` once, with `commitllm_pin`, `model_id`, `checkpoint_hash`.
- `provider.ready` once, when `/healthz` first returns green.
- `provider.audit` per audit request, with `request_id`, `trace_id` (if header present), `tier`, `token_index`, `layer_count`, `duration_ms`.

vLLM's own logs are not modified. The provider's logging format is structured JSON on stderr at INFO by default; format and level controlled by env vars.

### Span model

- One span per subcommand invocation, named `cli.<subcommand>`.
- Sub-spans for `verify.phase.<name>` during verification.
- No spans for trivial sync calls.

Spans add ~µs of overhead and let DEBUG-level output show parent/child relationships.

### Performance

The default-silent path is dominated by the EnvFilter check at event emission. Measured cost on the reference laptop: < 1 µs per suppressed event. At INFO over a single verify run: < 1 ms total ([08-performance-budget.md](../spec/08-performance-budget.md)).

## Alternatives Considered

**`log` crate + `env_logger`.** Rejected: `tracing` gives us spans, layered subscribers, and structured fields, which the redaction layer needs.

**Custom JSON emitter.** Rejected: `tracing_subscriber::fmt::layer().json()` is good enough; we add the redactor on top, not under.

**Always-on file logging to `~/.verifiable-intelligence/logs/`.** Rejected: PII leakage risk; surprises users; doesn't fit a CLI ethos.

**Use OpenTelemetry exporter from day one.** Rejected: adds a dependency without a v1 consumer. The schema is OTLP-compatible enough that adding an exporter in v1.x is mechanical.

## Drawbacks

- A redaction-layer bug could leak content. Mitigation: enforced unit tests; the field map is small and auditable.
- ULIDs are 26 characters and visually noisy in `--pretty` output. We accept this; the trace_id is operational, not human-aesthetic.

## Migration / Rollout

- `vi-log` lands with the workspace bootstrap.
- Provider-side events land with the entrypoint script.
- Redaction tests are part of `vi-log`'s test suite and gate any change to the field map.

## Testing Strategy

- Redaction unit tests: each forbidden field tried at each verbosity; assert it does not appear in emitted output.
- Snapshot tests for selected event shapes (e.g., `verify.end`).
- "Log is silent on success" CI test: `vi verify` on a passing fixture emits no stderr at default verbosity.
- "Log carries trace_id" test: at INFO, every event includes the trace_id.
- Provider entrypoint emits the three documented lines in a smoke test.

## Open Questions

None.

## References

- [05-observability.md](../spec/05-observability.md)
- [RFC-0014](./RFC-0014-error-taxonomy.md)
- `tracing` crate documentation.
