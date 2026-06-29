# Buyer Proof Guide

This guide explains how a consumer buyer should read a `VIEX` proof bundle in the browser demo.

## What You Can Check

A passing proof bundle means the verifier accepted these bindings for a supported open-weight provider:

| Item | What to inspect | Expected result |
|------|-----------------|-----------------|
| Quote | `provider_id`, `model_id`, `checkpoint_hash`, `key_hash`, `commitllm_pin`, `decode_policy_hash`, expiry | The quote is fresh and names the provider/model you selected. |
| Receipt | `receipt.sha256`, `receipt.bytes_b64` or content-addressed receipt reference | The receipt bytes match the bundle hash and decode under the pinned CommitLLM verifier. |
| Prompt binding | `request.prompt_hash`, `request.input_spec_hash` | The receipt binds the submitted prompt bytes or canonical prompt representation. |
| Decode policy | `quote.decode_policy_hash`, `request.max_tokens` | The verifier binds the sampling and output policy used for the delivered answer. |
| Delivered answer binding | `response.answer_hash`, optional `answer_preview` | The displayed answer matches the committed output path. |
| Verifier identity | `verifier.key_hash`, `verifier.commitllm_pin`, `verifier.verification_mode` | The verifier key and CommitLLM pin match the supported open-weight provider. |

The broker is not a trust root. Treat broker quote signatures as convenience metadata. The proof is useful only when the verifier checks the receipt, verifier key, CommitLLM pin, prompt hash, decode policy, and delivered answer binding.

## Proof Boundary Table

| Guarantee class | Buyer meaning | Examples in v1 |
|-----------------|---------------|----------------|
| Exact | Deterministic check against committed material. | Quote expiry, prompt hash, decode policy hash, delivered answer hash, CommitLLM pin, verifier key hash. |
| Algebraic | Randomized algebraic check with verifier-secret randomness. | Freivalds-style checks inside CommitLLM for supported linear-shell paths. |
| Statistical | Sampled check where coverage depends on challenge selection. | Routine audit layer selection. |
| Audited | Data is committed and opened for inspection, but the computation is not independently re-executed for every position. | CommitLLM audited attention inputs and wiring on stock GPU kernels. |
| Open | Unsupported or unresolved claim. The UI must not render this as verified. | Closed-weight frontier models, arbitrary-position attention output verification, factual correctness. |

## What A Pass Does Not Mean

- It does not prove the answer is factually correct, safe, or useful.
- It does not prove a closed-weight model such as GPT, Claude, or Gemini ran.
- It does not prove the provider is licensed, solvent, or compliant outside this inference call.
- It does not make the broker trustworthy.
- It does not authorize credential pooling, account sharing, provider-term evasion, or resale of third-party API access.

## Red Paths

The demo intentionally includes failure cases a buyer should understand:

- **Fake-model provider:** the quote names one model, but the receipt/key evidence binds another.
- **Prompt mismatch:** the receipt binds a different prompt hash than the buyer submitted.
- **Answer rewrite:** the displayed answer hash differs from the committed output path.
- **Expired quote:** the quote is outside its validity window.
- **Wrong key:** the verifier key hash does not match the provider claim.
- **Receipt tamper:** receipt bytes do not match the receipt hash or fail CommitLLM decoding.
- **Unsupported closed-weight provider:** no public checkpoint or compatible verifier key exists, so the result is `unsupported`.

## Comprehension Check

A reader should be able to answer these before trusting the demo:

1. Does v1 support closed-weight frontier model verification? No.
2. Does v1 accept third-party API keys or support credential resale? No.
3. Does a passing proof mean the answer is true? No.
4. What does v1 verify? Execution integrity for supported open-weight paths: model/checkpoint/key identity, prompt binding, decode policy, delivered answer binding, receipt integrity, and the supported CommitLLM checks.
