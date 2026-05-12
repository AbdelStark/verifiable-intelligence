# 01 — Architecture

## System view

```
┌───────────────────────────────────────────┐         ┌──────────────────────────────────────────┐
│ Client (developer laptop, CPU only)       │         │ Provider (GPU, HF Endpoint or self-host)  │
│                                           │         │                                           │
│  ┌──────────┐    ┌──────────┐    ┌──────┐ │  HTTPS  │  ┌────────────────────────────────────┐  │
│  │ vi chat  │───▶│ vi verify│───▶│  TUI │ │◀────────│  │ vLLM + CommitLLM prover            │  │
│  │ (sends   │    │ (CPU     │    │ (live│ │         │  │   - chat completion                │  │
│  │  prompt) │    │  verify) │    │ walk)│ │         │  │   - emits CommitLLM receipt        │  │
│  └──────────┘    └────┬─────┘    └──────┘ │         │  │   - serves audit endpoint          │  │
│                       │                   │         │  │   - W8A8 Llama 3.2 1B Instruct     │  │
│                  ┌────▼─────┐             │         │  └────────────────────────────────────┘  │
│                  │ verifier │             │         │                                          │
│                  │ key.bin  │             │         │  ┌───────────────────────────────────┐   │
│                  │ (<10 MB) │             │         │  │ Docker image (single Dockerfile)  │   │
│                  └──────────┘             │         │  │   - reproducible, vendor-neutral  │   │
└───────────────────────────────────────────┘         │  └───────────────────────────────────┘   │
                                                       └──────────────────────────────────────────┘
```

The protocol participants are: a **prover** running on the GPU, embedded in the provider image; a **verifier** running on the client CPU, embedded in the CLI and TUI; and a **shared verifier key** generated deterministically from the model checkpoint hash.

## Components

### Client-side

| Component | Crate (proposed) | Responsibility |
|-----------|------------------|----------------|
| `vi` CLI binary | `vi-cli` | Entry point, subcommand dispatch, CLI argument parsing, JSON output assembly |
| Verifier core | `vi-verifier` | Wraps CommitLLM verifier crate; turns CommitLLM raw report into our structured report shape |
| HTTP client | `vi-client` | Talks to the provider endpoint, sets the receipt opt-in header, parses streaming responses, retrieves audit payloads |
| TUI shell | `vi-tui` | `ratatui`-based terminal UI; drives `vi-client` and `vi-verifier`; renders phase walk |
| Keygen | `vi-keygen` | Fetches or accepts a model checkpoint, computes the deterministic verifier key, writes binary key artifact |
| Receipt codec | `vi-receipt` | Encode/decode CommitLLM receipt envelope; magic prefix; version handshake |
| Logging facade | `vi-log` | Structured JSON logging via `tracing`; redaction rules ([RFC-0015](../rfcs/RFC-0015-observability-schema.md)) |

Crate boundaries are normative and detailed in [RFC-0001](../rfcs/RFC-0001-workspace-and-crate-layout.md).

### Provider-side

| Component | Layer | Responsibility |
|-----------|-------|----------------|
| vLLM serving | container | Standard vLLM 0.x with CommitLLM patches applied at the pinned commit |
| CommitLLM prover | upstream lib | Emits the receipt envelope alongside the generated text; serves the audit endpoint |
| Entrypoint script | container | Boots vLLM with the pinned checkpoint and serving config; surfaces health/readiness on a documented path |
| Docker image | infra | Single Dockerfile; multi-stage; final image under 8 GB; CUDA 12.x base ([RFC-0005](../rfcs/RFC-0005-provider-image.md)) |
| HF Endpoint deployment script | infra | `hf` CLI driven; build → push → endpoint create → URL ([RFC-0007](../rfcs/RFC-0007-hf-deployment-recipe.md)) |
| `docker compose` recipe | infra | Self-hosted single-GPU bring-up |

The provider image does not include any Modal-specific code in the critical path. Modal compatibility is preserved by virtue of running a standard Docker image; it is not validated in CI ([PRD C3](../../PRD.md)).

### Cross-cutting

