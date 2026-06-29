# 08 - Performance Budget

The pivot moves the user-facing budget from CLI/TUI timing to browser demo and proof-bundle verification timing.

## Browser demo budgets

| Surface | Target | Notes |
|---------|--------|-------|
| Static HTML size | < 500 KB | Excluding Google Fonts |
| First render | < 2 s | Laptop browser, warm network |
| Simulated verification | < 3 s | Includes hash computation and UI render |
| Mobile interaction latency | < 100 ms | Provider/mode selection and panel updates |

## Proof bundle budgets

| Artifact | Target | Notes |
|----------|--------|-------|
| Routine `VIEX` bundle | < 250 KB | Excludes deep audit openings |
| Embedded receipt | measured | Publish actual CommitLLM size |
| Deep audit bundle | measured | May exceed routine target |
| Browser verifier WASM | < 10 MB preferred | Spike may revise |

## Live provider budgets

| Operation | Target | Notes |
|-----------|--------|-------|
| Quote creation | < 200 ms | Broker/local provider metadata only |
| Chat response | model-dependent | Report p50/p95, do not gate v1 on model speed |
| Routine verification | < 10 s in browser | Warm cache target |
| Server-side fallback verification | < 2 s | Prototype fallback only |

## Measurement rules

- Publish hardware, browser, CommitLLM pin, model ID, and fixture size.
- Report p50 and p95 over at least 20 runs for browser verifier measurements.
- If the target is missed, keep the measured number and narrow the claim. Do not hide it behind a spinner.

## Current browser-WASM fixture measurement

Measured on 2026-06-29 with Chromium headless through `tests/commitllm-wasm.spec.js`:

| Item | Measurement |
|------|-------------|
| CommitLLM pin | `25541e83347655e44ad6e84eb901e1e7ae392a66` |
| Fixture | upstream full-bridge toy fixture wrapped in `verifier/wasm/fixtures/commitllm-fullbridge.viex.json` |
| WASM size | 458,996 bytes |
| JS loader size | 8,180 bytes |
| Key fixture size | 2,615 bytes |
| Audit fixture size | 1,501 bytes |
| Happy-path browser verification | p50 0.10 ms, p95 0.30 ms over 20 runs |
| Tampered-receipt browser verification | p50 0.10 ms, p95 0.10 ms over 20 runs |
| JS heap delta | 0 bytes reported by Chromium `performance.memory`; this is a coarse browser metric |

These measurements are for the upstream toy/full-bridge fixture only. They are not evidence that a full `llama-3.1-8b-w8a8` marketplace receipt has the same size or latency.
