# verifiable-intelligence: Specification

- Status: Draft
- Authors: AbdelStark
- Created: 2026-05-12
- Source of intent: [`PRD.md`](./PRD.md)

This document is the canonical entry point to the project specification corpus. It indexes the per-area specs in [`docs/spec/`](./docs/spec/) and the RFCs in [`docs/rfcs/`](./docs/rfcs/). The detail lives in those files. This page is short by design.

## Thesis

`verifiable-intelligence` is a reference application that turns the CommitLLM commit-and-audit protocol into a shippable developer experience. The protocol is not ours. The deployment recipe, the CLI surface, the TUI demonstration surface, the corridor measurement on a smaller-than-upstream-validated model, and the integration story are. v1 targets two end-user surfaces (Rust CLI, Rust TUI) on Llama 3.2 1B Instruct W8A8 served from a Hugging Face Inference Endpoint via a vendor-neutral Docker image.

## Scope

v1 (this corpus) ships:

- A Rust CLI `vi` with subcommands `keygen`, `chat`, `verify`, `tui`.
- A Rust TUI for interactive verification walkthroughs with tamper demonstration.
- A provider-side Docker image running CommitLLM-instrumented vLLM on Llama 3.2 1B Instruct W8A8.
- An HF Inference Endpoints deployment recipe and a self-hosted `docker compose` recipe.
- A reproducible corridor measurement on Llama 3.2 1B Instruct W8A8 across three workloads.
- An honest README and per-surface preamble naming the protocol's bounded properties.
- A tamper-detection fuzz harness in CI on every PR.

v1.1 (browser WASM verifier) and v1.2 (batched compliance flow) are scoped in [PRD §12](./PRD.md) and tracked as separate milestones. They do not block v1 release.

## Spec corpus

| File | Purpose |
|------|---------|
| [`docs/spec/00-overview.md`](./docs/spec/00-overview.md) | Thesis, goals, non-goals, success criteria, scope boundaries |
| [`docs/spec/01-architecture.md`](./docs/spec/01-architecture.md) | System architecture, components, data flow, module boundaries |
| [`docs/spec/02-public-api.md`](./docs/spec/02-public-api.md) | Public CLI surface, HTTP API surface, versioning policy |
| [`docs/spec/03-data-model.md`](./docs/spec/03-data-model.md) | Receipt, verifier key, audit payload, schema versioning |
| [`docs/spec/04-error-model.md`](./docs/spec/04-error-model.md) | Error taxonomy, exit codes, failure modes, recovery |
| [`docs/spec/05-observability.md`](./docs/spec/05-observability.md) | Structured logging, metrics, tracing, redaction rules |
| [`docs/spec/06-security.md`](./docs/spec/06-security.md) | Threat model, trust boundaries, secrets handling, abuse |
| [`docs/spec/07-testing-strategy.md`](./docs/spec/07-testing-strategy.md) | Unit, property, integration, tamper fuzz, corridor, comprehension |
| [`docs/spec/08-performance-budget.md`](./docs/spec/08-performance-budget.md) | Latency, size, memory targets; measurement methodology |
| [`docs/spec/09-release-and-versioning.md`](./docs/spec/09-release-and-versioning.md) | Semver, deprecation policy, changelog discipline |
| [`docs/spec/10-glossary.md`](./docs/spec/10-glossary.md) | Canonical terms |

## RFC index