| Concern | Where it lives |
|---------|----------------|
| Error taxonomy | [04-error-model.md](./04-error-model.md), implemented via `vi-errors` shared crate ([RFC-0014](../rfcs/RFC-0014-error-taxonomy.md)) |
| Receipt schema | [03-data-model.md](./03-data-model.md), implemented in `vi-receipt` |
| CLI/HTTP contract | [02-public-api.md](./02-public-api.md) |
| Observability | [05-observability.md](./05-observability.md), `vi-log` crate ([RFC-0015](../rfcs/RFC-0015-observability-schema.md)) |
| Security | [06-security.md](./06-security.md) |

## Data flow: a single verified response

1. The integrating developer has run `vi keygen` previously and has `key.bin` and a record of the model checkpoint hash it was bound to. `vi keygen` is deterministic; running it again on the same input yields the same bytes ([FR-17](../../PRD.md)).
2. The developer runs `vi chat --endpoint <url> --prompt "<text>"`. `vi-cli` parses, dispatches to `vi-client`.
3. `vi-client` issues an HTTPS POST to the chat-completion path with the `X-Verifiable-Receipt: 1` header ([RFC-0006](../rfcs/RFC-0006-receipt-api-header.md)).
4. The provider's vLLM stack runs inference with the CommitLLM prover instrumenting decode. The response carries the generated text and a receipt envelope (`docs/spec/03-data-model.md`).
5. `vi-client` returns text and writes the receipt to stdout or a file per CLI flags.
6. The developer runs `vi verify --receipt receipt.bin --key key.bin --tier full`.
7. `vi-verifier` checks magic, version, model identity binding against the key, and dispatches into the CommitLLM verifier crate for the requested tier.
8. For `full` tier on a single token, the verifier may issue an audit challenge: `vi-client` POSTs the audit specification to the provider's audit endpoint, retrieves the audit payload, and `vi-verifier` consumes it.
9. The verifier emits a structured JSON report ([02-public-api.md](./02-public-api.md)). Exit code maps to the report ([RFC-0014](../rfcs/RFC-0014-error-taxonomy.md)).

The TUI is the same flow with phase boundaries rendered as they happen.

## Module boundaries (normative)

- `vi-cli` depends on `vi-client`, `vi-verifier`, `vi-keygen`, `vi-receipt`, `vi-log`, `vi-errors`. No other crate may depend on `vi-cli`.
- `vi-tui` depends on the same set as `vi-cli` plus `ratatui`. `vi-cli` and `vi-tui` may not depend on each other.
- `vi-verifier` depends on the CommitLLM verifier crate (pinned, see [RFC-0011](../rfcs/RFC-0011-commitllm-upstream-pinning.md)) and `vi-errors`. It may not depend on `vi-client` or any networking code.
- `vi-receipt` is leaf: depends only on the pinned CommitLLM receipt schema crate and `vi-errors`. It must not require an async runtime.
- `vi-keygen` may pull weights (network) and hash them. It depends on `vi-receipt` for binding fields only.

Violations of these boundaries are CI-failing.

## Determinism guarantees

- `vi keygen` is byte-deterministic in `(model_checkpoint_hash, params...)`. Tested in CI on every PR ([FR-17](../../PRD.md)).
- The verifier is byte-deterministic in `(receipt, key, tier, audit_payload?)`. Re-running yields the same report. Tested as a fixture-based regression in CI.
- Receipt encoding/decoding round-trips byte-for-byte. Tested in `vi-receipt` unit tests.

The provider is not deterministic in the temperature-free decode sense at v1; CommitLLM's prover commits to whatever the underlying decode produced. Reproducibility of generation is not a v1 requirement.

## Concurrency model

- `vi-cli` and `vi-tui` use `tokio` for I/O.
- `vi-verifier` is synchronous. Verification is CPU-bound and fast; spinning a runtime for it is wrong.
- Each CLI subcommand is a single-shot invocation. There is no daemon, no IPC, no shared cache that survives process exit.

## Failure-domain isolation

- A network error in `vi-client` never produces a "verification failed" report. It produces a transport-error exit code ([RFC-0014](../rfcs/RFC-0014-error-taxonomy.md)).
- A receipt parse error never produces a "verification passed" report. It produces a malformed-receipt exit code.
- Unknown receipt versions, unknown model identities, and unsupported tier requests all fail closed ([NFR-6](../../PRD.md)).

## Out of scope at the architecture layer

- No protocol-level extension is in scope; that lives upstream in CommitLLM.
- No alternative verifiers (ZKP, MPC) are in scope; see PRD NG5.
- No caching of audit payloads between runs; receipts are single-shot artifacts.
- No background services on the client.
