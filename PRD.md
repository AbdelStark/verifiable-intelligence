# verifiable-intelligence: Product Requirements Document

- Status: Draft
- Author: AbdelStark
- Created: 2026-05-12
- Target milestone: v1 (alpha MVP), with v1.1 (WASM verifier) and v1.2 (batched compliance) on the horizon

## 1. Summary

`verifiable-intelligence` is a reference application that demonstrates, end to end, what verifiable LLM inference looks like when consumed by a real developer at a real terminal. It does not invent new cryptography. It uses the CommitLLM commit-and-audit protocol as its verification engine, anchored on Llama 3.2 1B Instruct W8A8 served from a Hugging Face Inference Endpoint, and ships two end-user surfaces in v1: a Rust CLI for use in scripts and CI, and a Rust TUI that walks through verification phases interactively for talks, screencasts, and onboarding. A browser-based WASM verifier and a batched compliance flow are explicitly post-v1 milestones with their own scope sections.

The thesis is unchanged from any earlier framing: verifiable inference becomes load-bearing only when it has shippable user surfaces. The v1 scope picks the smallest model and the leanest two surfaces that fully exercise the protocol and let an integrating developer adopt the verification step in one command. Surface count and feature count are intentionally below the original draft; the work that was cut moved to v1.1 and v1.2, not to "future work, maybe."

This project is a showcase, not a protocol. The protocol is CommitLLM. The contribution is: integration depth on a smaller-than-upstream-validated model, honest corridor measurements that we own, two ergonomic developer surfaces, and a vendor-neutral provider deployment story.

## 2. Problem

A developer who calls an LLM provider has no technical way to confirm that the provider executed the model it advertised, with the configuration it advertised, on the input it received, without modification of decode policy or output. Model substitution is undetectable. Quiet quantization swaps are undetectable. Silent decode-policy changes are undetectable. Today the developer accepts these on trust because no shippable verification surface exists at the integration layer.

This is not hypothetical. The CommitLLM paper and the underlying body of work on verifiable inference document the concrete attack surface. The gap between what providers claim and what outside parties can independently check is what the CommitLLM authors call the integrity gap. The engineering pieces that close that gap on open-weight models exist as Rust crates and a Python sidecar, demonstrated on Llama 3.1 8B W8A8 and Qwen 2.5 7B W8A8. What does not exist is a developer-shaped surface: there is no CLI a backend engineer can run after an API call, no TUI for live audit during a talk, no documented HF Endpoint deployment recipe, no validation that the protocol behaves on a model small enough for a side-project to actually adopt.

The pain workflow this project addresses:

A developer integrating an inference API against a stated model wants assurance that the bytes streamed back came from that model. Today the answer is "trust the provider's logs." The developer cannot reproduce verification independently with current tooling without standing up the full prover, capturing internal state, running on an A100, and writing custom verification code against CommitLLM's `verilm_rs` Python bindings. The CommitLLM repository ships the protocol but the developer-facing path stops at a benchmark script.

A secondary pain, deferred to v1.2 but named here for honesty: a compliance engineer at a frontier-lab deployer needs a low-cost way to demonstrate inference integrity over a population of responses for internal audit or external regulator review. Real-time per-response verification by a regulator is not viable at production volume. A batched receipt-and-spot-audit pattern is. That pattern is in the CommitLLM literature but not shipped. v1.2 will address this; v1 does not.

## 3. Users & Personas

### Primary persona: the Integrating Developer (v1)

A backend or platform engineer integrating an inference API as part of a product feature. They build against an API key, ship code, and read provider documentation. They are technically literate but not a cryptography expert. They want one extra step in their integration that gives them grounded confidence the model they paid for is the model they got, without forcing them to operate GPU infrastructure or learn STARK internals.

Today they solve this problem by inspecting outputs heuristically (does it sound like the claimed model), trusting the provider's marketing, or by running their own evaluation suite against the API and comparing to a local open-weight baseline. None of these give them per-response assurance. The CommitLLM primitives are the closest available technology but require running the full prover stack themselves, on GPU hardware, with custom Python.

