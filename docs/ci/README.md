# CI Workflows

This page lists the workflows that currently exist under `.github/workflows/`.
Keep the workflow table exact: if a workflow file is added, renamed, or removed,
update this page in the same pull request.

## Current Workflows

| Workflow file | Display name | Trigger | Expected runtime | Artifacts and reports |
|---------------|--------------|---------|------------------|-----------------------|
| `.github/workflows/ci.yml` | CI | Pull requests and pushes to `main` | 5-10 minutes | Failure artifacts under `reports/ci/`; size-budget artifacts under `reports/perf/`; Playwright failure output under `test-results/` and `playwright-report/` |
| `.github/workflows/keygen-determinism.yml` | Keygen Determinism | Pull requests and pushes to `main` that touch keygen or workspace dependency paths | 2-5 minutes | Job logs only |
| `.github/workflows/nightly.yml` | Nightly | Daily cron at 03:17 UTC and manual dispatch | 2-5 minutes for the current browser verifier benchmark | `reports/perf/` uploaded as `nightly-browser-verifier-${{ github.run_id }}` |
| `.github/workflows/proof-bundle.yml` | Proof Bundle | Pull requests and pushes to `main` that touch proof-bundle, broker, docs, schema, script, or package paths | 1-3 minutes | Job logs only |
| `.github/workflows/static-demo-pages.yml` | Static Demo Pages | Tag pushes matching `demo-v*` and manual dispatch | 1-3 minutes | GitHub Pages artifact containing `demo/index.html`, `.nojekyll`, and `release.json` |
| `.github/workflows/watch-commitllm-rename.yml` | Watch CommitLLM Rename | Daily cron at 07:37 UTC and manual dispatch | Less than 1 minute unless GitHub API is slow | Job logs; may open a tracking issue when upstream rename evidence changes |

## Per-PR Gate

`ci.yml` is the broad merge gate. It currently runs:

- `lint`: Rust formatting and clippy through `scripts/ci/lints.sh`.
- `workflow-lint`: `npm run test:workflows` and actionlint.
- `stable-test`: `cargo test --workspace --all-features --locked` plus
  `cargo build -p verifiable-intelligence --no-default-features --locked` and
  `npm run test:error-envelopes`.
- `msrv-build`: workspace build on Rust `1.82.0`.
- `proof-bundle`: `npm run test:bundle`.
- `browser-demo`: Playwright Chromium tests through `npm run test:demo`.
- `size-budget`: `npm run test:size-budget` with `reports/perf/` uploaded.
- `provider-image`: Docker provider image build, CPU-stub smoke, size gate, and
  image ID/digest summary when `provider/` or the provider-image workflow
  changes.
- `supply-chain`: `scripts/ci/deny.sh`.
- `secret-scan`: `npm run test:secret-scan`.

The separate `proof-bundle.yml` workflow intentionally overlaps with the
`ci.yml` proof-bundle job for path-focused proof contract feedback.

## Planned But Not Installed

RFC-0013 describes future workflows named `release.yml`, `corridor.yml`, and
`deploy-hf.yml`. They are not present today, so they are not listed in the exact
workflow table above.

Until those workflows exist:

- release cuts are manual or tracked by release issues,
- corridor measurement remains a documented template, not a CI path,
- Hugging Face deployment is documented as an operator guide, not a GitHub
  Actions deploy workflow.

## Artifact Lookup

For a failed run:

```bash
gh run view <run-id> --log-failed
gh run download <run-id>
```

Use `docs/ci/red-build.md` for triage steps and `docs/ci/gpu-runners.md` before
adding GPU-backed workflow work.
