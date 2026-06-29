# RFC-0010: Corridor measurement methodology

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1 → v1.0 (numbers go in README at v1.0)

## Summary

The project produces a reproducible measurement of the CommitLLM attention corridor on Llama 3.2 1B Instruct W8A8 across three workloads (short-answer factual, multi-turn reasoning, long-context code), all layers, on real GPU hardware. Metrics published: global `L_inf`, first-generated-token max, decode max, `frac_eq`, `frac<=1`, and growth-vs-context. The measurement script lives at `scripts/corridor/measure.py`; output is a JSON report committed to `reports/corridor/`. If our numbers fall outside CommitLLM's published 7B/8B envelope, we publish the gap honestly and either tighten the published tolerance or escalate upstream (PRD OQ-7 — we do both).

**Pivot note, 2026-06-29:** after [RFC-0016](./RFC-0016-marketplace-demo-pivot.md), this is research backlog unless Llama 3.2 1B becomes the chosen live marketplace model. v1 should first prefer a CommitLLM-supported measured model.

## Motivation

The corridor is the empirical bound at the center of CommitLLM's protocol. CommitLLM has published it for 7B/8B; nobody has published it for 1B. This project's only protocol-adjacent original contribution is exactly this measurement ([PRD G3, G8](../../PRD.md)). It must be reproducible by a third party; the numbers must be defensible by reference to a script in the repository.

## Goals

- Three workloads with explicit prompt sets.
- All layers covered (no sampling at the layer level).
- Real GPU hardware (A10G, L4, or A100; documented per run).
- Reproducible: a documented command + the same workload + the same model checkpoint + the same CommitLLM pin → the same numbers (within float-tolerance bounds we document).
- Output schema versioned and committed to the repo.
- Pass criterion: `frac<=1 >= 99.5%` across all three workloads ([SM-4](../../PRD.md)).

## Non-Goals

- Not measuring other model sizes or families in v1 ([PRD NG12](../../PRD.md)).
- Not running corridor measurement on every PR (GPU budget; ([RFC-0013](./RFC-0013-ci-pipeline.md)) covers the on-demand model).
- Not optimizing the corridor numbers by changing decode policy. We measure what the deployed serving configuration produces.

## Proposed Design

### Workloads

Each workload is a JSON file under `scripts/corridor/workloads/`:

- `short-factual.jsonl`: 200 prompts, each ≤ 50 tokens. Examples: "What causes rainbows?", "Capital of Paraguay?", "Define photosynthesis in one sentence."
- `multi-turn-reasoning.jsonl`: 100 conversations × 3 turns each. Mixed arithmetic, logical inference, definitional follow-up.
- `long-context-code.jsonl`: 50 prompts with 4–8 KB context window (real code snippets), each asking for a small extension.

Workloads are committed to the repo. They are part of the measurement contract: changing them is a versioned change.

### Reference (teacher) computation

- fp16 reference forward pass on the same hardware, same checkpoint, same RoPE configuration, on a separate vLLM-free path.
- Captures per-layer attention outputs at the same token positions the prover instruments.

### Prover (deployed) computation

- The provider container running the W8A8 W8A8 deployed configuration; same vLLM revision pinned by [RFC-0011](./RFC-0011-commitllm-upstream-pinning.md).
- Captures per-layer attention outputs through CommitLLM's hook.

### Comparison

Elementwise diff per layer per token position. Aggregate metrics per workload:

