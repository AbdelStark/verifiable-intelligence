# 07 - Testing Strategy

Testing follows the pivot: proof bundle first, browser demo second, live provider third. The old raw receipt tests remain necessary but no longer sufficient.

## 1. Static demo tests

- Load `demo/index.html` in Chromium at desktop and mobile viewport sizes.
- Assert provider cards render.
- Run the honest path and assert overall `PASS`.
- Run `model swap`, `prompt edit`, `answer rewrite`, `receipt tamper`, and `expired quote`; assert they do not render as pass.
- Select the closed-weight provider and assert `UNSUPPORTED`.
- Assert no text overlaps or clipped primary controls in target viewports.

## 2. Proof bundle schema tests

- Validate happy-path `VIEX` fixture against JSON Schema.
- Validate each red-path fixture against JSON Schema.
- Reject missing `magic`, unknown `schema_version`, missing quote, missing key hash, and malformed receipt metadata.
- Ensure shared fixtures omit raw prompt and raw answer unless a test explicitly opts in.

## 3. Binding tests

Each mutation must fail:

- quote `model_id` changed after receipt,
- quote `checkpoint_hash` changed,
- verifier `key_hash` changed,
- request `prompt_hash` changed,
- response `answer_hash` changed,
- receipt bytes changed,
- CommitLLM pin changed,
- quote expired before verification.

## 4. CommitLLM verifier tests

Inherited from the old plan:

- receipt envelope parse tests,
- verifier-key identity mismatch tests,
- tamper fuzz over receipt bytes,
- audit payload binding tests,
- deterministic report fixture tests.

## 5. Browser verifier spike

The spike must produce:

- exact CommitLLM commit tested,
- build command,
- WASM binary size,
- browser memory measurement,
- verification time on fixture bundle,
- go/no-go decision,
- fallback path if blocked.

## 6. Provider integration tests

Once a live provider exists:

- `/healthz` advertises model ID, checkpoint hash, key hash, and CommitLLM pin.
- chat with `X-Verifiable-Receipt: 1` returns answer plus receipt.
- `POST /v1/audit` returns payload bound to the receipt and challenge.
- provider swap or wrong key fails in the browser/CLI verifier.

## 7. Comprehension gates

Before public v1:

- 5/5 README readers identify open-weight-only, no closed-weight frontier support, no unauthorized resale, and execution-integrity-only.
- 3/3 demo viewers trigger a red path and explain why it failed.
