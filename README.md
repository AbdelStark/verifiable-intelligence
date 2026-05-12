# verifiable-intelligence

A reference application demonstrating end-to-end verifiable LLM inference, built on the [CommitLLM](https://github.com/lambdaclass/CommitLLM) commit-and-audit protocol.

## Status

Pre-implementation. The product requirements are defined in [`PRD.md`](./PRD.md). The repository will be built out under that PRD, starting with a Rust CLI and TUI against Llama 3.2 1B Instruct W8A8 served from a Hugging Face Inference Endpoint.

## What v1 is

`verifiable-intelligence` is **not** a new proof system. The protocol is CommitLLM, and the cryptographic engine lives upstream in the lambdaclass repository. This repository builds two developer-facing surfaces on top of that engine:

1. **A Rust CLI (`vi`)** for use in scripts, CI, and integration. One-command verification of a CommitLLM receipt against a published verifier key, CPU-only on the user side.
2. **A Rust TUI (`vi tui`)** for talks, screencasts, and onboarding. Live walk through verification phases with per-phase pass/fail indicators. A tampered receipt fails visibly.

The provider side is a Docker image that runs the CommitLLM-instrumented vLLM stack and serves Llama 3.2 1B Instruct W8A8. The reference deployment is a Hugging Face Inference Endpoint as a custom container, deployed via the `hf` CLI. Self-hosted GPU is supported via `docker compose`. The image is vendor-neutral: no Modal-specific code in the critical path.

## What v1 is not

A WASM browser verifier (v1.1) and a batched compliance flow (v1.2) are explicitly post-v1 milestones. Both are scoped in section 12 of the PRD. Neither ships in v1.

The product cannot verify closed-weight models. The underlying protocol requires public weights, by design. This is not a regulatory product. The compliance flow (v1.2) is illustrative and has not been cleared by any regulator. This is not a production inference service. The provider endpoint is a demonstration deployment with sensible defaults, not a SaaS.

## Why CommitLLM and not ZKPs

Zero-knowledge proofs of LLM inference are not yet practical at production scale. CommitLLM trades that off for an interactive commit-and-audit scheme that runs on real hardware today, at the cost of (a) open-weights only and (b) requiring the provider and the verifier to exchange an audit challenge. Those tradeoffs are real and are documented up front rather than buried.

For the full requirements, scope boundaries, and open questions, read [`PRD.md`](./PRD.md).

## License

MIT, matching CommitLLM upstream.
