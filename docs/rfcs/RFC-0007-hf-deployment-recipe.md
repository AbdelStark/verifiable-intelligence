# RFC-0007: HF Inference Endpoints deployment recipe

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

The HF Inference Endpoints recipe is a documented script (`scripts/deploy/hf.sh` plus prose in `docs/deployment/hf.md`) that builds the provider image, pushes it to a documented registry, creates an HF endpoint via the `hf` CLI as a custom container, waits for readiness, and prints the endpoint URL. If the `hf` CLI does not yet support a step we need, the recipe documents the exact UI clicks. Self-hosted `docker compose` is the always-available fallback; HF is the preferred but not load-bearing deployment.

## Motivation

[PRD G6 and FR-3](../../PRD.md) require a one-command HF deployment path under 30 minutes wall clock. The recipe must be reproducible enough that a contributor can deploy a fresh endpoint without DMing the maintainer. It must also gracefully degrade if HF features we depend on are missing.

## Goals

- One documented sequence from `docker build` to live URL.
- 30-minute wall-clock budget end-to-end ([FR-3](../../PRD.md)).
- `hf` CLI driven where possible; documented UI fallback where not.
- Registry-agnostic: works with GitHub Container Registry, Docker Hub, or any registry HF endpoints can pull from.
- A documented smoke test against the deployed endpoint: hit `/healthz`, run `vi chat` and `vi verify`.

## Non-Goals

- No Infrastructure-as-Code (Terraform, Pulumi). HF endpoints are configured via `hf` CLI / JSON; full IaC is over-engineering for a demonstration deployment.
- No managed-deployment pipeline (GitHub Actions cron deploys). v1 is a documented, manually-triggered recipe.
- No multi-region or HA deployment. One endpoint at a time.

## Proposed Design

### Recipe script (`scripts/deploy/hf.sh`)

Shell script driving:

1. `docker buildx build --platform linux/amd64 -t <registry>/<repo>:<tag> --push provider/`
2. Compute the image digest, pin it for the next step.
3. `hf endpoints create --name <name> --image <registry>/<repo>@<digest> --compute <gpu-class> --region <region>`
4. Poll `hf endpoints get` until status is `running`.
5. Hit `/healthz` and assert `commitllm_pin` matches the local pin.
6. Print the endpoint URL.

The script takes its inputs from environment variables and a `.env.example` template:

| Var | Description |
|-----|-------------|
| `HF_TOKEN` | HF API token with endpoint-create scope |
| `HF_REGISTRY` | Container registry path (e.g., `ghcr.io/abdelstark/vi-provider`) |
| `HF_REGISTRY_TOKEN` | Registry token for the build push |
| `HF_ENDPOINT_NAME` | Endpoint name (e.g., `vi-demo`) |
| `HF_COMPUTE` | GPU class (e.g., `nvidia-a10g-medium`) |
| `HF_REGION` | Region |

### Self-hosted fallback (`docker compose`)

```yaml
services:
  provider:
    image: ghcr.io/abdelstark/vi-provider:v0.1.0
    runtime: nvidia
    deploy:
      resources:
        reservations:
          devices:
            - capabilities: ["gpu"]
              count: 1
    ports: ["8000:8000"]
    healthcheck:
      test: ["CMD", "curl", "-fsS", "http://localhost:8000/healthz"]
      interval: 30s
      retries: 3
      start_period: 90s
```

Self-hosted is always supported. The repository's `compose.yaml` lives at `provider/compose.yaml`.

### Documentation

`docs/deployment/hf.md` (tracked under a docs issue):

- Prerequisites (HF account, GPU quota approval, registry credentials).
- Step-by-step with example output.
- Smoke test sequence: `curl /healthz`, `vi chat`, `vi verify`.
- Cost estimation table.
- Tearing down the endpoint (`hf endpoints delete`).
- Troubleshooting: image too large, cold-start exceeds HF budget, GPU unavailable in region.

### Failure paths

| Failure | Behavior |
|---------|----------|
| `hf` CLI missing or out-of-date | Recipe exits early with install instructions |
| Registry push fails | Recipe exits with `docker push` error inline |
| HF endpoint create fails (quota, region, image rejection) | Recipe captures the HF error and prints the troubleshooting link |
| Endpoint never becomes ready within 15 minutes | Recipe exits, prints log dump retrieval command (`hf endpoints logs`) |
| `/healthz` returns mismatched `commitllm_pin` | Recipe loudly warns: the image and the local repo are out of sync |

The recipe never silently succeeds when verification of its own work fails.

### `hf` CLI feature gap mitigation

If the `hf` CLI does not yet support a needed action (e.g., custom container scaling-down config), the recipe documents the manual UI sequence and tags the gap as an `OPEN QUESTION` here in this RFC, with a target to remove the manual step once `hf` supports it. At the time of this RFC, custom container endpoint creation is documented as supported by HF; we validate during the build-phase spike.

## Alternatives Considered

**Make HF the only deployment target.** Rejected: dependency on a single vendor's evolving CLI is too brittle for a project that wants to be reusable. Self-hosted is always present.

**Make self-hosted the only documented target.** Rejected: the integrating developer should be able to run against a public endpoint trivially; HF is the cheapest, most reproducible way to provide that.

**Ship a Helm chart.** Rejected: out of scope for v1; HF + compose covers both extremes (managed and bare-metal). Kubernetes lands when a user asks for it.

**Use Modal.** Rejected per [PRD G6](../../PRD.md): the recipe must be vendor-neutral, and "Modal-specific code in the critical path" is explicitly out.

## Drawbacks

- HF endpoint provisioning takes minutes; the 30-minute wall-clock budget is realistic but not slack.
- Registry choice is a parameter not a constraint; some users prefer Docker Hub, others GHCR; the recipe stays parameterized.

## Migration / Rollout

- Self-hosted recipe lands first (simpler, covers CI).
- HF recipe lands after a 1-week build-phase spike (OQ-5 in [SPEC.md](../../SPEC.md)).
- If the spike reveals an HF limitation that breaks the 30-minute budget, we update the recipe to "two-step: build + manual deploy" and note the gap in the README's quickstart.

## Testing Strategy

- CI builds the image on every PR that touches `provider/`. CI does not deploy to HF (cost, secrets).
- CI runs `docker compose up` smoke against a CPU stub of the image (no GPU available in CI) to validate entrypoint and `/healthz` shape.
- Pre-release: maintainer runs the HF recipe end-to-end and records the wall-clock time in the release issue.
- A "deploy dry-run" GitHub Actions workflow exists, gated on a manual trigger, with the HF secrets available.

## Open Questions

- **OQ-5 carry-over**: HF Endpoint custom-container limits (RAM, startup, registry restrictions) are validated during a build-phase spike. Resolution: a Yes/No on whether HF is the recommended path in the README. Default if unresolved: self-hosted is recommended; HF is documented as "may require tuning."
- **OQ-6 carry-over**: whether a public demo HF endpoint runs continuously. Owner: AbdelStark. Resolution trigger: 2 weeks before public release. Default: self-hosted-only at launch.

## References

- [RFC-0005](./RFC-0005-provider-image.md)
- [PRD §7 FR-3, §11 OQ-5, OQ-6](../../PRD.md)
- HF Inference Endpoints custom container docs (linked from PRD §14)
