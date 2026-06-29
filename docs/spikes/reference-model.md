# Reference model decision

- Date: 2026-06-29
- Related issues: #119, #126
- Decision: use Llama 3.1 8B W8A8 as the v1 live reference model.
- CommitLLM pin: `lambdaclass/CommitLLM@25541e83347655e44ad6e84eb901e1e7ae392a66`
- Short pin for bundles: `25541e83`

## Selected profile

| Field | Value |
| --- | --- |
| VIEX `model_id` | `llama-3.1-8b-w8a8` |
| Upstream model ref | `neuralmagic/Meta-Llama-3.1-8B-Instruct-quantized.w8a8` |
| Resolved HF repository | `RedHatAI/Meta-Llama-3.1-8B-Instruct-quantized.w8a8` |
| Resolved HF revision checked | `fc2669564045fda605eac0db50e430d24d9aece1` |
| CommitLLM profile | `llama-w8a8-audited` |
| Decode acceptance | `CapturedLogits` |
| Attention claim | audited inputs and wiring only; arbitrary-position attention outputs are not verified |
| Verifier-key path | `provider/keys/llama-3.1-8b-w8a8/commitllm-25541e83/verifier-key.viky` |
| Decode artifact path | `provider/keys/llama-3.1-8b-w8a8/commitllm-25541e83/decode-artifact.vdec` |
| First live hosting target | Modal A100-80GB |
| Fixture checkpoint hash | `sha256:a41e82812a261708ec253803904d482a00e8d06d9f6d119418f2d9dc3edafeef` |

The fixture checkpoint hash above is a deterministic descriptor hash for static fixtures only. It is not the canonical hash of the full weight snapshot. A live provider build must download the resolved model revision, run the project canonical checkpoint-hash procedure over the actual file set, and replace the fixture hash before any real receipt is advertised as live.

## Candidate comparison

| Candidate | Upstream status | Strengths | Gaps | Decision |
| --- | --- | --- | --- | --- |
| Llama 3.1 8B W8A8 | Maintained E2E surface at current CommitLLM head | QKV Freivalds support, audited product profile, captured-logits decode, lower retained-logits footprint than Qwen in upstream notes, existing Llama demo/adversarial path | Still no arbitrary-position attention verification; A100-class live path first | Select for v1 live reference |
| Qwen2.5 7B W8A8 | Maintained E2E surface at current CommitLLM head | Maintained Qwen E2E, captured-logits decode, useful second-family evidence | QKV Freivalds skipped by Qwen profile because of the bridge gap; retained-logits footprint is higher in upstream notes | Keep as secondary compatibility/model-swap fixture |
| Llama 3.2 1B W8A8 | Project-owned research path only | Smaller and easier to host if the corridor is proven later | Requires project-owned quantization, corridor measurement, and verifier-key validation; not the maintained upstream path today | Move to research backlog, not v1 blocker |

## Source evidence

The current CommitLLM README says the maintained E2E surface covers Qwen2.5-7B-W8A8 and Llama-3.1-8B-W8A8, with binary keys, commitment-derived challenges, greedy and sampled decode, mixed audit tiers, tamper detection, and EOS handling. It also states that arbitrary-position attention outputs are not verified in the shipped product and that the kept stock-mode attention claim is audit-only.

The current CommitLLM profile definitions auto-detect W8A8 Llama checkpoints to `llama-w8a8-audited`. That profile uses `AuditedInputsOnly` attention and `CapturedLogits` decode. The same source records that the older `llama-w8a8` profile supports QKV Freivalds, while Qwen's W8A8 profile sets `supports_qkv_freivalds` to false.

The current CommitLLM E2E scripts run both maintained models on Modal `A100-80GB`. The v1 live path should start there before claiming cheaper L4/A10G hosting.

## Bundle and fixture policy

Canonical `VIEX` fixtures now use:

- `quote.model_id = "llama-3.1-8b-w8a8"`
- `quote.commitllm_pin = "25541e83"`
- `verifier.commitllm_pin = "25541e83"`
- `quote.checkpoint_hash = "sha256:a41e82812a261708ec253803904d482a00e8d06d9f6d119418f2d9dc3edafeef"`

`fixtures/viex/manifest.json` stores those reference fields, and `npm run test:schema` fails if a fixture drifts from them.

## SPEC resolution

SPEC OQ-2 is resolved for v1 provider integration:

> Use `llama-3.1-8b-w8a8` with CommitLLM profile `llama-w8a8-audited` at pin `25541e83`. Qwen remains a maintained secondary candidate and red-path/model-swap fixture. Llama 3.2 1B moves to research backlog until it has an upstream-supported or project-measured corridor.

## Follow-up gates

Before any live provider is advertised:

1. Compute the canonical checkpoint hash from the resolved HF revision and update this document, `fixtures/viex/manifest.json`, and provider health metadata.
2. Generate the verifier key and decode artifact at the selected CommitLLM pin.
3. Record key hash and artifact sizes.
4. Run the upstream `llama-w8a8-audited` E2E path on Modal A100-80GB.
5. Keep the browser/demo language limited to execution integrity and audited attention inputs; do not claim answer correctness or arbitrary-position attention verification.

## References

- [CommitLLM README](https://github.com/lambdaclass/CommitLLM/blob/main/README.md)
- [CommitLLM profile definitions](https://github.com/lambdaclass/CommitLLM/blob/main/crates/verilm-core/src/types.rs)
- [CommitLLM audited-inputs E2E](https://github.com/lambdaclass/CommitLLM/blob/main/scripts/modal/test_audited_inputs_e2e.py)
- [RFC-0016 marketplace demo pivot](../rfcs/RFC-0016-marketplace-demo-pivot.md)
