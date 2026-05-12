# 08 — Performance Budget

This document fixes the latency, memory, and size budgets for v1, names the reference hardware, and specifies how each number is measured. Budgets are gating; a regression beyond the documented margin fails CI.

## Reference hardware

| Class | Description | Used for |
|-------|-------------|----------|
| Reference laptop | 2023-class consumer laptop, 8 P-core x86_64 or M-series ARM, no GPU | CLI latency, key load, verify benchmarks |
| Reference CI runner | GitHub Actions `ubuntu-latest`, 4 vCPU x86_64, 16 GB RAM | CI integration tests, fresh-env timing |
| Reference GPU | A10G or L4 with 24 GB VRAM | Provider container, corridor measurement |

Latency budgets are stated against the **reference laptop**. CI runners are slower; CI tests check correctness, not latency, except where explicitly noted.

## Latency budgets

| Surface | Operation | p95 budget | Margin | Source |
|---------|-----------|------------|--------|--------|
| CLI | `vi verify --tier full` single-token audit, key already on disk | < 1.0 s | 10% (fail at 1.1 s) | [NFR-1, SM-2](../../PRD.md) |
| CLI | `vi verify --tier routine` | < 200 ms | 10% (fail at 220 ms) | [NFR-1, SM-2](../../PRD.md) |
| CLI | `vi keygen` against a cached checkpoint | < 60 s | 25% (fail at 75 s) | [PRD §6 Journey A](../../PRD.md) |
| CLI | Fresh-env time-to-first-verified-call (install → chat → verify) | < 10 min | flat (fail at 10 min CI runtime) | [SM-1](../../PRD.md) |
| Provider | `/healthz` response | < 100 ms | flat | operational |
| Provider | Cold start (container up → `/healthz` 200) | < 90 s | 25% (fail at 112 s) | HF endpoint constraint |
| TUI | Phase walk total wall clock (no `--phase-delay`) | matches `vi verify` plus < 100 ms render overhead | 25% | operational |

## Size budgets

| Artifact | Budget | Margin | Source |
|----------|--------|--------|--------|
| Verifier key for Llama 3.2 1B W8A8 | < 10 MB | 10% (fail at 11 MB) | [NFR-2](../../PRD.md) |
| Receipt for 256-token response | < 100 KB | 10% (fail at 110 KB) | [NFR-3](../../PRD.md) |
| CLI binary (Linux x86_64 stripped) | < 50 MB | 10% (fail at 55 MB) | [NFR-4](../../PRD.md) |
| CLI binary (macOS arm64 stripped) | < 60 MB | 10% (fail at 66 MB) | derived, Mach-O overhead |
| Provider Docker image (final stage) | < 8 GB | 10% (fail at 8.8 GB) | [PRD C8](../../PRD.md) |

## Memory budgets

| Surface | Working-set budget | Source |
|---------|--------------------|--------|
| `vi verify` on commodity laptop | < 256 MB resident peak | operational; CommitLLM verifier is small |
| `vi keygen` | < 4 GB resident peak (model bytes streamed) | operational |
| Provider container (model weights + vLLM) | < 12 GB VRAM in steady state at default batch | sized below the 16 GB GPU class floor ([NFR-11](../../PRD.md)) |

## Network budgets

| Path | Budget | Note |
|------|--------|------|
| `vi chat` 256-token response payload | < 200 KB | text + multipart receipt |
| `vi keygen` checkpoint download | < 2 GB | W8A8 Llama 3.2 1B in safetensors |
| `vi verify --tier full` audit request | < 32 KB request, < 200 KB response | upper bound from CommitLLM audit blob sizing |

## Measurement methodology

### Per-PR (correctness; not latency-gated)

- CI runs integration tests and the tamper fuzz harness; latency is recorded but not gated.
- Size budgets ARE gated per PR. A binary or key that regresses past margin fails the PR.

### Nightly (latency-gated)

- Latency benchmarks run on a documented runner class (GitHub Actions large runner or a dedicated VM, decided in [RFC-0013](../rfcs/RFC-0013-ci-pipeline.md)).
- p95 over N=30 iterations. Warmup discarded.
- Results published to `reports/perf/<date>-<commit>.json`.
- Regression beyond margin opens an issue automatically.

### Release-gate

- Latency benchmarks must pass on the reference laptop class once per release candidate. Run by the release engineer on a documented machine; recorded in the release issue.
- SM-1 fresh-env CI job must pass under 10 minutes for two consecutive runs.

## Where the budgets come from

- CLI latency tightened from PRD's original draft by 5× because the model is 8× smaller; per-phase work scales sub-linearly with parameter count for the audit portion but is bounded by parsing and key-load cost.
- Key size and receipt size are scaled-down envelopes from CommitLLM's 8B numbers. Both will be re-measured on the first end-to-end run on real W8A8 Llama 3.2 1B and the spec corrected if reality disagrees.
- Provider memory budget is well below the 16 GB GPU floor; W8A8 1B is roughly 1.5 GB in weights plus vLLM overhead.

## What the budgets do not promise

- No SLA. The provider is a demonstration deployment.
- No worst-case guarantee. Budgets are p95.
- No promise on Windows performance. Windows is best-effort.
- No promise on ARM Linux. Out of scope.

## Profiling discipline

- Anything failing a latency budget triggers a profiling step (`cargo flamegraph` or equivalent) before the PR is allowed to bypass the budget. "Profile first, fix or document" is the rule.
- Profiles are attached to the PR.
- A budget bump (raising the cap) is allowed only with a written rationale in the PR description and a documented update to this file.