| RFC | Title | Status | Locks |
|-----|-------|--------|-------|
| [RFC-0001](./docs/rfcs/RFC-0001-workspace-and-crate-layout.md) | Workspace and crate layout | Accepted | Cargo workspace, crate boundaries, MSRV |
| [RFC-0002](./docs/rfcs/RFC-0002-cli-surface.md) | `vi` CLI surface | Accepted | Subcommand shape, flags, JSON output schema |
| [RFC-0003](./docs/rfcs/RFC-0003-receipt-format-pinning.md) | Receipt format pinning and version handling | Accepted | Pinned CommitLLM commit, receipt magic, version policy |
| [RFC-0004](./docs/rfcs/RFC-0004-verifier-key-generation.md) | Verifier key generation and binding | Accepted | `vi keygen` contract, determinism, binding fields |
| [RFC-0005](./docs/rfcs/RFC-0005-provider-image.md) | Provider Docker image architecture | Accepted | Image layout, entrypoint, env contract |
| [RFC-0006](./docs/rfcs/RFC-0006-receipt-api-header.md) | Receipt API header convention | Accepted | `X-Verifiable-Receipt: 1` opt-in header (resolves PRD OQ-3) |
| [RFC-0007](./docs/rfcs/RFC-0007-hf-deployment-recipe.md) | HF Inference Endpoints deployment recipe | Accepted | Deploy script, registry choice, fallback path |
| [RFC-0008](./docs/rfcs/RFC-0008-tui-architecture.md) | TUI architecture | Accepted | Frame model, phase walk, tamper, delay |
| [RFC-0009](./docs/rfcs/RFC-0009-tamper-fuzz-harness.md) | Tamper fuzz harness | Accepted | Per-PR 100-flip + nightly 1000-flip protocol |
| [RFC-0010](./docs/rfcs/RFC-0010-corridor-measurement.md) | Corridor measurement methodology | Accepted | Workload set, layer coverage, report schema |
| [RFC-0011](./docs/rfcs/RFC-0011-commitllm-upstream-pinning.md) | CommitLLM upstream pinning | Accepted | Pin policy, rename window plan (resolves PRD OQ-4) |
| [RFC-0012](./docs/rfcs/RFC-0012-w8a8-quantization.md) | W8A8 quantization and checkpoint hosting | Accepted | `llm-compressor` recipe, mirror checkpoint (resolves PRD OQ-2 default) |
| [RFC-0013](./docs/rfcs/RFC-0013-ci-pipeline.md) | CI pipeline and GPU-on-demand workflow | Accepted | CI matrix, GPU job gating, artifact retention |
| [RFC-0014](./docs/rfcs/RFC-0014-error-taxonomy.md) | Error taxonomy and exit codes | Accepted | Error categories, exit code map, JSON shape |
| [RFC-0015](./docs/rfcs/RFC-0015-observability-schema.md) | Observability schema | Accepted | Log event schema, span model, redaction rules |

## Open questions

Carried from PRD and tracked as labelled issues. Each has an owner and a resolution trigger.

| ID | Question | Default | Resolution trigger |
|----|----------|---------|--------------------|
| OQ-1 | Repository organization (personal vs `starkware-libs`) | personal | Before public release |
| OQ-5 | HF custom-container limits acceptable? | self-hosted as reference, HF "may require tuning" | 1-week spike at build phase start |
| OQ-6 | Public demo endpoint? | self-hosted-only at launch | 2 weeks before public release |
| OQ-7 | Corridor escalation policy if numbers diverge | publish gap + contribute upstream | When corridor measurement returns |

OQ-2, OQ-3, OQ-4 are resolved by RFCs (see index).

## Decisions made under uncertainty

These are spec-phase decisions made without further user input. They are reversible by superseding RFC.

1. **`AbdelStark/verifiable-intelligence`** as the canonical home for v1. A move to an institutional org is a renaming exercise, not a re-spec.
2. **Quantize ourselves and publish under `AbdelStark/Llama-3.2-1B-Instruct-quantized.w8a8`** as the default path. Adopted if upstream W8A8 becomes available.
3. **`X-Verifiable-Receipt: 1` opt-in header** for receipt requests. Content negotiation rejected because vLLM's `Accept` handling is constrained; query-string rejected because it pollutes URL telemetry.
4. **Pin to a pre-rename CommitLLM commit** and track the rename PR; do not vendor.
5. **CI runs corridor measurement on demand, not on every PR.** GPU time is a budget.
6. **CLI default output is JSON**; `--pretty` for human reading. Inverted from many CLIs because the integration use case is the primary one.
7. **Receipt MIME identification by magic prefix**, not file extension. Files may be base64-text-wrapped on platforms that mangle binary; magic is authoritative.

## Residual risk

Three items that the spec cannot eliminate and that v1 must accept or escalate:

1. **Corridor numbers on 1B may fall outside CommitLLM's 7B/8B envelope.** RFC-0010 names this; PRD R1 names this. If `frac<=1 < 99.5%`, v1 ships with a tightened published tolerance and the gap is documented; if materially worse, v1 escalates to Llama 3.2 3B (PRD R1 fallback) and revises NFRs.
2. **HF Inference Endpoints custom-container behavior is not fully knowable in advance.** RFC-0007 ships self-hosted as the always-supported reference path; HF is the preferred but not guaranteed deployment target.
3. **CommitLLM rename window.** RFC-0011 pins pre-rename; a forced re-pin during v1 build is a small but real schedule risk.