- `L_inf`: max absolute deviation across all positions, layers, dims.
- `L_inf_first_gen_token`: max absolute deviation at the position of the first generated token.
- `L_inf_decode_max`: max absolute deviation during decode (across all generated tokens).
- `frac_eq`: fraction of paired elements with deviation `0`.
- `frac_le_1`: fraction of paired elements with deviation `<= 1` (in the quantized representation's unit).
- `growth_vs_context`: a list of `(context_length, L_inf)` pairs over a sweep.

### Tooling

- `scripts/corridor/measure.py`: orchestrates teacher and prover runs, aggregates, emits JSON.
- `scripts/corridor/aggregate.py`: merges per-workload JSONs into a single summary.
- `scripts/corridor/plot.py` (optional): generates PNGs for the README.

Implementation uses CommitLLM upstream's measurement utilities where available; we do not re-implement what they ship.

### Output schema

```json
{
  "schema_version": 1,
  "timestamp_utc": "2026-05-12T10:15:42Z",
  "model_id": "llama-3.2-1b-w8a8",
  "checkpoint_hash": "sha256:...",
  "commitllm_pin": "<short-sha>",
  "gpu": "A10G",
  "vllm_version": "...",
  "workloads": {
    "short-factual": {
      "n_prompts": 200,
      "l_inf": 7,
      "l_inf_first_gen_token": 4,
      "l_inf_decode_max": 7,
      "frac_eq": 0.9821,
      "frac_le_1": 0.99876,
      "growth_vs_context": [[64, 3], [128, 5], [256, 7]]
    },
    "multi-turn-reasoning": { "...": "..." },
    "long-context-code": { "...": "..." }
  },
  "verdict": "inside_envelope|outside_envelope|borderline",
  "notes": "..."
}
```

The `verdict` field is computed by comparing to CommitLLM's published 7B/8B envelope (`L_inf <= 10`, `frac_le_1 >= 99.8%`):

- `inside_envelope`: all workloads pass the envelope and SM-4.
- `borderline`: SM-4 passes (`frac_le_1 >= 99.5%`) but not the upstream envelope.
- `outside_envelope`: SM-4 fails on at least one workload.

### Escalation policy (resolves PRD OQ-7)

- `inside_envelope`: README publishes the numbers, no further action.
- `borderline`: README publishes the numbers AND a tightened, project-published tolerance (e.g. "verified within `L_inf <= 15`, `frac_le_1 >= 99.5%` for 1B"). An issue is filed against CommitLLM upstream with the measurements as a data point.
- `outside_envelope`: v1 escalates to Llama 3.2 3B per [PRD R1](../../PRD.md) fallback; we re-measure on 3B; the 1B measurement is published as a negative result with the gap analysis.

In all three cases the numbers are published. We do not suppress measurements that disagree with upstream.

### Reproducibility

- The exact `measure.py` invocation is in `docs/measurements/corridor.md` (to be authored under the corridor docs issue).
- The workload files, the model checkpoint hash, and the CommitLLM pin together fully specify the run.
- A third party with the same hardware should get within floating-point tolerance of the same numbers; we document the expected tolerance.

## Alternatives Considered

**Sample layers instead of measuring all.** Rejected: CommitLLM upstream measures all layers; we follow.

**Run only one workload (the simplest).** Rejected: a single workload could hide growth issues; three workloads is the minimum that tests short, multi-turn, and long-context behavior.

**Run corridor measurement in CI on every PR.** Rejected: GPU budget; the measurement is not a per-PR signal; it is a release-quality signal. Per-PR is replaced by a smoke test on a single fixed prompt to catch obvious drift ([RFC-0013](./RFC-0013-ci-pipeline.md)).

**Publish only summary numbers, not raw data.** Rejected: third-party reproducibility requires the workloads and the script.

## Drawbacks

- A GPU run takes minutes-to-hours depending on hardware. We accept this; it's on-demand.
- If our numbers diverge sharply from CommitLLM's, the project's launch story changes. Mitigation: the escalation policy is explicit; we plan for divergence.

## Migration / Rollout

- The measurement script lands in v0.1, when the provider image is buildable on real GPU.
- The first measurement is the v0.1 milestone close. Numbers may shift as we iterate; the published README numbers are the v1.0 measurement.

## Testing Strategy

- Unit tests for `measure.py` aggregation logic (no GPU required).
- A "tiny" corridor test in CI: 5 short prompts, all layers, on a CPU CommitLLM debug build (if upstream supports it); checks that the script runs end-to-end and emits valid JSON.
- A measurement-quality gate at release: SM-4 check.

## Open Questions

- **OQ-7**: corridor escalation policy. **Resolved by this RFC**: publish the gap AND escalate upstream. If outside envelope, fall back to Llama 3.2 3B.

## References

- [PRD §4 G3, §7 FR-13, FR-14, §9 SM-4, §11 OQ-7, §13 R1](../../PRD.md)
- CommitLLM published 7B/8B corridor numbers (in the upstream paper and `paper/` directory).
- [07-testing-strategy.md §6](../spec/07-testing-strategy.md)
