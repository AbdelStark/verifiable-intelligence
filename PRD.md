# verifiable-intelligence: Product Requirements Document

- Status: Draft, pivoted
- Author: AbdelStark
- Created: 2026-05-12
- Updated: 2026-06-29
- Target milestone: v1 research demo

## 1. Summary

`verifiable-intelligence` is now scoped as a browser-first research demo for verifiable open-weight LLM inference markets. A consumer chooses an authorized provider, sees the claimed model and price, sends a prompt, receives an answer, and verifies a portable proof bundle that binds the provider quote, prompt hash, model identity, decode policy, delivered answer, CommitLLM receipt, verifier key, and audit endpoint.

The motivating market failure is simple: a buyer cannot tell whether a remote provider served the advertised model or quietly substituted a cheaper one. The product answer is not a real resale marketplace. It is a proof-of-concept surface showing how a market could make model substitution visible and rejectable.

This project does not invent cryptography. CommitLLM remains the protocol engine. The contribution is a consumer-readable demo app, a proof bundle format, a broker/provider contract, and tests that show model swaps, prompt changes, answer rewrites, and receipt tampering fail verification.

## 2. Problem

Remote LLM APIs ask buyers to trust provider claims about model identity, quantization, prompt handling, decode policy, and post-processing. In adversarial secondary markets, those claims are especially weak: the seller can advertise a premium model and serve a cheaper model, a different quantization, or a rewritten answer. A buyer usually sees only text and a bill.

CommitLLM addresses this for open-weight models by returning compact receipts and opening trace data on challenge. That still leaves a product gap:

- There is no buyer-facing proof card that a non-cryptographer can inspect.
- There is no market-shaped artifact joining quote, prompt, receipt, delivered answer, and verifier report.
- There is no browser-first verifier path for consumers.
- The current backlog is terminal-first and optimized for developers, not buyers.

The project should make the proof moment obvious: the buyer asks "what did I buy?", the app answers "this model, this checkpoint, this prompt hash, this decode policy, this delivered answer, verified under this CommitLLM pin."

## 3. Lawful-Use Boundary

The idea was motivated by unauthorized token markets, but the project must not help build one. v1 supports only authorized open-weight providers and synthetic/demo credits. It must not store, solicit, validate, broker, or launder third-party API keys. It must not provide instructions for bypassing provider terms, rate limits, billing controls, or identity checks.

Allowed framing:

- Research demo for untrusted but authorized AI compute providers.
- Buyer protection for open-weight model marketplaces.
- Procurement, decentralized compute, compliance, and benchmark reproducibility.

Disallowed framing:

- Unauthorized token resale.
- Credential resale, API-key pooling, or stolen account usage.
- Claiming support for closed-weight frontier models without provider-signed compatible attestations.

## 4. Users & Personas

### Primary persona: the Consumer Buyer

A user buys an inference call from a provider they do not fully trust. They are not a cryptographer and should not need a Rust install. They need to know whether the claimed open-weight model ran on the prompt they submitted and whether the displayed answer is the answer committed by the provider.

Success for them is a browser proof card with a plain verdict, a downloadable bundle, and a failure that is obvious when the provider swaps the model or tampers with the answer.

### Secondary persona: the Honest Provider

An authorized open-weight inference provider wants to compete in a low-trust market. They can publish a verifier key and CommitLLM pin, serve receipts, answer audit challenges, and show buyers a verification badge that is harder to fake than marketing copy.

Success for them is a minimal adapter contract, not a new serving stack.

### Tertiary persona: the Research Reviewer

A researcher or investor wants to understand whether CommitLLM can support market-style proof objects. They need to inspect the protocol boundary, failure cases, and source fixtures.

Success for them is a repo where the demo's green path and red paths are reproducible.

## 5. Goals

