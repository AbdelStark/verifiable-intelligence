# Upstream Source

This crate is a minimal local vendor copy of CommitLLM's `crates/verilm-verify` package.

- Upstream: `https://github.com/lambdaclass/CommitLLM`
- Commit: `25541e83347655e44ad6e84eb901e1e7ae392a66`
- License: MIT, copied in `LICENSE`

Local patch:

- `src/canonical.rs` uses a zero-duration `Instant` shim on `wasm32` targets. Native builds still use `std::time::Instant`.

Reason for vendoring:

- The pinned upstream verifier compiles to `wasm32-unknown-unknown` after `getrandom/js` and pure-Rust zstd decoding are configured, but `std::time::Instant::now()` traps at runtime in the browser target. Cargo cannot patch one file inside a remote git dependency, so the verifier crate is vendored with this minimal cfg-gated patch while `verilm-core` remains pinned to the upstream git commit.
