# RFC-0001: Workspace and crate layout

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

The repository is a single Cargo workspace with a deliberately split crate graph: one user-facing binary crate (`vi-cli`), one binary for the TUI (`vi-tui`), and a small set of leaf libraries with strict dependency direction. Leaf crates have no runtime, no networking, no async, and no Cargo features beyond what they intrinsically need. MSRV matches CommitLLM upstream's MSRV plus our additional dependencies; stable Rust only.

**Pivot note, 2026-06-29:** this workspace remains useful for verifier, receipt, keygen, fixtures, and CI utilities. It no longer describes the whole v1 product, because [RFC-0016](./RFC-0016-marketplace-demo-pivot.md) adds a browser demo, proof bundle, and optional broker.

## Motivation

The CLI and the TUI share the same protocol-level logic (key generation, receipt parsing, verification, networking) but differ in their UX layer. Without a workspace split, either: (a) the TUI link-depends on argument-parsing surface it never uses, or (b) the CLI link-depends on `ratatui` and the dependency tree explodes. Both inflate the binary, both slow incremental rebuilds, both make it harder to extract a library for downstream users in v1.x.

The CommitLLM crates are the protocol's home; we depend on them, we do not re-implement them. The workspace must make that dependency boundary explicit and audit-able.

## Goals

- Distinct binary targets for CLI and TUI with shared library code.
- Leaf crates (`vi-receipt`, `vi-errors`) with no async runtime and minimal dependency surface.
- A `vi-verifier` crate that compiles to `wasm32-unknown-unknown`; after RFC-0016 this is a v1 spike, not a v1.1 deferral.
- Reproducible builds: `Cargo.lock` is checked in; MSRV is declared.
- A path for downstream consumers to depend on `vi-verifier` or `vi-receipt` directly (post-v1 documentation task).

## Non-Goals

- No alternative build systems (Bazel, Buck). Cargo only.
- No multi-language polyglot in v1 (no Python crate, no JS crate).
- No `no_std` support for `vi-receipt` in v1; it's a future option but adds work without v1 payoff.
- No public release of leaf crates to crates.io in v1; only `verifiable-intelligence` (the umbrella) ships. Leaf crates ship in v1.x or v2 with stable internal APIs.

## Proposed Design

### Workspace layout

```
verifiable-intelligence/
  Cargo.toml                # workspace manifest
  Cargo.lock                # checked in
  rust-toolchain.toml       # pins channel: stable, MSRV
  crates/
    vi-cli/                 # bin: vi
    vi-tui/                 # bin: vi-tui (wired into vi via subcommand dispatch)
    vi-client/              # lib: HTTP client
    vi-verifier/            # lib: verification wrapper around CommitLLM verifier
    vi-keygen/              # lib: key generation
    vi-receipt/             # lib: envelope codec
    vi-log/                 # lib: tracing setup, redaction
    vi-errors/              # lib: error types, exit codes
  schemas/                  # JSON schemas for CLI outputs
  scripts/                  # corridor measurement, deployment, fixture regen
  tests/                    # workspace-level integration tests
  docs/                     # spec + RFCs + roadmap (this corpus)
  provider/                 # Dockerfile, compose.yaml, entrypoint
```

The `vi tui` invocation dispatches into the `vi-tui` library; there are not two separate end-user binaries. From a packaging standpoint, one binary `vi` is published.

### Crate responsibilities and dependency direction

```
vi-cli ─┬─▶ vi-client ─▶ vi-receipt ─▶ vi-errors
        ├─▶ vi-verifier ─▶ (commitllm-verifier crate)
        │       └─▶ vi-receipt
        ├─▶ vi-keygen ─▶ vi-receipt
        ├─▶ vi-log
        └─▶ vi-tui (lib) ─▶ ratatui, vi-client, vi-verifier
```

Forbidden edges (CI-enforced via `cargo deny --hide-inclusion-tree`):

- `vi-receipt` → anything except `vi-errors`.
- `vi-verifier` → `vi-client` (no network in the verifier).
- `vi-cli` → `vi-tui` (the binary's tui subcommand uses `vi-tui` as a library; `vi-tui` does not depend on `vi-cli`).
- Any crate → `vi-cli`.

### Public re-exports

`verifiable-intelligence` (the umbrella crate published to crates.io for `cargo install`) is a thin wrapper that depends on `vi-cli` and re-exports nothing user-facing. The leaf crates are not published in v1; downstream Rust users wait for v1.1 or v2 once internal APIs are stable.

### MSRV

- Rust stable channel.
- MSRV is the maximum of CommitLLM's MSRV and the MSRV implied by our additional deps (`tokio`, `ratatui`, `reqwest`, `tracing`, etc.).
- Recorded in `rust-toolchain.toml`.
- CI builds against MSRV and against `stable` on every PR.

### Features

- No `default-features = false` cleverness in v1; every crate ships its dependencies enabled.
- The umbrella crate exposes one feature flag, `tui`, defaulting on. Disabling it produces a CLI-only build (`cargo install verifiable-intelligence --no-default-features`). Useful for minimal-footprint CI integration where `ratatui` is unwanted.

### Cargo lints and conventions

- `#![deny(warnings)]` in CI, not in source.
- `clippy::all`, `clippy::pedantic` (selectively allowed), `clippy::cargo` run in CI.
- `rustfmt` enforced.
- `cargo deny` configured to forbid GPL-class licenses (we are MIT and must stay distribution-compatible).

## Alternatives Considered

**Single binary crate, no workspace.** Simpler. Rejected: `vi tui` then depends on `ratatui` for users who only ever want CLI; tree-shaking is not a thing at the link level for Rust.

**Two completely separate binaries (`vi`, `vi-tui`).** Rejected: an integrating developer has to install two things; a screencast presenter has to install both. The shipped surface is one binary with one subcommand for the TUI.

**Mono-crate with feature flags everywhere.** Rejected: feature combinatorics in CI explode; downstream consumers can't depend on the verifier without pulling argument-parsing code.

**Split into more crates (per phase, per primitive).** Rejected: premature decomposition; the CommitLLM upstream already factors at the right granularity for the protocol parts; over-splitting adds CI burden without payoff.

## Drawbacks

- Workspace incremental build is slightly slower than mono-crate for the smallest changes.
- More `Cargo.toml` files to maintain.
- Forbidden-edge enforcement adds a CI step. Worth it; without it, the boundaries decay in months.

## Migration / Rollout

- The repository is empty at the time of this RFC. There is no migration. The first implementation issue creates the workspace skeleton.
- A `cargo new --lib` for each leaf crate, then wire dependencies per the diagram.
- Forbidden-edge CI check lands with the skeleton, not later.

## Testing Strategy

- Per-crate unit tests as specified in [07-testing-strategy.md](../spec/07-testing-strategy.md).
- A `cargo build --all-targets` and `cargo test --workspace` on every PR.
- A `cargo build --no-default-features` smoke test on every PR (umbrella CLI-only build).
- `cargo deny check` for license, advisories, banned crates, forbidden edges.

## Open Questions

None at this layer; choices below are decided.

## References

- [CommitLLM upstream](https://github.com/lambdaclass/CommitLLM) and its crate layout.
- [01-architecture.md](../spec/01-architecture.md), §"Module boundaries (normative)".