- G1. Ship a static browser prototype that demonstrates provider selection, quote, prompt, response, proof bundle, verification timeline, and tamper/model-swap rejection without a backend.
- G2. Define a `VIEX` proof bundle format that joins quote, prompt hash, delivered-answer hash, CommitLLM receipt, verifier-key identity, audit endpoint, verification report, and provider metadata.
- G3. Move browser verification into v1 scope. The target is WASM verification over CommitLLM verifier crates. If WASM is blocked, the prototype may temporarily call a server-side verifier, but the public claim must say so.
- G4. Keep the provider side close to CommitLLM's maintained path: open-weight W8A8 model, normal GPU serving path, receipt opt-in header, audit endpoint.
- G5. Prefer an upstream-supported CommitLLM profile for v1, such as Llama 3.1 8B W8A8 or Qwen2.5 7B W8A8, before spending v1 time on unvalidated Llama 3.2 1B corridor research.
- G6. Make failure cases first-class: swapped model, changed prompt, rewritten answer, expired quote, wrong key, unsupported closed-weight model, and tampered receipt.
- G7. Maintain conservative claim boundaries in README, UI copy, CLI output, and issues.

## 6. Non-Goals

- NG1. No unauthorized token resale, credential handling, or provider-term evasion.
- NG2. No closed-weight model verification unless the model owner publishes compatible CommitLLM material or an equivalent signed attestation. v1 has no such integration.
- NG3. No production marketplace, escrow, dispute system, KYC, or payment processing.
- NG4. No new proof system and no modifications to CommitLLM.
- NG5. No claim of factual correctness. The proof checks execution integrity for supported paths.
- NG6. No guarantee that every CommitLLM phase is exact. The UI must distinguish exact, algebraic, statistical, audited, and open components.
- NG7. No terminal TUI as a v1 requirement. The browser demo replaces it.
- NG8. No multi-model marketplace breadth in v1. One real supported model plus mock provider variants is enough.

## 7. User Journeys

### Journey A: Buyer verifies a purchased response

1. The buyer opens the demo and sees three providers with claimed model, price, proof coverage, latency, and verifier-key identity.
2. The buyer chooses an authorized provider and writes a prompt.
3. The app creates a quote with provider ID, model ID, checkpoint hash, decode policy, price, expiry, and provider signature or demo signature.
4. The provider returns an answer and CommitLLM receipt.
5. The app builds a `VIEX` proof bundle and runs verification.
6. The buyer sees a verdict: pass, fail, or unsupported. The proof card shows what is bound and what is not.
7. The buyer downloads the bundle or copies a shareable proof summary.

### Journey B: Buyer catches a fake model

1. The buyer toggles the demo provider from honest to fake.
2. The provider claims Llama but serves a fixture bound to Qwen, or returns a receipt under the wrong key.
3. Verification fails at identity binding before any persuasive answer quality heuristic matters.
4. The UI shows the mismatched fields and marks the quote as not satisfied.

### Journey C: Reviewer inspects the artifact

1. The reviewer opens a proof bundle JSON file.
2. They see quote, prompt hash, answer hash, receipt metadata, key hash, CommitLLM pin, audit endpoint, and verification report.
3. They run the verifier through the browser or CLI fixture command.
4. They tamper with one field and confirm the verifier rejects it.

## 8. Functional Requirements

- FR-1. The repository MUST include `demo/index.html`, a static browser prototype requiring no build step.
- FR-2. The demo MUST include at least three provider cards: honest verified, cheap unverified, and fake-model failure.
- FR-3. The demo MUST let the user enter or select a prompt and produce a simulated answer plus proof card.
- FR-4. The demo MUST show a verification timeline with at least these checks: quote expiry, provider identity, model/checkpoint binding, prompt hash, receipt integrity, decode policy, delivered answer hash, and audit challenge.
- FR-5. The demo MUST include visible red-path cases for model swap, prompt mismatch, answer rewrite, and receipt tamper.
- FR-6. The proof bundle format MUST have a magic identifier `VIEX`, schema version, quote, request binding, response binding, receipt reference or bytes, verifier-key identity, audit reference, and verification report.
- FR-7. The provider adapter MUST retain the `X-Verifiable-Receipt: 1` request convention and `POST /v1/audit` audit convention unless CommitLLM upstream provides a better maintained interface.
- FR-8. The broker API MUST be optional in v1. The static demo can simulate it; the spec must define `GET /providers`, `POST /quotes`, `POST /chat`, and `POST /verify` for later implementation.
- FR-9. The browser verifier MUST be investigated in v1. The spike must conclude with one of: native WASM verifier works, WASM works with size/perf caveats, or server-side verification is required for the prototype.
- FR-10. The UI and docs MUST reject unsupported closed-weight models clearly, not silently downgrade to heuristic claims.
- FR-11. The old CLI issues MAY be reused for keygen, receipt parsing, fixture verification, and CI, but they no longer define the user-facing v1.
- FR-12. CI MUST validate example proof bundles against JSON Schema and run tamper tests over bundle fields, not only raw receipts.

