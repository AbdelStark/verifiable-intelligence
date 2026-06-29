# RFC-0005: Provider Docker image architecture

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

The provider is a single Docker image built from one Dockerfile. It contains vLLM with the CommitLLM patches applied at the pinned commit, Llama 3.2 1B Instruct W8A8 weights pre-baked into the image (small enough at ~1.5 GB to fit), and a thin entrypoint that boots vLLM, exposes the OpenAI-compatible chat endpoint, the audit endpoint, and `/healthz`. The image targets a single GPU with at least 16 GB VRAM. No Modal-specific code is in the critical path; the image is portable across HF Inference Endpoints, self-hosted `docker compose`, and (best-effort) other targets.

**Pivot note, 2026-06-29:** after [RFC-0016](./RFC-0016-marketplace-demo-pivot.md), the provider-image pattern remains valid, but the baked model should follow the selected CommitLLM-supported marketplace reference model.

## Motivation

Two friction points the project must remove for the integrating developer and for the demo presenter:

1. **A deployment that requires running a Python prover separately from a model server is too many moving parts.** vLLM-plus-CommitLLM upstream already integrates the prover into the serving path. Our image should reflect that.
2. **A multi-step "download weights at boot" sequence costs cold-start time on HF Endpoints and adds a network dependency at runtime.** Pre-baking the W8A8 1B weights into the image avoids both. The image is bounded under 8 GB per [PRD C8](../../PRD.md).

## Goals

- One Dockerfile, multi-stage, reproducible build.
- Final image under 8 GB.
- Boots to `/healthz` green in under 90 s ([08-performance-budget.md](../spec/08-performance-budget.md)).
- Runs unchanged on:
  - HF Inference Endpoints (custom container).
  - Self-hosted single-GPU host via `docker compose`.
  - Locally on an A10G / L4 / A100 dev box.
- Carries the CommitLLM pin in `/healthz` output.

## Non-Goals

- No multi-GPU serving in v1. CommitLLM's prover instrumentation on multi-GPU is an upstream concern; we stick to single-GPU.
- No autoscaling, no HPA, no Kubernetes manifests. HF Endpoints does its own autoscaling; self-hosted is one container.
- No GPU type detection logic in the image. Operators choose hardware.
- No metrics exporter beyond what vLLM already provides.

## Proposed Design

### Dockerfile structure

```dockerfile
# Stage 1: builder
FROM nvcr.io/nvidia/cuda:12.4.1-devel-ubuntu22.04 AS builder
# - install python, build tools
# - clone CommitLLM at pinned commit
# - apply patches / build extension wheels
# - clone vLLM at the version CommitLLM is built against
# - install Python deps to a wheelhouse

# Stage 2: weights
FROM busybox AS weights
# - copy in pre-downloaded W8A8 safetensors (downloaded at build time
#   via a build arg with the canonical checkpoint hash; build fails if
#   hash mismatches)

# Stage 3: runtime
FROM nvcr.io/nvidia/cuda:12.4.1-runtime-ubuntu22.04
COPY --from=builder /opt/wheelhouse /opt/wheelhouse
COPY --from=builder /opt/python /opt/python
COPY --from=weights /weights /weights
COPY provider/entrypoint.sh /entrypoint.sh
COPY provider/healthz.py /healthz.py
ENV MODEL_DIR=/weights MODEL_ID=llama-3.2-1b-w8a8
EXPOSE 8000
ENTRYPOINT ["/entrypoint.sh"]
```

Specific versions, hashes, and base image tags are pinned in the Dockerfile itself; the spec governs the layout, not the exact tags (those evolve).

### Entrypoint

`entrypoint.sh` does:

1. Print `provider.boot` log line with `commitllm_pin`, `model_id`, `checkpoint_hash` (computed at build time, embedded as an env var).
2. Launch vLLM with serving config: `--model /weights`, `--max-num-seqs <N>`, `--port 8000`, and the CommitLLM-required flags.
3. Background a small HTTP server on `/healthz` that returns the documented JSON ([02-public-api.md §2.3](../spec/02-public-api.md)).
4. On readiness, print `provider.ready`.

