# CommitLLM WASM verifier spike

- Date: 2026-06-29
- Related issues: #119, #121, #131
- Upstream tested: `lambdaclass/CommitLLM@25541e83347655e44ad6e84eb901e1e7ae392a66`
- Decision: no-go for unmodified browser verification in v1. Go only after #131 adds a browser-WASM wrapper and wasm-safe canonical audit decoding.

## Objective

Issue #121 asked whether the CommitLLM verifier path needed for routine `VIEX` proof bundles can run locally in a consumer browser. The v1 demo target is still browser verification, because the buyer should not have to trust the broker. This spike checks whether the current upstream verifier can be built, packaged, and measured as a browser artifact.

## Environment

Test host:

- macOS 26.5.1, Darwin 25.5.0, arm64
- `rustc 1.94.0 (4a4ef493e 2026-03-02)`
- `cargo 1.94.0 (85eff7c80 2026-01-15)`
- `wasm32-unknown-unknown` Rust target installed
- Node `v25.2.0`, npm `11.6.2`

Upstream checkout:

```bash
git clone --depth 1 https://github.com/lambdaclass/CommitLLM.git /tmp/commitllm-wasm-spike
git -C /tmp/commitllm-wasm-spike rev-parse HEAD
# 25541e83347655e44ad6e84eb901e1e7ae392a66
```

Relevant upstream crates at that pin:

- `crates/verilm-core`
- `crates/verilm-verify`
- `crates/verilm-prover`
- `crates/verilm-keygen`
- `crates/verilm-test-vectors`
- `crates/verilm-py`

The routine verifier entry points live in `verilm-verify`, especially `canonical::verify_binary`, `canonical::verify_response`, `client::verify_challenged_binary`, and `client::verify_challenged_response`.

## Attempt 1: unmodified wasm build

Command:

```bash
/usr/bin/time -p cargo build -p verilm-verify --target wasm32-unknown-unknown --release
```

Result: failed before verifier code compiled.

Primary error:

```text
error: the wasm*-unknown-unknown targets are not supported by default, you may need to enable the "js" feature.
...
error[E0433]: failed to resolve: use of unresolved module or unlinked crate `imp`
```

Cause: `verilm-core` depends on `rand` and `rand_chacha`; those pull `getrandom 0.2.17` without the wasm JS backend. The dependency tree includes:

```text
verilm-verify
└── verilm-core
    ├── rand
    │   └── rand_core
    │       └── getrandom 0.2.17
    └── rand_chacha
```

## Attempt 2: temporary RNG patch

Temporary checkout-only patch:

```toml
getrandom = { version = "0.2", features = ["js"] }
```

This was added to `crates/verilm-core/Cargo.toml` only in `/tmp/commitllm-wasm-spike`.

Command:

```bash
/usr/bin/time -p cargo build -p verilm-verify --target wasm32-unknown-unknown --release
```

Result: progressed past `getrandom`, then failed in `zstd-sys`.

The local shell initially had a stale compiler override:

```text
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang
CFLAGS_wasm32_unknown_unknown=--target=wasm32-unknown-unknown
```

After retrying with the Xcode clang path, the failure was still a native C toolchain failure:

```text
error: unable to create target: 'No available targets are compatible with triple "wasm32-unknown-unknown"'
clang -cc1as: error: unknown target triple 'wasm32-unknown-unknown'
error occurred in cc-rs
```

Cause: upstream `verilm-core` uses `zstd` in the canonical V4 audit wire format:

- `serialize_v4_audit` writes `VV4A` plus zstd-compressed bincode.
- `deserialize_v4_audit` zstd-decompresses canonical audit bytes before bincode deserialization.
- `compress` and `decompress` are also zstd-backed transport helpers.

This is not test-only code. It is on the verifier path used by `canonical::verify_binary` and `client::verify_challenged_binary`.

## Attempt 3: temporary compression bypass

Temporary checkout-only patch:

- keep `getrandom` with the wasm JS feature,
- move `zstd` behind `cfg(not(target_arch = "wasm32"))`,
- make wasm `serialize_v4_audit`, `deserialize_v4_audit`, `compress`, and `decompress` use raw bytes.

