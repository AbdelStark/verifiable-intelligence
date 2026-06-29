# Corridor Measurement Template

- Status: template, no corridor run has landed in this repository.
- Source RFC: [`../rfcs/RFC-0010-corridor-measurement.md`](../rfcs/RFC-0010-corridor-measurement.md).
- Current pivot note: corridor measurement is research backlog unless the measured
  model becomes the live marketplace model. The v1 demo currently prefers the
  CommitLLM-supported `llama-3.1-8b-w8a8` reference path documented in
  [`../spikes/reference-model.md`](../spikes/reference-model.md).

This document is the fill-in surface for a reproducible CommitLLM attention
corridor run. It intentionally does not claim numbers yet.

## Prerequisites

- `scripts/corridor/measure.py` exists and matches RFC-0010.
- Workloads exist under `scripts/corridor/workloads/`:
  - `short-factual.jsonl`
  - `multi-turn-reasoning.jsonl`
  - `long-context-code.jsonl`
- A real GPU runner is available and documented.
- The model checkpoint, checkpoint hash, CommitLLM pin, verifier key identity,
  vLLM revision, and provider image digest are fixed before the run.
- The output path under `reports/corridor/` is ignored during local iteration and
  committed only once the run is reviewed.

## Intended Command

This command is the RFC-0010 shape. It is not runnable until
`scripts/corridor/measure.py` lands.

```bash
python scripts/corridor/measure.py \
  --model-id llama-3.2-1b-w8a8 \
  --checkpoint /models/llama-3.2-1b-w8a8 \
  --checkpoint-hash sha256:<checkpoint-hash> \
  --commitllm-pin 25541e83 \
  --provider-image ghcr.io/abdelstark/verifiable-intelligence-provider@sha256:<digest> \
  --teacher fp16 \
  --prover w8a8 \
  --workload scripts/corridor/workloads/short-factual.jsonl \
  --workload scripts/corridor/workloads/multi-turn-reasoning.jsonl \
  --workload scripts/corridor/workloads/long-context-code.jsonl \
  --all-layers \
  --context-sweep 64,128,256,512,1024,2048 \
  --gpu <a10g|l4|a100> \
  --output reports/corridor/<yyyy-mm-dd>-llama-3.2-1b-w8a8.json
```

If the current v1 reference model is measured instead, replace the model ID and
checkpoint path with `llama-3.1-8b-w8a8` and keep the rest of the template
structure unchanged.

## Run Metadata

Fill this table for every committed report.

| Field | Value |
|-------|-------|
| Date UTC | `TODO` |
| Operator | `TODO` |
| Host | `TODO` |
| GPU model | `TODO` |
| GPU count | `TODO` |
| Driver/CUDA | `TODO` |
| Browser/OS if browser tooling used | `TODO` |
| Model ID | `TODO` |
| Checkpoint hash | `sha256:TODO` |
| CommitLLM pin | `TODO` |
| Provider image digest | `sha256:TODO` |
| vLLM revision | `TODO` |
| Teacher path | `TODO` |
| Prover path | `TODO` |
| Output report | `reports/corridor/TODO.json` |

## Reproducibility Tolerance

For the same checkpoint, CommitLLM pin, workloads, GPU model, driver/CUDA stack,
and provider image digest:

- `l_inf`, `l_inf_first_gen_token`, and `l_inf_decode_max` should match exactly.
- `frac_eq` and `frac_le_1` should match within `0.0001` absolute difference,
  allowing only serialization or aggregation-order drift.
- `growth_vs_context` context lengths should match exactly; each measured
  `l_inf` value should match exactly.

For the same model and pin on a different machine in the same GPU family:

- `l_inf` metrics may drift by at most `1` quantized unit before the run needs a
  reproduction note.
- `frac_eq` and `frac_le_1` may drift by at most `0.0005` absolute difference.
- Any verdict change (`inside_envelope`, `borderline`, `outside_envelope`) must be
  treated as a failed reproduction until explained.

If a run exceeds these tolerances, commit the numbers only with a gap analysis
and a follow-up issue.

## Expected Report Shape

The committed JSON report should include at least:

```json
{
  "schema_version": 1,
  "timestamp_utc": "TODO",
  "model_id": "TODO",
  "checkpoint_hash": "sha256:TODO",
  "commitllm_pin": "TODO",
  "gpu": "TODO",
  "vllm_version": "TODO",
  "workloads": {
    "short-factual": {
      "n_prompts": 0,
      "l_inf": 0,
      "l_inf_first_gen_token": 0,
      "l_inf_decode_max": 0,
      "frac_eq": 0.0,
      "frac_le_1": 0.0,
      "growth_vs_context": []
    },
    "multi-turn-reasoning": {
      "n_prompts": 0,
      "l_inf": 0,
      "l_inf_first_gen_token": 0,
      "l_inf_decode_max": 0,
      "frac_eq": 0.0,
      "frac_le_1": 0.0,
      "growth_vs_context": []
    },
    "long-context-code": {
      "n_prompts": 0,
      "l_inf": 0,
      "l_inf_first_gen_token": 0,
      "l_inf_decode_max": 0,
      "frac_eq": 0.0,
      "frac_le_1": 0.0,
      "growth_vs_context": []
    }
  },
  "verdict": "inside_envelope",
  "notes": "TODO"
}
```

Allowed verdicts:

- `inside_envelope`: all workloads match the upstream comparison envelope and
  project pass criterion.
- `borderline`: project pass criterion holds, but the upstream comparison
  envelope is missed.
- `outside_envelope`: at least one workload misses the project pass criterion.

## Fill-In Results

Replace this section when a real report lands.

| Workload | Prompts | L_inf | First-token L_inf | Decode L_inf | frac_eq | frac_le_1 | Verdict |
|----------|---------|-------|-------------------|--------------|---------|-----------|---------|
| short-factual | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` |
| multi-turn-reasoning | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` |
| long-context-code | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` | `TODO` |

## Publication Checklist

- [ ] Commit the exact command used.
- [ ] Commit the report JSON under `reports/corridor/`.
- [ ] Record every workload file hash.
- [ ] Record checkpoint hash and provider image digest.
- [ ] Compare the run against the reproducibility tolerance above.
- [ ] If `borderline` or `outside_envelope`, file an upstream or local follow-up
      with the report attached.
- [ ] Update public docs with the numbers, or explicitly state why the corridor
      run remains research backlog.
