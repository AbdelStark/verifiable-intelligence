# Contributing to verifiable-intelligence

This repository is a research proof of concept for verifiable open-weight LLM
inference markets. The current v1 target is browser-first: a buyer chooses a
provider, submits a prompt, receives an answer with a proof bundle, and verifies
the model, prompt binding, decode policy, and delivered answer through
CommitLLM-backed artifacts.

Do not add flows for unauthorized token resale, stolen credentials, closed-weight
model claims, or provider-term bypass. The demo may model quotes and toy credits,
but real money, API-key resale, and unsupported frontier-model attestation are
out of scope.

Start with [`SPEC.md`](./SPEC.md), then read:

- [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md) for participation and enforcement
  expectations.
- [`docs/rfcs/RFC-0001-workspace-and-crate-layout.md`](./docs/rfcs/RFC-0001-workspace-and-crate-layout.md)
  for the Rust workspace and crate boundaries that still apply.
- [`docs/rfcs/RFC-0009-tamper-fuzz-harness.md`](./docs/rfcs/RFC-0009-tamper-fuzz-harness.md)
  for the receipt tamper-detection contract, now extended by the proof-bundle
  red-path suite.
- [`docs/rfcs/RFC-0011-commitllm-upstream-pinning.md`](./docs/rfcs/RFC-0011-commitllm-upstream-pinning.md)
  for CommitLLM pin discipline.
- [`docs/rfcs/RFC-0016-marketplace-demo-pivot.md`](./docs/rfcs/RFC-0016-marketplace-demo-pivot.md)
  for the browser marketplace pivot that supersedes the old TUI-first plan.

## Repository Layout

- `demo/`: static browser proof-market demo.
- `broker/`: fixture broker and local API skeleton for catalog, quote, chat, and
  audit flows.
- `fixtures/viex/`: proof-bundle fixtures, including happy path and red paths.
- `schemas/`: JSON Schemas for proof bundles, CLI outputs, and verifier reports.
- `verifier/wasm/`: browser verifier harness and pinned CommitLLM fixture bridge.
- `provider/`: provider container stub, compose file, entrypoint, and health check.
- `crates/`: Rust workspace for CLI utilities, keygen, verifier, receipt, logging,
  client, and shared error contracts.
- `docs/spec/` and `docs/rfcs/`: normative project specification corpus.
- `scripts/`: CI, schema, bundle, provider, docs, and release validation helpers.

The Rust TUI crate remains in the workspace for compatibility with earlier
issues, but it is not the v1 product surface. New contributor work should
default to the browser demo, proof bundle, broker/provider adapter, WASM verifier,
or supporting CLI utility paths unless an issue explicitly says otherwise.

## Build

Install JavaScript dependencies first:

```bash
npm install
```

Useful browser and bundle commands:

```bash
npm run test:demo
npm run test:bundle
npm run build:wasm
npm run test:wasm
```

Useful Rust commands:

```bash
cargo build --workspace --all-targets
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets
```

The local Docker daemon is only needed for provider image work. If Docker is not
available, still run the provider entrypoint tests that do not require the daemon:

```bash
npm run test:provider
```

## Tests

Choose the narrowest test set that covers the change, then run the broader bundle
gate before opening or updating a PR when practical.

- Demo UI or copy: `npm run test:demo` and `npm run test:docs`.
- Proof bundle, schema, or fixture changes: `npm run test:schema`,
  `npm run test:tamper`, and `npm run test:bundle`.
- Broker or provider contract changes: `npm run test:broker`,
  `npm run test:provider`, and `npm run test:bundle`.
- CommitLLM pin, verifier, or WASM changes: `npm run test:pin`,
  `npm run test:wasm`, `npm run test:changelog-pin`, and relevant Rust tests.
- CLI output or error changes: `npm run test:schema`,
  `npm run test:error-envelopes`, and `cargo test --workspace`.
- CI workflow changes: `npm run test:workflows`.

For documentation-only changes, run:

```bash
npm run test:docs
```

Run `git diff --check` before committing to catch whitespace and conflict markers.

## Updating Fixtures

Fixtures are part of the public contract. Update them deliberately, not as opaque
snapshots.

When changing `VIEX` proof bundles:

1. Update `schemas/viex.schema.json` if the contract changes.
2. Update the minimal affected files under `fixtures/viex/`.
3. Keep red-path fixtures readable: a reviewer should be able to see whether the
   fixture models a prompt mismatch, model swap, answer rewrite, receipt tamper,
   expired quote, missing receipt, or wrong key.
4. Run `npm run test:schema`, `npm run test:tamper`, and `npm run test:bundle`.

When changing CLI JSON snapshots:

1. Update the affected file under `crates/vi-cli/tests/snapshots/output/`.
2. Update the matching schema under `schemas/` if the public contract changed.
3. Run `npm run test:schema` and `cargo test --workspace`.

When changing WASM verifier fixtures:

1. Record the CommitLLM pin and fixture source in the PR body.
2. Keep binary fixture changes paired with the JSON proof bundle that explains
   them.
3. Run `npm run build:wasm` and `npm run test:wasm`.

The older RFCs describe a future `scripts/regenerate-fixtures.sh` flow for live
provider receipt fixtures. Until that script exists, do not hand-edit live
CommitLLM binary fixtures without documenting the source command, pin, model
identity, verifier key hash, and audit payload in the PR.

## Bumping the CommitLLM Pin

The CommitLLM pin lives in [`commitllm.lock`](./commitllm.lock). A pin bump must
be one coherent PR that updates every place the pin is consumed.

Checklist:

1. Update `commitllm.lock`.
2. Update Cargo, Docker, provider health, fixture, and verifier references that
   embed or expect the old pin.
3. Add a `CHANGELOG.md` entry under `## [Unreleased]` -> `### Pin`.
4. Regenerate affected proof or verifier fixtures and document how they were
   produced.
5. Run `npm run test:pin`, `npm run test:changelog-pin`, relevant bundle/WASM
   tests, and `cargo test --workspace`.

Do not vendor or fork CommitLLM as part of a pin bump unless a separate RFC or
issue explicitly approves that direction.

## Pull Requests

- Use a focused branch and keep one issue or coherent slice per PR.
- Reference the issue number in the PR body.
- Explain the user-facing or contributor-facing impact, not only the files
  touched.
- List the exact validation commands run, including commands that could not be
  run and why.
- Keep claim language conservative: proof verifies supported execution-integrity
  properties; it does not prove factual correctness or closed-weight provider
  identity.
- Preserve unrelated local or generated files. Stage only files that belong to
  the PR.
- Ask for contributor or maintainer review before merge. For docs, reviewers
  should verify that the guide still matches the active browser marketplace demo
  scope and does not reintroduce superseded TUI-first assumptions.

If a PR changes a public contract, update the matching docs, schemas, fixtures,
and tests in the same branch.
