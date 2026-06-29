# RFC-0012: W8A8 quantization and checkpoint hosting

- Status: Accepted (resolves PRD OQ-2 default path)
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

Until a maintained W8A8 quantization of Llama 3.2 1B Instruct is hosted on Hugging Face under a trustworthy maintainer, this project produces and hosts its own. The quantization uses `llm-compressor` with the same recipe CommitLLM applies to Llama 3.1 8B, parameterized for 1B. The output is published to `AbdelStark/Llama-3.2-1B-Instruct-quantized.w8a8` with a documented canonical hash and a `recipe.yaml` in the repo. If an upstream-maintained W8A8 of the same checkpoint becomes available later, the project switches the canonical pointer to it as a documented MINOR-with-pin-change event.

**Pivot note, 2026-06-29:** after [RFC-0016](./RFC-0016-marketplace-demo-pivot.md), this model-specific quantization plan no longer blocks v1. It remains valid only if Llama 3.2 1B is selected as the live model.

## Motivation

Llama 3.2 1B Instruct W8A8 is the v1 model. The base FP16 checkpoint is on HF; the W8A8 variant may or may not be hosted by a maintainer we trust ([PRD OQ-2](../../PRD.md)). The provider image bakes weights in at build time; `vi keygen` reads them. Both need a stable source. We take responsibility for hosting and reproducibility.

## Goals

- A reproducible `quantize.py` script under `scripts/quantize/` that produces W8A8 bytes from the FP16 checkpoint.
- A canonical hash of the W8A8 output, fixed at the time of upload, pinned in spec and code.
- A documented HF repository under the project's account hosting the mirror.
- A clear switch-over plan if upstream W8A8 becomes available.

## Non-Goals

- We do not maintain quantizations for multiple base models. v1 quantizes exactly one model.
- We do not customize the quantization recipe beyond what `llm-compressor` parameterizes naturally for a 1B model. No exotic calibration sets.
- We do not benchmark perplexity or downstream tasks of the quantized model. CommitLLM's corridor measurement is the only quality signal we publish.

## Proposed Design

### Recipe

`scripts/quantize/quantize.py` (Python, run on a GPU):

1. Load `meta-llama/Llama-3.2-1B-Instruct` (FP16).
2. Apply `llm-compressor` W8A8 recipe matching CommitLLM's 8B recipe, with model-size-appropriate calibration:
   - SmoothQuant alpha as per CommitLLM's published recipe.
   - INT8 per-channel weight quantization.
   - INT8 dynamic activation quantization.
   - Calibration set: 512 samples from the same calibration dataset CommitLLM uses (documented and committed to the repo, or downloaded by hash).
3. Save to a local directory in safetensors format with the same file conventions CommitLLM expects.
4. Compute and print the canonical hash (per [RFC-0004](./RFC-0004-verifier-key-generation.md) §"Compute checkpoint hash").

### Recipe artifact

`scripts/quantize/recipe.yaml` carries the exact compressor configuration. This file plus the FP16 source plus `quantize.py` is the reproducibility contract. A third party re-running the script with the same recipe and source on the same hardware class gets a byte-identical or near-identical output (compressor determinism caveats apply; we document them).

### Hosting

- Repository: `AbdelStark/Llama-3.2-1B-Instruct-quantized.w8a8`.
- License: matches Llama 3.2 license (we redistribute under the same terms). The `LICENSE` and `MODEL_CARD.md` files in the mirror repo declare the origin and the recipe.
- Files: `config.json`, `model-*.safetensors`, tokenizer files; matches CommitLLM's expectations.
- Pin: a commit SHA on the HF repo identifies the exact snapshot we publish.

### Canonical hash

Once the upload is final, the canonical checkpoint hash is computed via the procedure in [RFC-0004](./RFC-0004-verifier-key-generation.md) and pinned in:

- `crates/vi-keygen/src/models.rs` as a constant.
- `docs/spec/03-data-model.md` (or a referenced data file).
- The README ("Canonical checkpoint: `sha256:...`").

A change to the hash is a MAJOR-or-MINOR event depending on whether old receipts can still verify against new keys (they cannot; binding mismatch). Practically, a hash change is a coordinated re-release.

### Switch-over plan

If a maintained W8A8 checkpoint becomes available (e.g. published by `neuralmagic/...`):

1. Compare bytes. If the new checkpoint matches our mirror bit-for-bit (identical quantization), switch the canonical pointer to it; document the move; keep our mirror as a backup.
2. If the new checkpoint differs, evaluate the trade-off: a maintained upstream is preferable to a manual mirror, but switching invalidates existing keys. The decision is a release-issue conversation; default is to stay on our mirror through v1.x and reevaluate at v2.

### Verification

`vi keygen --checkpoint <path>` re-computes the canonical hash. CI runs this against a small known checkpoint and asserts the hash matches a stored expected value.

`vi keygen` against the mirror downloads, hashes, and confirms before generating the key. A mismatch (HF having silently changed bytes) fails closed with `hash_mismatch`.

## Alternatives Considered

**Quantize at provider boot time.** Rejected: adds GPU cycles and time to cold start; mismatched provider runs would produce silently different bytes; reproducibility is harder.

**Re-quantize on user's machine in `vi keygen`.** Rejected: forces every user to have a GPU; defeats the project's UX premise.

**Use FP16 directly (no W8A8).** Rejected: CommitLLM's published corridor is on W8A8; the project must measure on W8A8 to compare; the provider image VRAM budget assumes W8A8.

**Trust a third-party W8A8 checkpoint without verification.** Rejected: a checkpoint at an unverified hash is a supply-chain risk for verification material. We hash at keygen, always.

## Drawbacks

- We take on ongoing operational responsibility for a HF model repo. Acceptable: maintenance is minimal; a repo with a stable artifact and a stable hash needs almost no care.
- Re-quantization for a new base-model version is non-zero work. Mitigation: this is a v2 concern; v1 ships exactly one model.

## Migration / Rollout

- The mirror is created and uploaded during the build-phase spike (OQ-2 resolution).
- The canonical hash is computed and committed.
- The provider image build downloads from the mirror with hash-verification (a `wget --checksum` pattern via `huggingface_hub`).

## Testing Strategy

- A CI test (when network is available in CI; otherwise on demand) downloads a small file from the mirror and verifies its hash.
- The provider Dockerfile fails the build if the downloaded weights hash does not match the pinned canonical hash.
- A "quantization smoke test" is a separate on-demand test that runs `quantize.py` on a tiny stand-in model and asserts output format.

## Open Questions

- **OQ-2 carry-over**: if upstream W8A8 becomes available during build phase, do we switch? Default: no, stay on our mirror through v1.x; reevaluate at v2.

## References

- [PRD §7 FR-4, §11 OQ-2](../../PRD.md)
- [RFC-0004](./RFC-0004-verifier-key-generation.md)
- [llm-compressor](https://github.com/vllm-project/llm-compressor) upstream.
- CommitLLM 8B W8A8 recipe (in the upstream repo's `paper/` or `scripts/` directory).