What inadequacy this product addresses for them specifically: a one-command local verification step against a receipt the provider returned, CPU-only on their machine, with a binary key under 10 MB and a verifier latency under one second.

### Secondary persona: the Demo Audience (v1)

A researcher, conference attendee, prospective partner, or hiring contact watching a screencast or live walkthrough. They are not the user of the CLI; they are the audience for the TUI. The TUI exists to make verification visible: phases pass in real time, tampered receipts fail loudly, the protocol's exact-vs-approximate boundary is shown on screen.

What this product gives them: a 60 to 90 second walkthrough that turns "verifiable inference" from a phrase into a watched event. The TUI is the demonstration surface; the CLI is the integration surface; both ship in v1.

### Deferred personas (v1.1 and v1.2, named for scope clarity)

The Browser User (v1.1) verifies a receipt in a tab without installing anything. This persona requires the WASM verifier. The Compliance Engineer (v1.2) bundles a population of receipts for periodic audit. This persona requires the batched compliance flow. These are real users. They are not v1 users. The PRD scopes them in section 12.

## 4. Goals

- G1. Ship a working end-to-end demonstration where Llama 3.2 1B Instruct W8A8 is served from a Hugging Face Inference Endpoint running the CommitLLM-instrumented vLLM stack, and a developer on a laptop can run `vi chat` against the endpoint, receive a receipt, run `vi verify`, and see a structured pass/fail report. No GPU on the user side. No Modal dependency in the deployment harness.
- G2. Ship a Rust TUI (`vi tui`) that runs a prompt against the endpoint, displays the receipt as it arrives, and walks through verification phases (embedding Merkle, shell Freivalds, bridge replay, attention corridor, LM-head, decode policy) with live pass/fail indicators per phase. A tampered receipt MUST be visibly rejected by the TUI with the failing phase highlighted.
- G3. Validate the CommitLLM attention corridor on Llama 3.2 1B Instruct W8A8 on a real GPU across at least three workloads (short-answer factual, multi-turn reasoning, long-context code), all layers, and produce the same measurement shape CommitLLM has published for 7B/8B: global `L_inf`, first-generated-token max, decode max, `frac_eq`, `frac<=1`, growth-vs-context. Numbers go in the README and the paper-like writeup. Acceptance: numbers exist, are reproducible from a script in the repo, and either match CommitLLM's family-level tolerance envelope (around `L_inf <= 10`, `frac<=1 >= 99.8%`) or we publish exactly where and why they diverge.
- G4. Time-to-first-verified-call on a clean developer machine under 10 minutes, from `cargo install` (or binary download) through `vi keygen`, `vi chat`, and `vi verify` against the public endpoint. Measured by a fresh-environment CI job that runs the full sequence in a clean container.
- G5. Wall-clock CLI verification latency on commodity laptop hardware under 1 second for a full-tier single-token audit and under 200 milliseconds for a routine-tier audit. Numbers tighter than the original draft because the model is 8 times smaller, so the key is smaller, and the per-phase work scales accordingly. Measured on a 2023-class laptop CPU, no GPU.
- G6. Provider-side deployment is vendor-neutral. The provider package is a Docker image plus a `compose.yaml` for self-hosted, plus a documented recipe for Hugging Face Inference Endpoints (custom container) using the `hf` CLI. Modal-specific code is not in the critical path. Adding a new deployment target is a documentation exercise, not a refactor.
- G7. The preamble (README, docs) is honest about the protocol's boundaries: open-weights only, interactivity required, exact-where-possible and bounded-where-not, no claim of ZKP-class properties. A reviewer reading only the README understands what is and is not guaranteed. Validated by pre-release reader-comprehension check (SM-5).
- G8. Zero new cryptography is introduced in v1. All verification logic comes from CommitLLM crates as upstream dependencies. The only protocol-adjacent work we own is corridor measurement on a new model size (G3).

## 5. Non-Goals

