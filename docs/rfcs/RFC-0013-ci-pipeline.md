# RFC-0013: CI pipeline and GPU-on-demand workflow

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

CI is GitHub Actions. Per-PR runs cover the full Rust workspace build, the test pyramid up through the tamper fuzz at N=100, the fixture-based end-to-end verify, schema validation, license/advisory checks, and image-build smoke. Latency benchmarks and corridor measurements run on-demand because they require dedicated hardware. The pipeline is the gating mechanism for PR merge and for release.

## Motivation

The project lives or dies on the soundness of its small contract surface. CI is the place that contract is enforced before code reaches main. Without a disciplined per-PR pipeline, the binding-header CRC check, the tamper detection rate, the size budgets, and the schema invariants all rot in months.

## Goals

- Every claim in the spec that can be mechanically verified is mechanically verified on every PR.
- Cost-bounded: per-PR runs fit in standard GitHub Actions minutes.
- GPU work is separate, manually-triggered, and well-scoped.
- A clear release-gate workflow that runs all release-relevant tests on a release candidate.

## Non-Goals

- No self-hosted CI infrastructure in v1. GitHub Actions only.
- No multi-arch test matrix beyond Linux x86_64 and macOS arm64 (the published binary targets). Windows is best-effort: built but not gated.
- No load testing or chaos testing.

## Proposed Design

### Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | PR, push to main | Full per-PR pipeline (see jobs below) |
| `release.yml` | Tag push `v*` | Build release binaries, publish crates, push image, create GitHub Release |
| `nightly.yml` | Cron, daily | Tamper fuzz at N=1000; perf benchmarks; weekly `--no-cache` provider build |
| `corridor.yml` | `workflow_dispatch` only | Run corridor measurement on a GPU runner |
| `deploy-hf.yml` | `workflow_dispatch` only | Run HF deploy recipe against a configured endpoint |

### `ci.yml` jobs

1. **Lint.** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
2. **Build.** `cargo build --workspace --all-targets` on both stable and MSRV.
3. **Unit + integration tests.** `cargo test --workspace`.
4. **Tamper fuzz.** `cargo test --test tamper-fuzz` with N=100.
5. **Schema validation.** A small Rust runner that produces fixture outputs and validates them against `schemas/*.schema.json`.
6. **License/advisory.** `cargo deny check`.
7. **Forbidden edges.** A `cargo deny` rule or a dedicated script that asserts the dependency graph respects [RFC-0001](./RFC-0001-workspace-and-crate-layout.md) §"Crate responsibilities".
8. **`--help` snapshot.** `cargo test help_snapshots`.
9. **Size budgets.** Compare release binary sizes and `key.bin` fixture size against budgets in [08-performance-budget.md](../spec/08-performance-budget.md). Regression beyond margin fails.
10. **Image smoke.** If `provider/**` changed, `docker build` the image and run a CPU stub of `/healthz` (no GPU). Image size budget check.
11. **CHANGELOG-pin lint.** If `commitllm.lock` changed, the same PR must add a `### Pin` entry under `[Unreleased]` in `CHANGELOG.md`.
12. **Secret scan.** `gitleaks` (or equivalent) on the diff.

Jobs run in parallel where they don't depend on each other. The build-and-test jobs are the longest pole; parallelizing the others keeps overall PR feedback under ~10 minutes.

### `nightly.yml` jobs

1. **Tamper fuzz at N=1000.** A failure opens an issue with the seed.
2. **Bench.** `cargo bench --workspace`. Results uploaded to `reports/perf/`; regressions opened as issues.
3. **`--no-cache` provider build.** Detects base-image drift.

### `corridor.yml`

Manually triggered by a maintainer. Inputs:

- `workload`: which workload to run (or `all`).
- `gpu`: which GPU runner to use.
- `commitllm_pin`: optional override.

Output: a corridor JSON committed to `reports/corridor/` via a PR opened by the workflow. Numbers in the README update only after the PR is reviewed and merged.

The corridor workflow uses a self-hosted GPU runner (documented in `docs/ci/gpu-runners.md`, tracked as a docs issue). If no GPU runner is configured, the workflow exits with a clear setup message rather than failing silently.

### `release.yml`

On tag `v<MAJOR>.<MINOR>.<PATCH>`:

1. Build release binaries for Linux x86_64 and macOS arm64 (and Windows best-effort).
2. Strip and sign (where applicable).
3. Compute SHA-256 checksums.
4. `cargo publish` the umbrella crate.
5. Build and push the provider Docker image to the canonical registry.
6. Create the GitHub Release with binaries, checksums, and the CHANGELOG section for the tag.

### Caching

- `cargo` cache via `Swatinem/rust-cache` (or maintained successor).
- Docker layer cache via GHCR or BuildKit's inline cache.

### Secrets

- `HF_TOKEN`, `HF_REGISTRY_TOKEN`, `CRATES_IO_TOKEN`, registry credentials live in encrypted GitHub secrets.
- No secret is required for `ci.yml`. The fresh-environment integration test runs against a CI-only provider container (a service container started by the job), not a live HF endpoint.

### Reporting

- Each workflow uploads structured artifacts (junit-xml or json).
- A maintained `docs/ci/README.md` (tracked as a docs issue) lists every workflow, its trigger, its expected runtime, and where the artifacts go.

## Alternatives Considered

**CircleCI / Buildkite.** Rejected: GitHub Actions integrates natively with the repo, costs nothing for public OSS, and is enough for v1.

**Self-hosted runners for everything.** Rejected: GA's hosted runners are cheap and present. Self-hosted is reserved for GPU work.

**Mandate corridor in every PR.** Rejected per [RFC-0010](./RFC-0010-corridor-measurement.md). GPU budget.

**Skip license / advisory checks.** Rejected: license discipline is part of "world-class OSS practices".

## Drawbacks

- GitHub Actions outages stall the project. Acceptable: a one-day outage is rare; the spec is not delayed by CI.
- `cargo deny`'s advisory feed has occasional false positives. We document the override mechanism (`[advisories.ignore]`) in `deny.toml` with comments per override.

## Migration / Rollout

- The bootstrap PR for the workspace skeleton lands `ci.yml` at the same time.
- `nightly.yml` lands after the tamper fuzz harness ([RFC-0009](./RFC-0009-tamper-fuzz-harness.md)).
- `corridor.yml` lands when the corridor script is implementable (after provider image builds against a real GPU).
- `release.yml` lands one minor before v1.0 so the cut is well-rehearsed.

## Testing Strategy

The CI pipeline is, itself, the testing strategy at the integration layer. Meta-tests:

- A deliberately broken assertion is caught (validated once during initial implementation).
- Workflow YAML is linted with `actionlint`.
- A documented "what to do when CI is red" runbook lives in `docs/ci/red-build.md` (tracked as a docs issue).

## Open Questions

None at this layer.

## References

- [07-testing-strategy.md](../spec/07-testing-strategy.md)
- [08-performance-budget.md](../spec/08-performance-budget.md)
- [RFC-0009](./RFC-0009-tamper-fuzz-harness.md)
- [RFC-0010](./RFC-0010-corridor-measurement.md)
