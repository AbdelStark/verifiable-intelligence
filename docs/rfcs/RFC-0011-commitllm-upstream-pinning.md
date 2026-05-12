# RFC-0011: CommitLLM upstream pinning

- Status: Accepted (resolves PRD OQ-4)
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

The project pins to a specific CommitLLM upstream commit recorded in `commitllm.lock`. We pin to a **pre-rename commit** during v0.1 and v0.2, track the upstream rename PR (CommitLLM roadmap #49), and execute a coordinated pin change to a post-rename commit before v1.0. A pin change is a structured event with a CHANGELOG entry, a fixture-regeneration step, and explicit testing.

## Motivation

CommitLLM is the protocol implementation. It is actively developed and mid-rename (`verilm` → `commitllm`). We must:

- Build against a stable target so our own development is not chasing upstream churn.
- Migrate forward as upstream stabilizes; we are not a fork.
- Communicate the pin clearly to verifiers and provider operators so a mismatch is detectable.

Three approaches were on the table per [PRD OQ-4](../../PRD.md):

1. Pin to a pre-rename commit, wait for the rename, then move.
2. Wait for the rename, build only against post-rename commits.
3. Vendor CommitLLM and rename ourselves.

This RFC picks (1) and lays out the rename-window plan.

## Goals

- A single source of truth for the pin (`commitllm.lock`).
- Cargo and Docker both honor the pin.
- The pin is reflected in the receipt binding header and the provider's `/healthz`.
- A pin change is impossible to ship silently.
- A planned, coordinated migration to a post-rename commit.

## Non-Goals

- We do not modify the CommitLLM source on our side. If a fix is needed upstream, we open a PR there; we do not maintain a fork.
- We do not pin to a branch. Commits only.

## Proposed Design

### `commitllm.lock` format

```
# verifiable-intelligence pin
# Changing this requires:
#  1. Update Cargo.toml git rev to match.
#  2. Rebuild provider/ and verify smoke test passes.
#  3. Regenerate canonical fixture: scripts/regenerate-fixtures.sh
#  4. Add CHANGELOG entry under [Unreleased] / Pin section.
#  5. Verify tamper-fuzz still 100% passes.
commitllm = "lambdaclass/CommitLLM@<full-40-char-sha>"
commitllm_short = "<first-8-chars>"
pinned_at = "2026-05-12"
note = "Pre-rename pin; see RFC-0011."
```

`commitllm_short` is referenced from spec docs and binding headers; `commitllm` is the canonical identifier.

### Cargo wiring

```toml
# crates/vi-verifier/Cargo.toml
[dependencies]
commitllm-verifier = { git = "https://github.com/lambdaclass/CommitLLM", rev = "<sha>", package = "verilm-verifier" }
```

(The `package` keyword handles the pre-rename name. After the rename, both the `rev` and the `package` change in one commit.)

`Cargo.lock` is committed; `cargo update -p commitllm-verifier` is the controlled mechanism for moving the pin.

### Docker wiring

The provider's Dockerfile clones CommitLLM at the same commit:

```dockerfile
RUN git clone https://github.com/lambdaclass/CommitLLM /opt/commitllm \
 && cd /opt/commitllm \
 && git checkout <sha> \
 && pip install -e .
```

A build arg can override the SHA for local experimentation; production builds use the value from `commitllm.lock` via a `make` target that reads the file.

### Binding header advertisement

Every receipt and key carries `commitllm_pin` (8-char short SHA). The verifier checks that receipt pin == key pin. A mismatch fails closed with `identity_mismatch`.

### `/healthz` advertisement

The provider's `/healthz` returns `commitllm_pin`. `vi` can use this for an optional pre-flight check: if the provider's pin differs from the CLI's built-in pin, warn (do not fail, because CLI clients are often older than providers).

### Rename window plan

CommitLLM's upstream rename touches crate names (`verilm-*` → `commitllm-*`), import paths, and public types. The transition plan:

1. **Pre-rename phase (v0.1, v0.2):** Pin to a known-good pre-rename commit. Build, test, ship. This is where v1 development happens.
2. **Watch phase:** A maintainer subscribes to the upstream rename PR. When it merges, a `pin-bump` issue opens in this project automatically (via GitHub Actions watching `lambdaclass/CommitLLM`'s default branch for a tag or specific path change).
3. **Bump phase:** A single PR updates `commitllm.lock`, Cargo deps, Dockerfile, fixtures, and binding header value. CI runs full integration + tamper fuzz. CHANGELOG entry under Pin.
4. **Post-rename phase (v1.0 onward):** Pin to a post-rename commit. v1.0 cannot ship on a pre-rename pin without a documented exception in the release issue.

### Pin change checklist (binding rule)

A PR changing `commitllm.lock` MUST also:

- [ ] Update `Cargo.toml` `rev`.
- [ ] Update `Dockerfile` clone command.
- [ ] Regenerate `tests/fixtures/canonical-receipt.bin` and `canonical-key.bin`.
- [ ] Run `cargo test --workspace` green.
- [ ] Run `cargo test --test tamper-fuzz` green at N=100.
- [ ] Add `CHANGELOG.md` entry under `## [Unreleased]` → `### Pin`.
- [ ] Update README "Verified against CommitLLM pin: ..." badge.

A CI check parses the diff: if `commitllm.lock` changed without a matching CHANGELOG `### Pin` entry in the same PR, the build fails.

### Vendor escape hatch (out of scope, named for clarity)

If upstream becomes unmaintained or actively diverges in a direction incompatible with the project's goals, vendoring becomes an option. The escape hatch is documented but not adopted: it would be a v2 conversation. Not a v1 plan.

## Alternatives Considered

**Wait for the rename, build only against post-rename commits.** Rejected: blocks the project on an upstream timeline outside our control. CommitLLM's rename does not deliver value to us; the pre-rename API is sufficient for v1 features.

**Vendor CommitLLM and rename in-tree.** Rejected: increases maintenance burden, breaks the "we depend on upstream" contract, undermines G8 ("zero new cryptography").

**Track a release tag instead of a commit.** Rejected: CommitLLM does not maintain semver tags as a primary contract; commit SHA is what they recommend.

**Auto-bump via Dependabot or Renovate.** Rejected: too risky; pin changes need the full checklist above; a bot bypassing it would break fixtures or fuzz harness.

## Drawbacks

- Pinning to a pre-rename commit means the project name in code (`verilm-*`) does not match what upstream is moving toward. Mitigation: documented; cosmetic for now.
- A maintainer must actively watch the upstream rename to time the bump. Mitigation: an automated `pin-bump` issue triggers the work; it isn't on the maintainer to remember.

## Migration / Rollout

- v0.1: pin set on day one to the pre-rename commit chosen at project bootstrap.
- v0.2: pin remains unless a critical upstream fix lands.
- v1.0: rename completed (assuming upstream merges by then); pin bumped to a post-rename commit. If upstream has not merged by v1.0 cut, we ship v1.0 on the pre-rename pin and bump on v1.1; this is documented in the v1.0 release notes.

## Testing Strategy

- A test asserts that the value of `commitllm_short` in `commitllm.lock` equals the value embedded in compiled `vi`'s `--version` output and the value advertised by the provider's `/healthz`.
- CHANGELOG-pin link check: PRs touching `commitllm.lock` are scanned for a matching entry in CHANGELOG.
- Fixture regeneration script: a `scripts/regenerate-fixtures.sh` end-to-end runs against the current pin.

## Open Questions

None.

## References

- [PRD §11 OQ-4](../../PRD.md)
- CommitLLM upstream roadmap (item #49 — rename).
- [RFC-0003](./RFC-0003-receipt-format-pinning.md) for the binding-header field.
- [RFC-0009](./RFC-0009-tamper-fuzz-harness.md) for fixture regeneration.
