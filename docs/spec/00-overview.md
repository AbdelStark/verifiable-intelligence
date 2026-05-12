# 00 — Overview

## Thesis

The integrity gap between what an inference provider claims it ran and what a consumer can independently check is the load-bearing problem in verifiable AI. The CommitLLM protocol closes that gap for open-weight models under interactive challenge. `verifiable-intelligence` ships the smallest possible developer-facing surfaces on top of CommitLLM such that a backend engineer can adopt the verification step in one command and a demo audience can watch the protocol work in real time.

This project is not a protocol. It is an integration artifact and a developer experience layer.

## Goals (v1)

Each goal is testable. The success metric in parentheses is from [PRD §9](../../PRD.md).

- **G1 — End-to-end working demo.** Llama 3.2 1B Instruct W8A8 served from HF Inference Endpoints, `vi chat` → receipt → `vi verify` → structured report, no client-side GPU. (SM-1)
- **G2 — Working TUI with tamper demonstration.** Phase walk visible, tampered receipt visibly fails at the breaking phase. (SM-6)
- **G3 — Corridor validated on Llama 3.2 1B W8A8.** Three workloads, all layers, real GPU, reproducible script, numbers published in README and writeup. (SM-4)
- **G4 — Time-to-first-verified-call under 10 minutes** on a fresh developer machine, measured by a CI job that runs the install-to-verify sequence in a clean container. (SM-1)
- **G5 — CLI full-tier verification under 1 s, routine-tier under 200 ms** on a 2023-class laptop CPU. (SM-2)
- **G6 — Vendor-neutral provider deployment.** Single Docker image, HF + self-hosted + (best-effort) Modal recipes; adding a target is a docs task. ([RFC-0005](../rfcs/RFC-0005-provider-image.md), [RFC-0007](../rfcs/RFC-0007-hf-deployment-recipe.md))
- **G7 — Honest preamble.** README, CLI help, TUI labels surface protocol bounds on first encounter; reviewer comprehension gated. (SM-5)
- **G8 — Zero new cryptography.** All verification logic comes from CommitLLM crates; the only protocol-adjacent original work is corridor measurement.

## Non-goals (v1)

Inherited verbatim from [PRD §5](../../PRD.md). The load-bearing ones:

- No new proof system. No modifications to the CommitLLM protocol.
- No closed-weight model support. Property of the underlying protocol.
- No production-grade hosted inference service.
- No WASM verifier (v1.1). No batched compliance flow (v1.2).
- No Python SDK in v1. No multi-model support in v1.
- No fine-tuned / LoRA / merged-weight variants.
- No regulator dashboard.
- No privacy-preserving receipt variants.

If a contributor proposes work that conflicts with any non-goal, the PR is closed with a pointer to PRD §5.

## Personas (in scope for v1)

- **Integrating Developer.** Builds against the CLI, integrates into application code and CI.
- **Demo Audience.** Watches the TUI in screencasts, talks, and live walkthroughs.

Deferred personas — Browser User (v1.1) and Compliance Engineer (v1.2) — are named for scope clarity. v1 must not constrain their design unnecessarily but is not required to support them.

## Success criteria (release gates)

v1 ships when, and only when, all of:

1. **SM-1.** Time-to-first-verified-call CI job passes under 10 minutes wall clock from a clean container.
2. **SM-2.** Full-tier verification latency benchmark under 1 s p95 on a documented commodity laptop reference; routine-tier under 200 ms p95.
3. **SM-3.** 100% rejection across 100 random single-byte flips on every PR run for two consecutive weeks; 1000 flips on the nightly run for one week.
4. **SM-4.** Corridor numbers published in README; either inside CommitLLM's published 7B/8B envelope (around `L_inf <= 10`, `frac<=1 >= 99.8%`) or with the gap documented and the published tolerance tightened. Three workloads, all layers, reproducible script. (Failure mode and fallback in [RFC-0010](../rfcs/RFC-0010-corridor-measurement.md).)
5. **SM-5.** 5/5 external reviewers correctly describe (a) open-weights only, (b) interactive challenge required, (c) attention corridor is empirical not exact, after reading only the README.
6. **SM-6.** 3/3 non-cryptographers correctly describe the TUI green-then-red transition.

SM-7 (50 stars at 30 days) is tracked, not gating.

## What this corpus does and does not contain

This corpus is the implementation contract. It is sufficient detail to file shippable issues against. It is not a user manual; user-facing documentation is delivered as part of the work tracked by docs issues, derived from this corpus.

This corpus does not duplicate the CommitLLM protocol specification. Where behaviour is inherited from CommitLLM, the corpus names the upstream contract and the pin (see [RFC-0011](../rfcs/RFC-0011-commitllm-upstream-pinning.md)), it does not re-specify the protocol.

## Versioning

- **v0.1** — provider image + `vi keygen` + `vi chat` + `vi verify` against fixtures; corridor measurement script; CI green; not publicly released.
- **v0.2** — `vi tui`; tamper fuzz harness in CI; HF deployment recipe; reviewer comprehension gate.
- **v1.0** — public release; all SM gates green; binaries published; corridor numbers in README.

Detailed semver and release policy in [09-release-and-versioning.md](./09-release-and-versioning.md).
