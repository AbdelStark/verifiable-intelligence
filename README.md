# verifiable-intelligence

Browser-first research demo for verifiable open-weight LLM inference markets, built on the [CommitLLM](https://github.com/lambdaclass/CommitLLM) commit-and-audit protocol.

## Status

Scope pivot in progress. The original repository was specified as a Rust CLI plus TUI reference application. As of 2026-06-29, the v1 target is a buyer-facing proof marketplace demo: a consumer picks an inference provider, sends a prompt, receives an answer plus a CommitLLM-backed proof bundle, and verifies in the browser which model, prompt, decode policy, and delivered answer were bound.

This is a research proof of concept. It is not a token resale service, not a payment product, and not a tool for unauthorized API-key resale or provider-term evasion.

## What v1 is

`verifiable-intelligence` turns CommitLLM into a demo of a lawful, adversarial AI compute marketplace:

1. **Browser demo app**: provider catalog, quote, prompt, response, proof card, and verifier timeline in one page.
2. **Proof bundle format**: a portable artifact containing the provider quote, CommitLLM receipt, verifier-key identity, prompt hash, delivered-answer hash, audit endpoint, and verification report.
3. **Provider adapter path**: OpenAI-compatible chat surface extended with `X-Verifiable-Receipt: 1`, backed by a CommitLLM-instrumented open-weight model.
4. **WASM verifier spike**: browser-side verification becomes v1 work because consumers should not need Rust tooling to inspect a proof.

The older Rust CLI remains useful for key generation, fixture validation, CI, and power users. The old TUI is no longer the primary demo surface.

The v1 live reference model is `llama-3.1-8b-w8a8` with CommitLLM profile `llama-w8a8-audited` at pin `25541e83`; see [`docs/spikes/reference-model.md`](./docs/spikes/reference-model.md).

## What v1 is not

- No closed-weight model verification. CommitLLM requires public weights.
- No Anthropic/OpenAI/Gemini attestation unless those providers publish compatible model commitments or signatures.
- No real-money payment rail in v1. The demo can simulate credits or use test-mode payments only.
- No unauthorized token resale, credential handling, or bypass of provider terms.
- No new cryptography. The protocol engine remains CommitLLM.
- No claim that verification proves factual correctness. It checks execution integrity for supported paths.

## Try the demo prototype

Hosted static demo: <https://abdelstark.github.io/verifiable-intelligence/>

Open [`demo/index.html`](./demo/index.html) in a browser. It is a static prototype with simulated provider responses and proof objects. It is meant to make the buyer flow legible before the live CommitLLM backend lands.

Browser smoke tests:

```bash
npm install
npx playwright install chromium
npm run test:demo
```

Proof bundle fixture validation:

```bash
npm run test:bundle
```

Pinned CommitLLM browser-WASM verifier harness:

```bash
npm run build:wasm
npm run test:wasm
```

The static demo still labels its in-page checks as simulated. The WASM harness under `verifier/wasm/` is the real browser verifier path for the pinned upstream full-bridge fixture.

## Why CommitLLM

CommitLLM returns a compact receipt and opens trace data only when challenged. On supported open-weight deployments, verifier work is CPU-side and provider serving stays on the normal GPU path. Its current boundary matters: model identity, prompt/request binding, decode policy, delivered answer, and many execution checks are covered; arbitrary-position attention output on stock GPU kernels remains a documented open problem upstream.

For the current scope, read [`PRD.md`](./PRD.md), [`SPEC.md`](./SPEC.md), and [`docs/rfcs/RFC-0016-marketplace-demo-pivot.md`](./docs/rfcs/RFC-0016-marketplace-demo-pivot.md).

Guides:

- [Buyer proof guide](./docs/guides/buyer-proof-guide.md)
- [Provider integration guide](./docs/guides/provider-integration-guide.md)
- [Self-hosted provider compose guide](./docs/deployment/self-hosted.md)
- [v0.1.0 pivot demo release notes](./docs/release/v0.1.0-pivot-demo.md)

## Protocol Pin

Verified against CommitLLM pin: `lambdaclass/CommitLLM@25541e83347655e44ad6e84eb901e1e7ae392a66` (`25541e83`).

## License

MIT, matching CommitLLM upstream.