- NG1. We are not building a new proof system or modifying the CommitLLM protocol. Any limitation of CommitLLM (open-weights only, interactivity required, attention corridor tolerance, statistical KV provenance unless deep audit) is a limitation of this product.
- NG2. We are not supporting closed-weight models. The product cannot verify Claude, GPT-4, Gemini, or any model whose weights are not public. Property of the underlying protocol.
- NG3. We are not building a production-grade hosted inference service. The provider-side serves a demonstration endpoint with sensible defaults for cost and abuse control, not a SaaS.
- NG4. We are not building a regulator dashboard. Compliance flow is v1.2 work and even there ships only as a CLI plus a bundle format, not a UI.
- NG5. We are not implementing a zero-knowledge proof variant. The choice of CommitLLM over a ZKP scheme is deliberate and documented in the preamble. Practical zkLLM is not yet available; integration would be a future-work item if it ever became viable, not a v1 swap-out.
- NG6. We are not supporting fine-tuned, LoRA-adapted, or merged-weight model variants in v1. The verifier key is bound to a published checkpoint hash.
- NG7. We are not building privacy-preserving variants of receipts. Receipts reveal the prompt and the generated tokens.
- NG8. We are not shipping a WASM verifier in v1. WASM is the v1.1 milestone. v1 ships CLI plus TUI.
- NG9. We are not shipping a batched compliance flow in v1. Batched compliance is the v1.2 milestone.
- NG10. We are not vendor-locked to Hugging Face. HF is the reference deployment because it is the cheapest, most reproducible target for the demo, but the provider package is portable.
- NG11. We are not building a Python SDK as part of v1. CommitLLM's Python bindings (`verilm_rs`) are inherited as-is for the prover side; the developer-facing surface in this project is Rust only in v1.
- NG12. We are not measuring corridors on multiple model sizes or families in v1. G3 covers Llama 3.2 1B exclusively. Validating Llama 3.2 3B, Qwen 2.5 1.5B, or any other model is out of scope and goes in v1.x or follow-on work.

## 6. User Journeys

### Journey A: Integrating Developer verifies a single response (v1)

A backend engineer at a mid-sized company is evaluating verifiable inference for their content-moderation feature. They want to see, in their own terminal, what the developer integration looks like end to end.

1. They read the project README, decide to try it, and run `cargo install verifiable-intelligence` (or download a prebuilt binary). Install completes in under two minutes.
2. They run `vi keygen --model llama-3.2-1b-w8a8 --output ./key.bin`. This is a deterministic operation that produces a binary verifier key under 10 MB and an artifact file. Time: under one minute on a laptop.
3. They run `vi chat --endpoint https://verifiable-intelligence.demo/v1 --prompt "What causes rainbows?"`. The CLI returns the generated text and a receipt blob on stdout in a documented format.
4. They run `vi verify --receipt receipt.bin --key key.bin --tier full`. Time from invocation to verification report: under one second. Output is structured JSON: tier, phases checked, phases passed, overall pass/fail, any failure details.
5. They wire the same logic into their CI: a sampled fraction of production calls is verified out-of-band, failures alert. CLI exit code drives this directly.

Moments of friction the product must remove. (a) The engineer must not have to operate GPU infrastructure or understand the prover internals. CommitLLM's prover side runs on the endpoint, not on their machine. (b) The verifier key generation step must not require manually downloading multi-gigabyte weights. The keygen tool fetches and hashes the checkpoint deterministically, or accepts a local path if they already have it. (c) The receipt format must be one stable, documented binary layout consumable by a CLI argument. No custom parsing required. (d) Errors must be specific. "Receipt rejected" without phase information is useless; "Receipt rejected at phase 4 (bridge replay) with `L_inf = 47` against tolerance `10`" is actionable.

### Journey B: Demo Audience watches the TUI (v1)

A conference attendee watches a screencast or a live walkthrough during a talk.

1. The presenter runs `vi tui`. A full-screen terminal interface opens: a prompt input at the top, a chat history in the middle, a verification panel on the right.
2. The presenter types a prompt. The TUI streams the response from the endpoint. The receipt arrives. The verification panel comes alive: phases appear one by one (embedding Merkle, shell Freivalds, bridge replay, attention corridor, KV provenance, LM-head, decode policy), each turning green as it passes. Total elapsed time visible on screen.
3. The presenter runs the same flow but with `--tamper byte-flip`. The TUI shows the same generation, then verification stops at a specific phase, the phase row turns red, and the failure detail is visible.
4. The presenter explains: the green-vs-red transition is the integrity guarantee. The audience sees, not hears, what verifiable inference means.

