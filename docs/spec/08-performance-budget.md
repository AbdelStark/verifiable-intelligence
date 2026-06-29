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