Signal handling: SIGTERM triggers a graceful shutdown via vLLM's standard mechanism.

### Build determinism

- Base image pinned by digest, not tag.
- Python deps locked via `requirements.lock`.
- CommitLLM cloned by SHA, not branch.
- Weight download happens once at build time with a known checkpoint hash; build fails on hash mismatch.
- `docker buildx` with `--platform linux/amd64`; ARM is out of scope for the provider.

CI builds the image periodically (weekly nightly) with `--no-cache` to catch silent base-image drift.

### Configuration surface

| Env var | Default | Effect |
|---------|---------|--------|
| `VI_BIND_ADDR` | `0.0.0.0:8000` | Listen address |
| `VI_MAX_NUM_SEQS` | `8` | vLLM concurrency |
| `VI_RATE_LIMIT_RPM` | `12` | Per-IP rate-limit (60 req / 5 min) |
| `VI_AUDIT_RATE_LIMIT_RPM` | `120` | Audit endpoint rate-limit |
| `VI_MAX_TOKENS` | `1024` | Generation cap |
| `VI_LOG_LEVEL` | `info` | Provider log level |

Anything missing falls back to documented defaults. Defaults match the abuse controls in [06-security.md](../spec/06-security.md).

### CommitLLM patches

CommitLLM upstream maintains a vLLM patch set. We apply it during the builder stage. If upstream's patch set drifts (e.g. due to vLLM version bumps), we open a tracking issue, NOT a workaround in our image. The build either succeeds against the pinned commit or fails the build; we do not paper over upstream changes.

### Health check

`/healthz` returns:

```json
{
  "status": "ok",
  "model_id": "llama-3.2-1b-w8a8",
  "checkpoint_hash": "sha256:...",
  "commitllm_pin": "<short-sha>",
  "uptime_s": 412
}
```

- Used by HF Endpoints readiness probe.
- Used by `docker compose` healthcheck.
- Used by `vi`'s optional preflight to warn on pin mismatch.

## Alternatives Considered

**Pull weights at boot instead of baking them in.** Rejected: HF Endpoints custom container has a cold-start budget; downloading 1.5 GB at boot pushes us against it; also introduces a runtime dependency on HF that we can avoid trivially because the model is small.

**Use a non-CUDA base (rocm, cpu-only, openvino).** Rejected: CommitLLM upstream is validated on CUDA; using a different runtime adds an unsupported configuration. Possible v1.x.

**Ship multiple images (one per GPU class).** Rejected: vLLM autodetects; one image works.

**Bake the prover Python sidecar separately from vLLM.** Rejected: CommitLLM upstream integrates it into vLLM's serving loop; we follow upstream.

## Drawbacks

- A weight update means an image rebuild + retag + redeploy. Acceptable: weights change rarely (per upstream tokenizer or config bumps).
- Pre-baked weights mean the image is ~3 GB instead of ~1.5 GB, but it is still well inside the 8 GB envelope.

## Migration / Rollout

- First-pass image lands behind the `provider:image` tracking issue.
- HF Endpoint dry-run gates the v0.1 milestone close.
- Image tagging discipline is set on day one: `:v0.1.0`, `:v0.1.1`, ..., `:latest` advances on stable releases only.

## Testing Strategy

- Container smoke test in CI on every PR that touches `provider/`: `docker build` succeeds, `docker run` boots in CPU-degraded mode (no real model, just verify entrypoint) and `/healthz` returns the correct envelope shape.
- GPU integration test on-demand (not per PR; budget): `docker run --gpus all`, send a known prompt, verify the receipt round-trips through `vi verify`.
- Image-size budget gate ([08-performance-budget.md](../spec/08-performance-budget.md)): regression fails CI.
- HF Endpoint dry-run: documented manual test before v0.1 close.

## Open Questions

- OQ-5 (HF Endpoint constraints) is resolved during build phase. Mitigation: self-hosted is the always-available fallback; HF is the preferred path but not a load-bearing dependency.

## References

- [01-architecture.md](../spec/01-architecture.md)
- [RFC-0007](./RFC-0007-hf-deployment-recipe.md)
- [PRD §7 FR-2, FR-3, FR-4](../../PRD.md)
