# verifiable-intelligence: Specification

- Status: Draft, pivoted
- Authors: AbdelStark
- Created: 2026-05-12
- Updated: 2026-06-29
- Source of intent: [`PRD.md`](./PRD.md)

This document is the canonical entry point to the project specification corpus. The v1 target changed on 2026-06-29 from a terminal-first developer reference app to a browser-first proof marketplace research demo.

## Thesis

`verifiable-intelligence` makes model-substitution risk legible to a consumer buying inference from an untrusted but authorized open-weight provider. The consumer should see a quote, submit a prompt, receive an answer, and get a portable proof bundle that independently verifies the model identity, prompt binding, decode policy, and delivered answer under the CommitLLM protocol.

The protocol is not ours. The contribution is the marketplace-shaped integration: provider catalog, proof bundle, browser verifier, demo broker API, and a consumer-readable verification report.

## Scope

v1 ships:

- A static browser demo app that shows the full buyer flow: provider selection, quote, prompt, generated answer, proof bundle, verification timeline, and tamper/model-swap failure cases.
- A `VIEX` proof bundle format that packages the provider quote, CommitLLM receipt envelope, verifier-key identity, prompt and answer hashes, audit endpoint, and verification report.
- A provider adapter contract for OpenAI-compatible chat with `X-Verifiable-Receipt: 1` and `POST /v1/audit`.
- A WASM verifier spike and integration plan. Browser verification is now v1-critical.
- A toy credit/quote flow for demo purposes. Real money is out of scope unless handled by a test-mode payment provider.
- A lawful-use boundary: no stolen credentials, no unauthorized token resale, no bypass of upstream provider terms.
- CI fixtures that prove the demo rejects tampered receipts, swapped model identities, changed prompts, and rewritten answers.

The old Rust CLI/keygen/verifier crates remain implementation assets. The old Rust TUI and terminal-first release gates are superseded by [RFC-0016](./docs/rfcs/RFC-0016-marketplace-demo-pivot.md).

## Spec corpus

| File | Purpose |
|------|---------|
| [`docs/spec/00-overview.md`](./docs/spec/00-overview.md) | Thesis, goals, non-goals, success criteria, scope boundaries |
| [`docs/spec/01-architecture.md`](./docs/spec/01-architecture.md) | System architecture, components, data flow, module boundaries |
| [`docs/spec/02-public-api.md`](./docs/spec/02-public-api.md) | Browser app, broker API, provider API, CLI utility surface |
| [`docs/spec/03-data-model.md`](./docs/spec/03-data-model.md) | Proof bundle, quote, receipt, verifier key, audit payload |
| [`docs/spec/04-error-model.md`](./docs/spec/04-error-model.md) | Error taxonomy, exit codes, failure modes, recovery |
| [`docs/spec/05-observability.md`](./docs/spec/05-observability.md) | Structured logging, metrics, tracing, redaction rules |
| [`docs/spec/06-security.md`](./docs/spec/06-security.md) | Threat model, trust boundaries, secrets handling, abuse |
| [`docs/spec/07-testing-strategy.md`](./docs/spec/07-testing-strategy.md) | Unit, property, integration, tamper fuzz, browser checks |
| [`docs/spec/08-performance-budget.md`](./docs/spec/08-performance-budget.md) | Latency, size, memory targets; measurement methodology |
| [`docs/spec/09-release-and-versioning.md`](./docs/spec/09-release-and-versioning.md) | Semver, deprecation policy, changelog discipline |
| [`docs/spec/10-glossary.md`](./docs/spec/10-glossary.md) | Canonical terms |

## RFC index