This patch is intentionally not production-valid because it changes wasm handling of the canonical `VV4A` wire format. It was only used to see whether verifier code compiles once RNG and zstd are removed from the wasm build.

Command:

```bash
/usr/bin/time -p cargo build -p verilm-verify --target wasm32-unknown-unknown --release
```

Result:

```text
Finished `release` profile [optimized] target(s) in 6.57s
real 7.08
user 13.41
sys 0.70
```

This means the Rust verifier library path is close to wasm-compatible, but only after non-production changes to receipt/audit decoding.

## Artifacts and measurements

No browser-loadable WASM artifact is produced by the upstream crate.

```bash
find target/wasm32-unknown-unknown/release -name '*.wasm' -print | wc -l
# 0
```

The successful temporary build produced Rust library artifacts only:

```text
target/wasm32-unknown-unknown/release/libverilm_verify.rlib              1015310 bytes
target/wasm32-unknown-unknown/release/deps/libverilm_core-*.rlib         3619848 bytes
```

Fixture sizes in upstream `verilm-verify/tests/fixtures`:

```text
12 bytes     reject_truncated.bin
21 bytes     reject_corrupted_bincode.bin
974 bytes    reject_unknown_magic.bin
974 bytes    v4_audit_canonical.bin
1501 bytes   v4_audit_fullbridge.bin
2223 bytes   reject_cross_format.bin
2223 bytes   v4_key_canonical.bin
2615 bytes   v4_key_fullbridge.bin
```

Browser memory and browser verification time were not measured because there is no `.wasm` plus JS loader artifact to execute in Playwright. Measuring the temporary raw-bincode bypass would be misleading because it would not verify canonical CommitLLM audit bytes.

## Blockers

| Area | Status | Evidence |
| --- | --- | --- |
| Exact CommitLLM pin | resolved | Tested `25541e83347655e44ad6e84eb901e1e7ae392a66`. |
| Rust verifier logic | promising | Compiles to `wasm32-unknown-unknown` after temporary RNG and zstd bypasses. |
| RNG | blocked | `getrandom 0.2.17` needs wasm JS support or verifier-only dependency pruning. |
| Canonical audit decoding | blocked | `zstd-sys` compiles native C and blocks the browser target with the available toolchain. |
| Browser ABI | blocked | `verilm-verify` is a Rust library crate and emits no `.wasm` or JS loader. |
| Browser measurements | blocked | No browser-callable artifact exists. |
| File IO | not observed in library path | `verilm-verify/src` does not require file IO for `verify_binary` or `verify_response`; file fixtures are in tests. |
| Async/runtime assumptions | not observed | The verifier entry points are synchronous functions over bytes and deserialized structs. |
| Parallelism | watch item | `verilm-core` pulls `rayon`; the temporary build compiled it for wasm, but browser runtime performance/threading still needs measurement. |
| Payload size | unresolved | Upstream fixtures are small, but real routine audit payloads must be measured after the browser artifact exists. |

## Go/no-go

No-go for claiming consumer-side browser verification on the current unmodified upstream CommitLLM verifier.

Go for a clearly labeled server-side verifier fallback in the prototype while #131 builds the actual browser-WASM path.

The smallest unblocker is #131:

1. Add a browser-callable verifier wrapper for routine `VIEX` proof bundles.
2. Configure or remove `getrandom` from the verifier-only wasm target.
3. Preserve the canonical `VV4A` zstd-compressed audit format while making decoding browser-safe.
4. Produce a `.wasm` plus JS loader.
5. Measure size, memory, and verification time in Playwright on happy-path and tamper fixtures.

## Fallback for the demo

Until #131 lands, the v1 demo may use a server-side verifier API for live CommitLLM receipts if it is labeled as a fallback. The static browser demo can keep fixture-level simulated checks, but it must not present those checks as real CommitLLM WASM verification.

The buyer-facing language should distinguish:

- `browser-wasm`: consumer locally verifies the proof bundle,
- `server`: prototype server verifies and returns a report,
- `fixture`: static demo checks schema and red-path bindings only.

This keeps the research demo honest while preserving the final consumer-trust target.
