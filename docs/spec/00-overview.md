# 00 - Overview

## Thesis

The integrity gap between what an inference provider claims and what a consumer can independently check is most visible in low-trust markets. `verifiable-intelligence` turns CommitLLM into a buyer-facing proof demo: choose a provider, submit a prompt, receive an answer, verify the model/prompt/decode/answer binding, and keep a portable proof bundle.

This project is not a protocol and not a marketplace operator. It is an integration artifact and a research demo for authorized open-weight providers.

## Goals (v1)

- **G1 - Browser-first buyer demo.** Static app shows provider catalog, quote, prompt, response, proof card, verification timeline, and red-path failures.
- **G2 - Portable proof bundle.** `VIEX` bundles quote, request, response, receipt, key identity, audit reference, and report.
- **G3 - Browser verification path.** WASM verifier spike is v1-critical; server-side verification is only a labeled prototype fallback.
- **G4 - CommitLLM-supported provider path.** Use an upstream-supported open-weight W8A8 profile before new small-model corridor research.
- **G5 - Failure cases as product surface.** Model swap, prompt mismatch, answer rewrite, expired quote, wrong key, and receipt tamper are visible demo states.
- **G6 - Honest boundary.** Docs and UI state open-weight-only, execution-integrity-only, no unauthorized resale, and no closed-weight frontier support.

## Non-goals (v1)

- No unauthorized token resale, credential pooling, or provider-term evasion.
- No closed-weight model verification.
- No production marketplace, escrow, KYC, billing, or dispute resolution.
- No new proof system or protocol fork.
- No factual-correctness claim.
- No terminal TUI release gate.
- No multi-model marketplace breadth.

## Personas

- **Consumer Buyer.** Wants to know which open-weight model produced a purchased answer.
- **Honest Provider.** Wants to prove a model claim in a low-trust market.
- **Research Reviewer.** Wants reproducible fixtures and clear protocol limits.

## Success criteria

v1 ships when:

1. Static demo happy path works in a browser with no build step.
2. Fake-model, prompt-mismatch, answer-rewrite, and receipt-tamper demo states fail visibly.
3. `VIEX` schema and fixtures are checked in and validated in CI.
4. Browser verifier spike has measured output and a clear go/no-go.
5. README/demo comprehension test passes: open-weight-only, no unauthorized resale, execution-integrity-only, and proof-boundary understood.

## Versioning

- **v0.1** - pivot docs, static demo prototype, proof bundle draft schema.
- **v0.2** - proof bundle fixtures, browser smoke tests, red-path validations.
- **v0.3** - WASM verifier spike result and provider adapter skeleton.
- **v1.0** - public research demo with all success criteria green.
