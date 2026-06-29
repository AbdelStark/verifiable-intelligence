# Implementation Tracker - 2026-06-29 Pivot

The original tracker was generated from PR #1 on 2026-05-12 and filed issues #2-#115 for a Rust CLI/TUI reference app. That backlog is still live on GitHub, but it no longer matches the v1 product shape.

The new v1 target is the marketplace proof demo from [RFC-0016](../rfcs/RFC-0016-marketplace-demo-pivot.md): browser app, proof bundle, provider quote, CommitLLM receipt, browser verification, and red-path failures.

## Current gap

| Area | Old backlog state | Pivot gap |
|------|-------------------|-----------|
| Demo UX | TUI issues #3, #53-#58, #112 | Browser app, provider cards, proof card, bundle inspector, mobile layout |
| Browser verifier | Deferred to v1.1 | v1-critical WASM spike and measured decision |
| Proof artifact | Binary receipt/key/audit envelopes only | `VIEX` bundle schema joining quote, prompt, answer, receipt, key, audit, report |
| Marketplace context | Not modeled | Provider catalog, quote, price, expiry, provider identity, lawful-use boundary |
| Provider model choice | Llama 3.2 1B W8A8 corridor blocker | Prefer CommitLLM-supported measured Llama/Qwen W8A8 profile |
| Red paths | Receipt byte tamper only | Model swap, prompt mismatch, answer rewrite, expired quote, wrong key, unsupported model |
| Safety framing | Provider abuse controls | Explicit no unauthorized resale, no credential handling, no closed-weight proof claim |
| Release gates | CLI timing and TUI comprehension | Browser demo completion, red-path comprehension, bundle validation, WASM spike |

## Reusable old issues

These old issues remain valuable with little or no conceptual change:

- #22 CommitLLM pin.
- #26-#30 receipt envelope and fuzz tests.
- #31-#35 keygen and key-size checks.
- #36-#40 HTTP client and audit client.
- #41, #43, #44 verifier dispatch and identity mismatch tests.
- #59-#62 observability and trace propagation.
- #63-#69 provider image and compose path.
- #83-#86 tamper fixtures and CI pipeline, extended to bundles.
- #94-#98 JSON schemas, expanded for `VIEX`.

## Superseded or deferred old issues

These should not block v1 after the pivot:

- #3, #49, #53-#58, #112 TUI work. Deferred unless the browser demo later needs a terminal companion.
- #70-#72 HF deployment as the default path. Keep as a deploy option, not the v1 spine.
- #79-#82 Llama 3.2 1B corridor measurement. Move to research backlog unless chosen as the live model.
- #90 old fresh-environment CLI timing gate. Replace with browser demo timing.
- #99 old README quickstart. Rewrite around proof marketplace flow.
- #106 old integrating-developer user guide. Replace with buyer/provider proof guide.

## New issue plan

### Epic #119: Marketplace proof demo

Child issues:

- #123 demo: browser proof market prototype hardening and smoke tests.
- #120 bundle: define `VIEX` JSON Schema and canonical fixtures.
- #121 verifier: WASM feasibility spike for CommitLLM proof bundles.
- #126 provider: choose live CommitLLM reference model and pin.
- #127 broker: fixture provider catalog and quote/chat API skeleton.
- #125 tests: proof-bundle tamper and model-substitution suite.
- #124 docs: buyer/provider proof guide and lawful-use boundary.
- #122 release: static demo hosting and pivot comprehension gate.

### Epic A: Marketplace proof demo surface

1. Browser demo: static `demo/index.html` with provider cards, quote, prompt, proof card, and red-path toggles.
2. Browser demo tests: Playwright smoke for desktop and mobile, happy path and red paths.
3. Demo copy review: lawful-use boundary and unsupported closed-weight model state.

### Epic B: Proof bundle and schemas

1. `VIEX` JSON Schema.
2. Happy-path proof bundle fixture.
3. Red-path fixtures for model swap, prompt mismatch, answer rewrite, expired quote, wrong key, and receipt tamper.
4. `vi bundle inspect` and `vi bundle verify` utility commands.

### Epic C: Browser verifier

1. WASM feasibility spike against pinned CommitLLM verifier.
2. Browser verifier package size and memory report.
3. Browser verifier fixture runner.
4. Server-side verifier fallback only if spike fails, labeled in UI.

### Epic D: Provider and broker adapter

1. Provider catalog JSON and local fixture provider.
2. Quote API shape and quote-signature policy.
3. Chat proxy preserving `X-Verifiable-Receipt: 1`.
4. Audit proxy preserving challenge and payload binding.
5. Live provider integration with the chosen CommitLLM-supported model.

### Epic E: Release and comprehension gates

1. README rewrite for buyer/provider/reviewer flow.
2. Proof guide: what the bundle proves and does not prove.
3. Comprehension gate for open-weight-only, execution-integrity-only, and no unauthorized resale.
4. v1 release checklist and hosted-demo cost decision.

## Dependency order

1. Static demo and proof bundle draft.
2. Schema and fixtures.
3. Browser tests over static fixtures.
4. WASM verifier spike.
5. Provider/broker adapter skeleton.
6. Live CommitLLM provider integration.
7. Public release gates.

## Live GitHub issue action

Create a new pivot epic and child issues for the plan above. Do not mass-close #2-#115 until the pivot epic is accepted. For old issues that are clearly superseded, comment with the pivot epic link before closing so GitHub history stays understandable.