Moments of friction the TUI must remove. (a) The phase walk must be visible at human reading speed. If verification finishes in 200 ms, the TUI inserts deliberate per-phase delay (configurable, off by default in CI) so the audience can follow. (b) The tamper demonstration must be one keystroke or one flag, not a multi-step setup. (c) The TUI must run on any modern terminal (alacritty, kitty, iTerm2, GNOME Terminal, Windows Terminal) without special configuration.

### Deferred journeys

The browser verification journey (v1.1) and the batched audit journey (v1.2) are documented in section 12 with sketch-level user flows. They are not v1 journeys.

## 7. Functional Requirements

- FR-1. The system MUST provide a provider-side package that loads Llama 3.2 1B Instruct W8A8 on a GPU, runs the CommitLLM-instrumented vLLM serving stack, exposes a chat-completion HTTP endpoint, and returns a CommitLLM receipt alongside the generated text when the client requests one via a documented header (see OQ-3 for header convention).
- FR-2. The provider package MUST be deployable as a single Docker image. The image MUST be runnable via `docker run` for self-hosted GPU, via `docker compose up` with a provided `compose.yaml`, and via Hugging Face Inference Endpoints as a custom container using the `hf` CLI. No Modal-specific code in the image.
- FR-3. The repository MUST include a one-command deployment recipe for HF Inference Endpoints. The recipe builds the image, pushes it to a registry, creates the endpoint via `hf` CLI (or documents the exact UI clicks if CLI support is incomplete), and prints the endpoint URL. Time from running the command to a live endpoint under 30 minutes including image build, gated on HF availability.
- FR-4. The provider package MUST support W8A8 quantization of Llama 3.2 1B Instruct. If the model is not available pre-quantized in W8A8 on Hugging Face, the repository MUST include a one-time `quantize.py` script using the same `llm-compressor` toolchain CommitLLM uses for Llama 3.1 8B, and the resulting checkpoint MUST be published to Hugging Face under a documented name.
- FR-5. The provider MUST expose an audit endpoint accepting a request ID and an audit specification (token index, layer indices, tier: receipt-only, routine, deep, or full) and return a CommitLLM audit payload in the binary format defined by the CommitLLM verifier crate.
- FR-6. The system MUST provide a Rust CLI `vi` with these subcommands in v1: `keygen`, `chat`, `verify`, `tui`. Each subcommand MUST have a documented input contract, exit codes, and `--help` text. Additional subcommands (`verify-batch`, `batch-package`) are v1.2.
- FR-7. The CLI MUST be installable from a single `cargo install verifiable-intelligence` command, with prebuilt binaries published for Linux x86_64 and macOS arm64 via GitHub Releases. Windows is best-effort.
- FR-8. The CLI MUST produce machine-readable output (JSON to stdout) by default, with a `--pretty` flag for human-readable formatted output. CI integration MUST be possible without parsing prose.
- FR-9. The CLI MUST surface the CommitLLM verifier report verbatim in structured form: tier, phases checked, phases passed, phases failed, per-phase failure details (e.g. `L_inf` measurement vs tolerance for the attention corridor), elapsed time. The CLI never silently elevates a routine audit to a "verified" claim.
- FR-10. The system MUST provide a Rust TUI (`vi tui`) using `ratatui` (or an equivalent maintained crate) that runs a chat session against the configured endpoint, displays the receipt as it arrives, and walks through verification phases interactively with live pass/fail indicators per phase.
- FR-11. The TUI MUST support a `--tamper <kind>` flag that deliberately corrupts a chosen part of the receipt before verification, for demonstration purposes. At minimum, `byte-flip` (single random byte flip) MUST be supported. The tampered case MUST visibly fail verification and surface the failing phase.
- FR-12. The TUI MUST support a `--phase-delay <ms>` flag that inserts a configurable delay between phase transitions so the verification walk is human-readable during demos. Default delay is 0; the flag exists for screencast and talk usage.
- FR-13. The system MUST validate the attention corridor on Llama 3.2 1B Instruct W8A8 across at least three workloads (short-answer factual, multi-turn reasoning, long-context code generation), all layers, on a real GPU. The repository MUST include a reproducible script `scripts/corridor/measure.py` (or equivalent in Rust) that runs the measurement and emits a JSON report with global `L_inf`, first-generated-token max, decode max, `frac_eq`, `frac<=1`, and per-context-length growth.
- FR-14. The repository MUST publish corridor numbers in the README and (when written) the paper-like writeup. If our numbers fall outside CommitLLM's published 7B/8B envelope (around `L_inf <= 10`, `frac<=1 >= 99.8%`), the writeup MUST state exactly where and why and either tighten the tolerance for the 1B path or escalate the question to CommitLLM upstream.
- FR-15. The system MUST tamper-test in CI: a deliberately corrupted receipt (single-byte flip) MUST be rejected by the verifier, and the failure phase MUST be surfaced in the report. CI runs at least 100 distinct single-byte flips on every PR; 100% MUST be rejected.
- FR-16. The system MUST support a documented receipt format (versioned, binary, with a magic prefix) such that an external party can identify a receipt without parsing the inference response. The format is whatever CommitLLM v4 receipt format is at the pinned commit; we document the version.
- FR-17. The keygen CLI MUST be deterministic given (model checkpoint hash, seed): a third party running the same command on the same model MUST get the same verifier key bytes. Inherited from CommitLLM; we re-test in our CI on every PR.
- FR-18. The repository MUST include reproducible scripts to run the full end-to-end demo on a GPU backend. The reference path is HF Inference Endpoints; self-hosted GPU is documented; Modal compatibility is mentioned but not validated in CI.

