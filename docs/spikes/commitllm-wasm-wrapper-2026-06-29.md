# CommitLLM browser-WASM wrapper

- Date: 2026-06-29
- Related issues: #119, #121, #131
- Upstream pin: `lambdaclass/CommitLLM@25541e83347655e44ad6e84eb901e1e7ae392a66`
- Result: browser-WASM verifier wrapper works on the pinned upstream full-bridge fixture.

## What changed

The wrapper lives under `verifier/wasm/` and exports:

- `commitllm_pin()`
- `verify_v4_audit(key_bytes, audit_bytes)`
- `verify_viex_bundle(bundle_json, key_bytes)`

`verify_viex_bundle` checks the VIEX receipt hash, decodes the embedded CommitLLM `VV4A` receipt bytes, and delegates to the pinned CommitLLM canonical verifier in WASM.

## Upstream patches

The wrapper keeps `verilm-core` pinned as a git dependency. `verilm-verify` is vendored locally because one browser-runtime patch is required:

- native targets keep `std::time::Instant`,
- `wasm32` uses a zero-duration timer shim for report duration.

This does not change verifier decisions. It only avoids the browser trap from `Instant::now()` on `wasm32-unknown-unknown`.

The wrapper also patches the `zstd` crate name to a small compatibility crate backed by pure-Rust `ruzstd`. This preserves the canonical `VV4A + zstd-compressed bincode` receipt format; the browser path does not use the raw-bincode bypass from the earlier spike.

## Browser validation

Command:

```bash
npm run build:wasm
npm run test:wasm
```

Result:

- Chromium desktop: pass.
- Chromium mobile profile: pass.
- Happy fixture: `overall = pass`, `checks_run = 37`, `checks_passed = 37`, `coverage = full`.
- Tampered fixture: `overall = fail` after receipt bytes are mutated and the VIEX receipt hash is recomputed, so the failure reaches the CommitLLM verifier path.

## Measurements

Measured with the route-backed Playwright harness in `verifier/wasm/harness.html`.

| Item | Value |
|------|-------|
| WASM artifact | `verifier/wasm/pkg/vi_commitllm_verifier_bg.wasm` |
| WASM size | 458,996 bytes |
| JS loader size | 8,180 bytes |
| VIEX fixture size | 4,712 bytes |
| Key fixture size | 2,615 bytes |
| Audit fixture size | 1,501 bytes |
| Happy verification p50 / p95 | 0.10 ms / 0.30 ms over 20 browser runs |
| Tampered verification p50 / p95 | 0.10 ms / 0.10 ms over 20 browser runs |
| Browser memory | Chromium `performance.memory` reported 10,000,000 bytes before and after; delta 0 bytes |

## Boundaries

This is a real browser-WASM verifier for the pinned upstream full-bridge fixture. It is still not a live `llama-3.1-8b-w8a8` provider receipt. The static marketplace demo remains fixture-simulated until a live provider emits compatible CommitLLM receipts and verifier keys.
