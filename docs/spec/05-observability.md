# 05 — Observability

The CLI and TUI emit structured logs and timing data. The provider container emits standard vLLM logs plus a small set of CommitLLM-specific events. This document defines the log schema, the events the project owns, redaction rules, and the user-facing controls. Implementation detail in [RFC-0015](../rfcs/RFC-0015-observability-schema.md).

## Goals

- A user can reproduce a verification failure by reading logs alone.
- An operator of the provider can correlate a client-side `trace_id` with a server-side request.
- Sensitive content (prompts, generated text, key bytes) never appears in logs by default.
- Logs are machine-parseable JSON; humans use `--pretty` or a `jq` filter.

## Non-goals

- No tracing backend integration in v1 (no OTLP exporter, no Jaeger, no Tempo).
- No metrics endpoint on the client. The provider exposes vLLM's metrics as-is; we do not add to them.
- No correlation across multiple `vi` invocations beyond what the user does themselves with `trace_id`.

## Client log model

The `tracing` crate is the implementation. Output format is JSON-per-line on stderr. The envelope:

```json
{
  "ts": "2026-05-12T10:15:42.123Z",
  "level": "INFO",
  "trace_id": "01J...",
  "subcommand": "verify",
  "event": "phase.started",
  "fields": { "phase": "bridge_replay" }
}
```

- `ts`: ISO-8601 UTC with millisecond precision.
- `level`: `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`.
- `trace_id`: ULID, set once at process start, carried in the error envelope.
- `subcommand`: one of `keygen`, `chat`, `verify`, `tui`.
- `event`: dotted-namespace event name, stable.
- `fields`: event-specific structured fields. Schema per event.

## Default verbosity

- Default: silent on stderr unless an error occurs.
- `--log` flag or `RUST_LOG=verifiable_intelligence=info`: INFO and above.
- `RUST_LOG=verifiable_intelligence=debug`: full phase-boundary trace.
- `RUST_LOG=verifiable_intelligence=trace`: byte-level decode diagnostics.

`--log` and `RUST_LOG` together: the most verbose wins.

## Events the project owns

| Event | Level | When | Fields |
|-------|-------|------|--------|
| `process.start` | INFO | First action of every process | `trace_id`, `subcommand`, `version`, `args` (redacted) |
| `process.end` | INFO | Last action; emitted from a drop guard | `trace_id`, `exit_code`, `duration_ms` |
| `keygen.fetch.start` | INFO | Begin downloading checkpoint | `model_id`, `source` |
| `keygen.fetch.end` | INFO | Finish download | `bytes`, `duration_ms` |
| `keygen.hash` | DEBUG | Compute checkpoint hash | `checkpoint_hash` |
| `keygen.emit` | INFO | Write key | `key_path`, `key_size_bytes`, `key_hash` |
| `chat.request` | INFO | Send chat request | `endpoint`, `model_id`, `max_tokens`, `prompt_hash` (always; never the prompt) |
| `chat.response` | INFO | Response received | `http_status`, `text_chars`, `receipt_size_bytes`, `duration_ms` |
| `verify.start` | INFO | Begin verification | `tier`, `receipt_path`, `key_path` |
| `verify.phase.start` | DEBUG | Phase begins | `phase` |
| `verify.phase.end` | DEBUG | Phase ends | `phase`, `passed`, `measured?`, `tolerance?`, `duration_ms` |
| `verify.end` | INFO | Verification complete | `overall`, `phases_passed`, `phases_failed`, `duration_ms` |
| `audit.request` | INFO | Request audit payload | `endpoint`, `tier`, `token_index`, `layer_count` |
| `audit.response` | INFO | Audit payload received | `bytes`, `duration_ms` |
| `tui.tamper.applied` | INFO | TUI applied a tamper | `kind`, `offset?` |
| `error` | ERROR | Any error reaching the boundary | `category`, `detail` |

Event names are stable. Adding events is non-breaking. Renaming or removing is breaking.

## Redaction rules

- **Never logged at default verbosity:** prompt text, generated text, key bytes, receipt bytes, audit payload bytes, raw HTTP headers other than the receipt opt-in marker.
- **Always logged:** prompt hash, response character count, byte counts, durations, phase identifiers.
- **DEBUG and TRACE may log additional fields**, but never raw key material or full prompts. `TRACE` may log the first 32 bytes of a receipt as a hex prefix for offset analysis; never more.

The redaction layer is implemented as a `tracing` subscriber middleware so that bypassing it requires touching the subscriber, not an `info!` call site. Unit-tested per [07-testing-strategy.md](./07-testing-strategy.md).

## Provider-side observability

The provider container emits two log streams:

1. **vLLM stdout/stderr** — verbatim from upstream. Not normalized.
2. **CommitLLM events** — the upstream prover emits its own events at known levels. We do not wrap or reformat.

The provider entrypoint adds three CommitLLM-adjacent log lines:

- `provider.boot` with `commitllm_pin`, `model_id`, `checkpoint_hash`.
- `provider.ready` once `/healthz` is green.
- `provider.audit` once per audit request: `request_id`, `tier`, `token_index`, `layer_count`, `duration_ms`.

These are sufficient to correlate a client trace with a server log.

## Trace correlation

The `vi` CLI sends `X-Verifiable-Intelligence-Trace: <trace_id>` on chat and audit requests. The provider entrypoint echoes it in `provider.audit` log lines and (when available) in `/v1/audit` response headers. Optional; not required for protocol correctness; not used as a security signal.

## Performance impact

- Default-silent emission cost is dominated by error-path allocation; benchmarks show negligible overhead on hot paths.
- `INFO` level on a verify run costs less than 1 ms total on a 2023-class CPU (measured per NFR-1).
- `TRACE` is not budgeted for; it's a debugging mode.

## Auditability

Logs are a debugging tool, not a security primitive. A failed verification's authoritative output is the error envelope on stderr and the JSON report on stdout; logs add context but do not change the outcome.