## 9. Non-Functional Requirements

- NFR-1. Static demo first meaningful render under 2 seconds on a laptop browser.
- NFR-2. Happy-path simulated verification under 3 seconds in the static demo.
- NFR-3. Real browser verification target under 10 seconds for a routine proof on a warmed cache. If missed, publish measured numbers.
- NFR-4. Proof bundle target under 250 KB for routine verification excluding deep audit openings. If CommitLLM receipts exceed this, publish measured size.
- NFR-5. The UI must work on mobile and desktop without overlapping controls or clipped proof fields.
- NFR-6. All public wording must distinguish execution integrity from answer correctness.
- NFR-7. Provider and broker logs must never include raw prompts by default; hashes only.
- NFR-8. Any hosted demo must rate-limit provider calls and must not accept user-supplied third-party API keys.

## 10. Success Metrics

- SM-1. A new reader can complete the static demo happy path and identify the claimed model, prompt hash, and verification verdict in under 90 seconds.
- SM-2. A new reader can trigger the fake-model path and explain why it failed in under 90 seconds.
- SM-3. Example proof bundles validate against schema in CI.
- SM-4. Tampering any one of quote model ID, prompt hash, answer hash, key hash, receipt bytes, or CommitLLM pin fails verification or schema validation.
- SM-5. Browser verifier spike produces a measured decision with commands and artifacts.
- SM-6. README comprehension: 5/5 readers identify open-weight-only, no closed-weight frontier support, no unauthorized resale, and execution-integrity-only.

## 11. Constraints & Assumptions

- C1. CommitLLM remains the upstream protocol implementation.
- C2. v1 uses open-weight W8A8 models on a CommitLLM-supported path.
- C3. The demo can be static and simulated first; live GPU integration follows after the proof bundle and UI are clear.
- C4. Payment is simulated. If a payment provider is added, it must be test mode only and outside the proof-critical path.
- C5. The repository remains MIT, matching CommitLLM upstream.
- C6. The old issue corpus exists and is not deleted. It is superseded where it conflicts with RFC-0016.

## 12. Open Questions

- OQ-1. Resolved for v1: the live reference is `llama-3.1-8b-w8a8` with CommitLLM profile `llama-w8a8-audited` at pin `25541e83`. Qwen2.5 7B W8A8 remains a maintained secondary candidate and red-path/model-swap fixture. Llama 3.2 1B W8A8 moves to research backlog until it has a measured supported corridor.
- OQ-2. Can the CommitLLM verifier compile to WASM with acceptable binary size and browser memory use?
- OQ-3. Should proof bundles embed receipt bytes directly or reference them by content-addressed URL for deep audits?
- OQ-4. Should provider quotes be signed in v1 with Ed25519, or is a demo signature enough until live providers exist?
- OQ-5. Should the static prototype be kept as a long-term educational artifact after the live app lands?

## 13. Risks

- R1. **Misuse framing.** Marketplace language can be read as support for unauthorized resale. Mitigation: lawful-use boundary in README, PRD, demo UI, and issue templates.
- R2. **WASM blocker.** CommitLLM verifier may not compile cleanly to browser WASM. Mitigation: spike early and publish fallback honestly.
- R3. **Protocol overclaim.** CommitLLM has exact, algebraic, statistical, audited, and open components. Mitigation: proof card classifies each check.
- R4. **Reference model drift.** Upstream supported profiles may change. Mitigation: pin CommitLLM commit and verifier-key identities.
- R5. **Demo becoming too abstract.** A static mock can feel fake. Mitigation: fixtures must map to real CommitLLM receipt fields and later live integration issues.

## 14. References

- CommitLLM repository: https://github.com/lambdaclass/CommitLLM
- CommitLLM project site: https://commitllm.com/
- CommitLLM paper: https://raw.githubusercontent.com/lambdaclass/CommitLLM/main/paper/main.pdf
- Scope pivot RFC: [`docs/rfcs/RFC-0016-marketplace-demo-pivot.md`](./docs/rfcs/RFC-0016-marketplace-demo-pivot.md)