| RFC | Title | Status | Locks |
|-----|-------|--------|-------|
| [RFC-0001](./docs/rfcs/RFC-0001-workspace-and-crate-layout.md) | Workspace and crate layout | Partially superseded | Rust crates remain useful, but v1 adds browser app and broker |
| [RFC-0002](./docs/rfcs/RFC-0002-cli-surface.md) | `vi` CLI surface | Partially superseded | CLI is utility, not v1 primary UX |
| [RFC-0003](./docs/rfcs/RFC-0003-receipt-format-pinning.md) | Receipt format pinning and version handling | Accepted | Still valid inside proof bundle |
| [RFC-0004](./docs/rfcs/RFC-0004-verifier-key-generation.md) | Verifier key generation and binding | Accepted | Still valid |
| [RFC-0005](./docs/rfcs/RFC-0005-provider-image.md) | Provider Docker image architecture | Accepted | Still valid for provider adapters |
| [RFC-0006](./docs/rfcs/RFC-0006-receipt-api-header.md) | Receipt API header convention | Accepted | Still valid |
| [RFC-0007](./docs/rfcs/RFC-0007-hf-deployment-recipe.md) | HF Inference Endpoints deployment recipe | Partially superseded | Reference deploy may move to the easiest CommitLLM-supported GPU host |
| [RFC-0008](./docs/rfcs/RFC-0008-tui-architecture.md) | TUI architecture | Superseded for v1 | Browser demo replaces TUI as the primary demo surface |
| [RFC-0009](./docs/rfcs/RFC-0009-tamper-fuzz-harness.md) | Tamper fuzz harness | Accepted | Extended to proof bundles |
| [RFC-0010](./docs/rfcs/RFC-0010-corridor-measurement.md) | Corridor measurement methodology | Partially superseded | v1 should prefer a CommitLLM-supported measured model before new 1B research |
| [RFC-0011](./docs/rfcs/RFC-0011-commitllm-upstream-pinning.md) | CommitLLM upstream pinning | Accepted | Still valid |
| [RFC-0012](./docs/rfcs/RFC-0012-w8a8-quantization.md) | W8A8 quantization and checkpoint hosting | Partially superseded | Model choice reopens under the marketplace pivot |
| [RFC-0013](./docs/rfcs/RFC-0013-ci-pipeline.md) | CI pipeline and GPU-on-demand workflow | Partially superseded | Browser and bundle tests added |
| [RFC-0014](./docs/rfcs/RFC-0014-error-taxonomy.md) | Error taxonomy and exit codes | Accepted | Still valid for CLI and verifier internals |
| [RFC-0015](./docs/rfcs/RFC-0015-observability-schema.md) | Observability schema | Accepted | Extended to broker/provider correlation |
| [RFC-0016](./docs/rfcs/RFC-0016-marketplace-demo-pivot.md) | Marketplace proof demo pivot | Accepted | Browser-first v1, proof bundle, lawful-use boundary |

## Open questions

| ID | Question | Default | Resolution trigger |
|----|----------|---------|--------------------|
| OQ-1 | Repository organization | personal repo | Before public release |
| OQ-2 | Reference model | CommitLLM-supported Llama/Qwen W8A8 profile | Before provider integration starts |
| OQ-3 | Browser verifier strategy | WASM wrapper over CommitLLM verifier if feasible; server-side verification fallback only for prototype | WASM spike |
| OQ-4 | Public hosted demo | Static demo public, live GPU endpoint gated by cost | Before public release |
| OQ-5 | Payment or credits | toy credits only | Before any hosted demo |
| OQ-6 | Provider eligibility | authorized open-weight providers only | Before provider catalog launch |

## Decisions made under uncertainty

1. **Browser-first demo.** Consumers of a marketplace proof should not install Rust to understand a receipt.
2. **No unauthorized resale support.** The motivating failure mode is model substitution in untrusted markets, but the project only supports authorized providers and open-weight models.
3. **Proof bundle over raw receipt.** A buyer needs quote, model identity, prompt hash, answer hash, audit endpoint, and report in one portable artifact.
4. **Prefer existing CommitLLM measured profiles.** The old Llama 3.2 1B W8A8 corridor work becomes research backlog, not the v1 blocker.
5. **CLI remains, TUI defers.** CLI utilities help implementation and CI. The terminal TUI no longer carries the demo story.

## Residual risk

1. **CommitLLM is still in active development.** Pinning and fixtures are mandatory.
2. **Browser verification may hit WASM or payload limits.** A server-side verifier can support the prototype, but v1 should prove the browser path or explicitly narrow the claim.
3. **Proof language can be misread.** The UI must say what is verified and what remains open, especially attention coverage and closed-weight non-support.
4. **Marketplace framing can be misused.** Docs and UI must prohibit unauthorized token resale and credential handling.
