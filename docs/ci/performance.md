# Performance CI

The marketplace-demo pivot measures the browser and proof-bundle surfaces first.
The old CLI binary/key-size gate is no longer the v1 spine.

## Per-PR Size Gate

`ci.yml` runs `npm run test:size-budget` on every pull request and push to
`main`. The job records artifact sizes in the GitHub Step Summary and uploads
`reports/perf/size-budgets.json`.

Current hard gates:

- `demo/index.html` stays under the static-demo budget in
  `docs/spec/08-performance-budget.md`.
- Every routine `fixtures/viex/*.json` proof bundle stays under the routine
  `VIEX` budget.
- The browser verifier WASM stays under the preferred v1 WASM budget.

## Nightly Browser Benchmark

`nightly.yml` runs `npm run bench:browser` daily and on manual dispatch. The
benchmark loads the browser verifier harness in Chromium, runs the happy-path
and tampered-receipt fixture checks, and uploads `reports/perf/browser-verifier.json`.

The benchmark is measurement-only. If a number misses the published target, the
project should update the claim or add a new issue with the measured regression
instead of hiding the result.