## 8. Non-Functional Requirements

- NFR-1. Cold-start CLI verification time (process launch through full-tier single-token audit) MUST be under 1 second on a 2023-class laptop CPU. Routine-tier MUST be under 200 milliseconds. Numbers are 5 times tighter than the original draft because the model is 8 times smaller.
- NFR-2. Verifier-key binary size MUST be under 10 MB for Llama 3.2 1B W8A8. CommitLLM's Llama 3.1 8B key is around 50 MB; the key scales with shell matrix size, which scales with parameter count, so under 10 MB is a defensible envelope. We measure and revise if needed.
- NFR-3. Receipt size for a typical 256-token response MUST be under 100 KB. Half the original draft's bound because the model is smaller; per-layer audit payload shrinks proportionally.
- NFR-4. The Rust CLI binary MUST be under 50 MB stripped on Linux x86_64. Half the original draft's bound; smaller model means we can drop large lookup tables that CommitLLM's 8B path needs.
- NFR-5. The verifier MUST run on CPU only on the client side. No GPU dependency at verification time. CommitLLM property; we enforce in our build.
- NFR-6. The verifier MUST fail closed on unknown receipt versions, unknown model identities, unsupported tier requests, and any structural validation failure. No silent success on partial data.
- NFR-7. The system MUST log structured events (JSON) at every verification phase boundary for debugging and audit reproducibility. Logs are off by default; opt-in via `RUST_LOG=verifiable_intelligence=info` or a `--log` flag.
- NFR-8. The repository MUST run a CI pipeline that, on every PR, builds the Rust workspace, runs unit tests, runs a fixture-based verification end-to-end test against a stored receipt and key, and runs the tamper-detection fuzz harness (100 byte flips). GPU-backed corridor measurement runs on demand (manually triggered) not every PR, because GPU time has a budget.
- NFR-9. The MIT license MUST apply, matching CommitLLM upstream, so the work is mechanically distributable as a dependent and demonstration project.
- NFR-10. All public-facing artifacts (README, docs, CLI help text, error messages, TUI labels) MUST avoid marketing language, MUST avoid "proof" in contexts where the underlying mechanism is bounded-approximate or statistical, and MUST surface protocol limitations on first encounter. The preamble is part of the product.
- NFR-11. The provider Docker image MUST run on a single GPU with at least 16 GB VRAM. Target hardware classes: L4 (24 GB), A10G (24 GB), T4 (16 GB), A100 (80 GB) for headroom. Llama 3.2 1B W8A8 needs roughly 1.5 GB for weights plus runtime overhead; 16 GB is comfortable.

