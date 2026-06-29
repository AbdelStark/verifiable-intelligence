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
- `verifier/wasm/fixtures/v4_key_fullbridge.bin` stays under the CommitLLM
  verifier-key budget. CI reports a 10 MiB target and fails above the 11 MiB
  hard limit.

The current checked-in CommitLLM full-bridge verifier-key fixture is 2,615 bytes
as measured on 2026-06-29. The full `llama-3.2-1b-w8a8` key remains a measured
artifact once a canonical fixture exists; the CI gate is deliberately wired to
the reproducible fixture that exists in this repository today.

The `provider-image` job runs when `provider/` or its CI workflow changes. It
records `reports/ci/provider-image-size.log` with the final Docker image size,
the 8 GiB hard limit, the local image ID, and repo digests when Docker has them.
The job fails if the final image exceeds `8589934592` bytes.

## CLI-Only Build Budget

The umbrella crate exposes a `tui` feature that defaults on. Disabling default
features keeps the binary on the CLI utility path and excludes the optional
`vi-tui` dependency:

```bash
cargo build -p verifiable-intelligence --no-default-features --release --locked
```

Current documented budget:

| Artifact | Target | Current measurement |
|----------|--------|---------------------|
| `target/release/vi` built with `--no-default-features` | < 10 MB | 4,657,728 bytes on 2026-06-29 |

The CI `stable-test` job runs a debug no-default build as a smoke. Release-size
measurement is documented here rather than used as a v1 merge gate because the
browser proof-market demo is the primary release surface.

## Nightly Browser Benchmark

`nightly.yml` runs `npm run bench:browser` daily and on manual dispatch. The
benchmark loads the browser verifier harness in Chromium, runs the happy-path
and tampered-receipt fixture checks, and uploads `reports/perf/browser-verifier.json`.

The benchmark is measurement-only. If a number misses the published target, the
project should update the claim or add a new issue with the measured regression
instead of hiding the result.

## Weekly Provider Image Drift Check

`nightly.yml` also builds the provider image with `--no-cache` on Sundays and on
manual dispatch. It runs the CPU-stub smoke, records the image size and image ID
under `reports/perf/provider-image-no-cache-summary.md`, and fails if the image
exceeds the same 8 GiB limit used by per-PR provider-image CI.

This job catches base-image drift. It is not evidence that the live GPU path or
canonical W8A8 weights are ready.
