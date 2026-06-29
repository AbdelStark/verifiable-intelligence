# Red Build Runbook

Use this runbook when a GitHub Actions check is failing on a pull request or on
`main`.

## First Pass

1. Open the failing check and identify the workflow file, job, and failing step.
2. Download artifacts before rerunning, especially `reports/ci/`, `reports/perf/`,
   `test-results/`, and `playwright-report/`.
3. Classify the failure as one of:
   - deterministic repo regression,
   - missing or changed CI dependency,
   - external GitHub Actions or network failure,
   - expected failure from a deliberately changed contract.
4. Reproduce locally with the matching command from the table below.
5. Patch the branch, rerun the focused local command, then rerun the broader
   command that owns the contract.

Rerun a hosted job only after reading the log. One rerun is reasonable for clear
infrastructure failures; repeated reruns without a code or environment change are
not a fix.

## Local Reproduction Map

| Hosted job | First local command |
|------------|---------------------|
| `lint` | `scripts/ci/lints.sh` |
| `workflow-lint` | `npm run test:workflows` |
| `stable-test` | `cargo test --workspace --all-features --locked`, `cargo build -p verifiable-intelligence --no-default-features --locked`, and `npm run test:error-envelopes` |
| `msrv-build` | `cargo +1.82.0 build --workspace --all-targets --locked` |
| `proof-bundle` | `npm run test:bundle` |
| `browser-demo` | `npm run test:demo` |
| `size-budget` | `npm run test:size-budget` |
| `provider-image` | Docker build/smoke from `.github/workflows/ci.yml` when Docker is available; otherwise run `npm run test:provider` and leave a note that Docker was unavailable |
| `supply-chain` | `scripts/ci/deny.sh` |
| `secret-scan` | `npm run test:secret-scan` |
| `Keygen Determinism / determinism` | `cargo test -p vi-keygen tests::keygen_same_inputs_are_byte_identical_and_seed_changes_bytes -- --exact` |
| `Nightly / browser-verifier-bench` | `npm run bench:browser` |
| `Proof Bundle / proof-bundle` | `npm run test:bundle` |
| `Static Demo Pages` | inspect `demo/index.html` and rerun `npm run test:demo`; Pages deployment problems may require GitHub environment inspection |
| `Watch CommitLLM Rename / watch` | `npm run test:commitllm-rename-watch` |

## Common Failures

### Proof Bundle

- Run `npm run test:schema` first if the failure mentions JSON Schema.
- Run `npm run test:tamper` when the failure mentions red-path fixtures.
- Keep proof-bundle changes paired with schema and docs changes.

### Browser Demo

- Install Chromium dependencies with `npx playwright install --with-deps chromium`
  if the browser is missing locally.
- Check desktop and mobile failures; layout regressions often fail only one
  viewport.
- Use the Playwright trace or screenshot artifact before changing selectors.

### Secret Scan

- Do not paste matched values into issues or PR comments.
- If the finding is a false positive, add the fingerprint to
  `secret-scan.allowlist.json` with a clear reason and keep the allowlist review
  window fresh.

### Provider Image

- This job runs only when `provider/` changes.
- Local Docker daemon failures are environment blockers, not code fixes.
- If Docker is unavailable locally, run `npm run test:provider`, document the
  Docker limitation, and rely on hosted CI for the image smoke.

### Workflow Lint

- Run `npm run test:workflows`.
- Keep YAML changes small; actionlint errors usually point to the exact line.

## Main Branch Is Red

1. Find the first failing run on `main`.
2. Identify the merge commit or direct push that introduced the failure.
3. Prefer a forward fix over reverting. Revert only when the break blocks all
   work and the fix is not clear.
4. If a release or hosted demo is affected, update the relevant release issue or
   deployment note with the user-visible impact.

Useful commands:

```bash
gh run list --branch main --limit 10
gh run view <run-id> --log-failed
gh run download <run-id>
gh pr checks <pr-number>
```