## 9. Success Metrics

- SM-1. Time-to-first-verified-call on a clean machine, measured by a fresh-environment CI job. Baseline: untimed (path does not exist today). Target: under 10 minutes including all install steps and a `vi keygen` plus `vi chat` plus `vi verify` sequence against the demo endpoint.
- SM-2. CLI full-tier verification latency on commodity laptop hardware. Baseline: CommitLLM's ~10 ms server-class measurement on 8B (1B should be faster). Target: under 1 second on a 2023-class laptop including process launch, key load, and JSON serialization.
- SM-3. Tamper-detection rate in CI fuzz harness: every targeted single-byte flip across the audit payload MUST cause rejection. Baseline: CommitLLM existing tamper test passes 1/1. Target: 100% rejection across 100 random single-byte flips on every PR; 1000 flips on a nightly run.
- SM-4. Corridor measurement quality on Llama 3.2 1B W8A8. Baseline: unmeasured. Target: `frac<=1 >= 99.5%` across all three workloads, with a published tolerance envelope. If the number is materially worse than CommitLLM's 7B/8B numbers, we publish the gap honestly and either tighten the tolerance or escalate to CommitLLM upstream.
- SM-5. External readers who can describe the protocol's boundary correctly after reading the README, sampled from 5 reviewers we identify and ask. Baseline: untested. Target: 5/5 correctly identify (a) open-weights only, (b) interactive challenge required, (c) attention corridor is empirical not exact. Pre-release gate.
- SM-6. Demo experience signal: the TUI tamper demonstration produces a visibly different (red vs green) result that an audience member with no cryptography background can describe back. Validated by showing the TUI to 3 non-cryptographers and asking what they saw. Target: 3/3 correctly describe the green-then-red transition. Pre-release gate.
- SM-7. GitHub stars at 30 days post public release. Baseline: 0. Target: 50. Lower than the original draft because the project is one milestone in a roadmap, not a complete platform. Tracked, not gating.

## 10. Constraints & Assumptions

- C1. CommitLLM remains the upstream protocol implementation. We track its main branch, pin to a known-good commit, and document the pin in `commitllm.lock` (or equivalent). Any breaking change upstream forces a documented version bump on our side.
- C2. The supported model in v1 is Llama 3.2 1B Instruct W8A8. We are responsible for corridor validation on this model (G3, FR-13). We are not extending CommitLLM's prover to architectures CommitLLM does not support; if the Llama 3.2 1B config materially diverges from Llama 3.1 8B (RoPE scaling, tied embeddings, attention layout), we either configure CommitLLM to handle the divergence or escalate upstream.
- C3. The reference GPU backend is a Hugging Face Inference Endpoint running our Docker image as a custom container, deployed via the `hf` CLI. Self-hosted GPU is documented and supported via `docker compose`. Modal compatibility is preserved but not validated.
- C4. The Rust toolchain target is stable Rust (no nightly features). MSRV matches CommitLLM's MSRV plus our additional crates' constraints, recorded in `rust-toolchain.toml`.
- C5. The work is discreet: published under `AbdelStark/verifiable-intelligence` on GitHub by default unless explicitly redirected to a StarkWare-owned org (see OQ-1).
- C6. The MIT license applies, matching CommitLLM upstream.
- C7. The repository assumes the developer's machine is x86_64 Linux or arm64 macOS. Windows is best-effort. ARM Linux (Raspberry Pi class) is out of scope.
- C8. The provider's GPU image is built from a single Dockerfile. We assume Docker BuildKit is available; multi-stage builds are used to keep the final image under 8 GB.

## 11. Open Questions

