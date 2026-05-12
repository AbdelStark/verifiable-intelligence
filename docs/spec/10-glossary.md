# 10 — Glossary

Canonical terms used across the corpus. When a term has both a colloquial sense and a precise sense here, the precise sense governs in spec/RFC/issue prose. Synonyms that should not be used are listed under "do not say."

## Protocol terms

**Attention corridor.** The CommitLLM construct that bounds the elementwise deviation between a teacher (reference, fp16) and a prover (deployed, quantized) attention computation. The corridor is empirical: published as `L_inf`, `frac_eq`, `frac<=1` over a calibration set. Not a proof; a bound.

**Audit endpoint.** The provider-side HTTP path (`POST /v1/audit`) that returns an audit payload for a given `(request_id, tier, token_index, layer_indices)`.

**Audit payload.** The binary blob produced by the provider in response to an audit challenge. Envelope-wrapped (`VIAU`), CommitLLM-defined payload.

**Bridge replay.** A verification phase that re-runs a subset of model computations between layers from received commitments. Inherited terminology from CommitLLM.

**Decode policy.** The sampling configuration (greedy vs sampled, temperature, top-k, top-p) the provider used at inference time. Verifiable per the protocol.

**Deep tier.** A verification tier that requests more layer-level state than `routine`. Strictly between `routine` and `full` in cost.

**Embedding Merkle.** A verification phase that proves the input token embeddings were drawn from the committed embedding table. Inherited.

**Freivalds (shell).** A probabilistic verification of matrix products applied to outer shell projections. Inherited.

**Full tier.** The most thorough verification tier. Requires audit-endpoint round-trips. Single-token by default in v1.

**KV provenance.** A verification phase that proves the key-value cache state used at decode time was consistent with the committed prefix. Inherited.

**LM head.** The output projection from hidden state to logits. A verification phase ensures the logits the prover claims are what the LM head produces from the committed hidden state.

**Phase.** One of the seven verification steps the verifier walks through: `embedding_merkle`, `shell_freivalds`, `bridge_replay`, `attention_corridor`, `kv_provenance`, `lm_head`, `decode_policy`. The exact set is defined by CommitLLM at the pin.

**Prover.** The provider-side instrumented inference engine that emits a receipt and serves audit challenges.

**Receipt.** The binary blob a provider returns alongside generated text when the client opts in via `X-Verifiable-Receipt: 1`. Envelope-wrapped (`VIRC`), CommitLLM-defined payload.

**Receipt-only tier.** A verification tier that checks structural integrity and binding without challenging the prover. Useful for batch operations; not a strong guarantee.

**Routine tier.** The default verification tier. Probabilistic spot-check; cheap (< 200 ms target); no audit round-trip.

**Tamper.** Any post-emission modification to a receipt or audit payload. Detected as `corrupt_envelope` or `verification_failed` depending on the layer.

**Verifier.** The client-side library and CLI that consumes receipts and produces a structured report. Pure CPU.

**Verifier key.** A binary artifact bound to a specific `(model_id, checkpoint_hash, commitllm_pin, seed)` that the verifier uses to validate receipts. Public, distributable. Envelope-wrapped (`VIKY`).

## Project terms

**Binding header.** Project-defined prefix inside an envelope payload that carries the `model_id`, `checkpoint_hash`, `commitllm_pin`, and a CRC32C across them. Enforced before any CommitLLM-internal parsing runs.

**CommitLLM pin.** The exact upstream commit SHA the provider and verifier are bound to. Stored in the binding header and the `/healthz` advertisement.

**Comprehension gate.** A pre-release manual test (SM-5, SM-6) that requires an external reviewer or non-cryptographer audience to demonstrate the README or TUI communicates the intended bounds.

**Corridor measurement.** This project's reproducible measurement of the attention corridor on Llama 3.2 1B Instruct W8A8. The numbers go in the README.

**Deployment recipe.** A documented sequence (script + prose) for deploying the provider image to a named target. v1 ships HF Endpoints and `docker compose` recipes.

**Demo audience.** The persona that watches the TUI rather than running the CLI. See [PRD §3](../../PRD.md).

**Envelope.** The four-byte magic + version + flags + payload wrapping every binary artifact this project produces. See [03-data-model.md](./03-data-model.md).

**Fresh-environment test.** The CI job that runs the install-to-verify sequence in a clean container with no project caches. Gates SM-1.

**Integrating developer.** The primary v1 persona. Builds the CLI into their application or CI.

**Magic prefix.** Four ASCII bytes that identify an envelope kind (`VIKY`, `VIRC`, `VIAU`).

**Reference deployment.** The provider deployment target with end-to-end CI coverage. For v1, HF Inference Endpoints (with `docker compose` as the always-available fallback).

**Reference laptop.** The 2023-class consumer machine class against which latency budgets are measured.

**Tamper fuzz harness.** The CI mechanism that randomly flips bytes in a valid receipt and asserts that every flip produces an error. 100 flips per PR, 1000 nightly.

**Trace ID.** A ULID generated at process start by the CLI. Surfaced in logs, in the error envelope, and (optionally) sent to the provider as a correlation header.

## Excluded vocabulary

Do not use, in specs, RFCs, issues, commits, code comments, or user-facing text:

- "Proof" (without qualifier) when referring to corridor or attention checks. They are bounds. Use "bound", "check", or "verification".
- "Cryptographically certified", "guaranteed correct", "tamper-proof". Use the specific protocol term.
- "ZK", "zero-knowledge". Not what this is. Reserve for explicit contrast in the README preamble.
- "Verified inference" without qualifying that verification is integrity-and-execution-integrity, not factual correctness.
- "Audit log" for our trace logs. We call them "logs". "Audit" is reserved for the protocol's audit endpoint and payload.
- "Endpoint" for anything other than the provider's HTTP endpoint. Do not call CLI subcommands "endpoints".