- OQ-1. **Repository organization**: under `AbdelStark/verifiable-intelligence` (personal, discreet) or `starkware-libs/verifiable-intelligence` (institutional, higher signal)? Default for v1: personal. Owner: Abdel. Resolution trigger: before public release.
- OQ-2. **Llama 3.2 1B W8A8 availability**: as of the PRD date, the model may or may not be pre-quantized in W8A8 on Hugging Face under a maintained checkpoint. If yes, we pin that checkpoint. If no, we run `llm-compressor` ourselves and publish the result under `AbdelStark/Llama-3.2-1B-Instruct-quantized.w8a8` (or equivalent). Owner: Abdel. Resolution trigger: 1-week spike at start of build phase. Default if unresolved: quantize ourselves.
- OQ-3. **Receipt API header convention**: `Accept: application/json+receipt-v1` (content-negotiated), `X-Verifiable-Receipt: 1` (explicit opt-in header), or a query parameter? Spec-phase decision. Owner: Abdel. Resolution trigger: spec phase.
- OQ-4. **CommitLLM rename status**: the CommitLLM upstream is in the process of renaming internal `verilm` crates to `commitllm` (their roadmap item #49). Pin to a pre-rename commit, wait for the rename, or vendor and rename ourselves. Owner: Abdel. Resolution trigger: contact lambdaclass or read CommitLLM main HEAD at start of build phase. Default: pin pre-rename and track the rename PR.
- OQ-5. **HF Inference Endpoints custom-container limits**: HF endpoints have RAM, GPU, and startup-time limits that may interact with our image size and cold-start time. If they bite, we may need to publish a slimmer image or fall back to self-hosted as the reference deployment. Owner: Abdel. Resolution trigger: 1-week spike at start of build phase. Default if unresolved: ship self-hosted as the reference and document HF as "may require tuning."
- OQ-6. **Public demo endpoint**: do we leave an HF endpoint running publicly (with rate limiting and abuse controls) so anyone can `vi chat` against it, or do we ship "run it yourself" only? Cost is the main concern; an HF endpoint on an L4 idles cheaply but not freely. Owner: Abdel. Resolution trigger: 2 weeks before public release. Default if unresolved: ship self-hosted-only and add a hosted demo if budget allows.
- OQ-7. **Corridor escalation policy if numbers diverge**: if the Llama 3.2 1B W8A8 corridor falls outside CommitLLM's published 7B/8B envelope, do we (a) publish the gap honestly and tighten our advertised tolerance, (b) treat it as a CommitLLM upstream issue and contribute the measurements back, or (c) both? Owner: Abdel. Resolution trigger: when corridor measurement returns. Default: both.

## 12. Out of Scope (Future Work)

### v1.1: WASM Browser Verifier (planned, scoped)

A browser-based WASM verifier and a single-page demo where any visitor can paste a receipt, load the verifier key, and see a verification report rendered in the browser. CPU-only on the user side. Requires: the CommitLLM verifier crate compiles cleanly to `wasm32-unknown-unknown` (assumed feasible because the verifier is CPU-only Rust; to be validated by a spike at the start of v1.1); a binary key under 10 MB is small enough for first-visit download and trivial after browser cache; a demo HTML page (no framework) that drives `wasm-bindgen` glue. v1.1 ships after v1 is publicly released and the v1 surface is stable. Target latency: under 10 seconds page-load-to-result on second visit, under 30 seconds first visit (key download dominates).

### v1.2: Batched Compliance Flow (planned, scoped)

A batched flow where a provider bundles N receipts into a signed evidence package against a single verifier key, and `vi verify-batch` sample-audits the bundle and produces a structured report. Targeted at a Compliance Engineer persona. Requires: a bundle file format (receipts plus Merkle root plus model identity hash plus verifier-key hash plus time window plus provider signature); three audit modes (sampled, full, targeted); a reproducible report format that an auditor can verify independently; honest sampling math in the report output. v1.2 ships after v1.1. The EU AI Act framing in the demo is explicitly illustrative; no regulator has cleared the format.

### True future work (not scoped, may never ship)

Closed-weight model verification (impossible under the underlying protocol). Zero-knowledge-proof variants as a replacement for CommitLLM. Fine-tuned, LoRA-adapted, or merged-weight model support (CommitLLM upstream item #80; inherited when it lands). Encrypted or selectively disclosed receipts (CommitLLM upstream item #81). Cross-provider receipt portability or a standardization effort. Support for non-text modalities (image, audio, video, embeddings). TypeScript, Go, Java, Swift, Kotlin SDKs. A regulator-facing web dashboard. Integration with public transparency logs (OTS, Sigstore, on-chain anchoring) for batched receipts. An OpenAI-compatible proxy that injects receipts (CommitLLM upstream item #78). Multi-model support in v1 (Qwen 2.5 1.5B, Llama 3.2 3B, Mistral, etc.). Possible v1.x once v1 is stable; not promised.

## 13. Risks

- R1. **Corridor surprise on Llama 3.2 1B W8A8** (technical, medium). The model has not been measured by CommitLLM. Our measurement could reveal numbers materially worse than 7B/8B (`frac<=1 < 99%`, `L_inf` growing with context, tokenizer or RoPE quirks breaking attention replay). Mitigation: budget for it (G3, FR-13 are explicit work); fallback path is to escalate to Llama 3.2 3B (also small enough to be useful) or contribute the measurements back to CommitLLM upstream as a third-family-or-smaller-size control (their roadmap items #7 and #8). Accepted: this is the v1 critical-path risk; if mitigation fails, v1 ships on Llama 3.2 3B and the latency / key-size numbers revise.
- R2. **HF Inference Endpoints incompatibility** (technical, medium). Custom-container endpoints may have startup-time limits, registry constraints, or networking quirks that interact badly with our vLLM plus CommitLLM stack. Mitigation: spike OQ-5 at the start of build phase. Fallback: self-hosted is the reference deployment, HF documented as "may require tuning."
- R3. **CommitLLM upstream churn during the rename window** (technical, medium). The upstream is mid-rename and actively developing. Mitigation: pin to a known-good commit, document the pin, watch the rename PR. Accepted: we move with upstream on a published cadence.
- R4. **Model identity confusion** (product, medium). A developer could believe their verified receipt means the API returned a "correct" answer. Mitigation: README and CLI output language is explicit that verification proves identity-and-execution-integrity, not factual correctness. Validated by SM-5.
- R5. **Demo audience misreading the TUI** (product, low). The TUI's green-to-red transition needs to be unmistakable. Mitigation: SM-6 gates release on non-cryptographer comprehension.
- R6. **Tamper-detection gap** (security, low). A bug in our integration could fail to surface a CommitLLM-detected failure. Mitigation: FR-15 and SM-3 require explicit tamper-detection tests in CI plus a per-PR fuzz harness.
- R7. **Confusion between "verifiable inference" and "ZKP-verified inference"** (product, medium). Readers familiar with the broader verifiable-AI conversation may assume the project provides ZKP-class properties. Mitigation: the preamble explicitly contrasts CommitLLM with ZKPs and names the tradeoffs. Validated by SM-5.
- R8. **HF model availability** (operational, low). If Hugging Face removes Llama 3.2 weights, changes licensing, or rate-limits the downloads, our keygen path stalls. Mitigation: once we have a working W8A8 checkpoint, mirror it under our own HF account.

## 14. References

- CommitLLM repository: https://github.com/lambdaclass/CommitLLM (MIT, lambdaclass)
- CommitLLM roadmap: https://github.com/lambdaclass/CommitLLM/blob/main/roadmap.md
- CommitLLM paper: `paper/` directory in the upstream repo (Typst source)
- Upstream behavioural baseline: `scripts/modal/demo_llama_e2e.py` from CommitLLM (provided as input to this PRD; Llama 3.1 8B W8A8 reference)
- Llama 3.2 model card: https://huggingface.co/meta-llama/Llama-3.2-1B-Instruct
- `llm-compressor` (neuralmagic) for W8A8 quantization: https://github.com/vllm-project/llm-compressor
- Hugging Face Inference Endpoints custom container docs: https://huggingface.co/docs/inference-endpoints/guides/custom_container
- VeriFlow specification (internal StarkWare reference, for context on the alternative ZKP path)
- Source draft: `docs/source-draft.md`
- Scope revision context (this PRD rewrite): captured in the v1-scope chat exchange on 2026-05-12
