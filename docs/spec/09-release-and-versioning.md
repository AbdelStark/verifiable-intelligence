# 09 — Release and Versioning

## Versioning model

The project ships a single user-facing version, applied to:

- The `vi` CLI binary (`vi --version`).
- The `verifiable-intelligence` crate published to crates.io.
- The provider Docker image tag.
- GitHub release tags.

Semantic versioning (`MAJOR.MINOR.PATCH`):

- **MAJOR**: breaking change to any public surface defined in [02-public-api.md](./02-public-api.md), including: exit code map, JSON schema field removal/rename, magic prefixes, HTTP path prefix, error category removal/rename.
- **MINOR**: additive changes: new subcommand, new optional flag, new optional JSON field, new error category, new phase emitted in the report.
- **PATCH**: bug fixes, performance improvements, documentation, internal refactors with no observable surface change.

The CommitLLM upstream version (`commitllm_pin`) is independent. A change in upstream pin that breaks our integration is MAJOR even if upstream's own version did not bump; a change that is fully backward-compatible at our surface is MINOR or PATCH.

## Schema versioning

Three independent schema-version dimensions:

1. **Envelope `ver` byte** — bumped on binary layout change to the envelope.
2. **`*_schema_version` u16 fields inside envelopes** — bumped on additive changes within an envelope's payload (keygen binding, receipt header, audit header).
3. **`schema_version` field in CLI JSON outputs** — independent per subcommand.

Compatibility matrix (v1 target):

| Surface | Versions supported by `vi verify` v1.x |
|---------|----------------------------------------|
| Receipt envelope `ver` | `1` |
| Receipt header `receipt_schema_version` | `1` |
| Key envelope `ver` | `1` |
| Key header `keygen_schema_version` | `1` |
| Audit envelope `ver` | `1` |
| Audit header `audit_schema_version` | `1` |
| CLI output `schema_version` per subcommand | `1` |

A v2 of any schema requires a paired CLI release. The CLI prints a clear error on unknown versions ([04-error-model.md](./04-error-model.md)).

## Release channels

| Channel | Audience | Cadence | Artifacts |
|---------|----------|---------|-----------|
| `main` | Contributors, brave users | continuous | crates.io publish on every tag; Docker `:edge` on every main push |
| `v0.x` (pre-release) | Internal demo, sponsors | weekly during build | Docker `:v0.x.y`, GitHub Release "Pre-release" |
| `v1.0` | Public | one-time | Docker `:v1.0.0`, `:latest`; GitHub Release; crates.io; README announcement |
| `v1.x` | Public | as needed | same as v1.0 |

## Deprecation policy

- **Deprecation window**: a minimum of one MINOR release between marking a public surface deprecated and removing it.
- **Mark**: deprecated surfaces emit a `WARN` log line on use and include `"warnings": ["deprecated:<symbol>"]` in JSON output.
- **Removal**: only in a MAJOR.
- **Communication**: deprecations and removals are listed in `CHANGELOG.md` and announced in the GitHub Release notes.

## Changelog discipline

`CHANGELOG.md` at the repo root, Keep-a-Changelog format. Sections per release:

- **Added** — new public surface.
- **Changed** — behavioral changes without surface break.
- **Deprecated** — surfaces marked for removal.
- **Removed** — surfaces removed (MAJOR only).
- **Fixed** — bug fixes.
- **Security** — security-relevant fixes.
- **Schemas** — schema version bumps and what changed.
- **Pin** — CommitLLM pin changes.

Every PR that changes any public surface MUST add an entry to `[Unreleased]`. CI lints for this on PRs touching public crates.

## Release artifacts

Per public release:

- GitHub Release with binaries for Linux x86_64 and macOS arm64, SHA-256 checksums, and a signed release manifest. Windows binary attached if it built; not required.
- `cargo publish` for `verifiable-intelligence` (the user-facing crate; underlying crates published only as needed).
- Docker image push to the documented registry, tagged with the semver tag and `:latest`.
- README badge updates (corridor numbers, perf summary).
- A pinned issue (or Discussions thread) for release feedback.

## Yanking

A release found to fail any of the gates in [00-overview.md §Success criteria](./00-overview.md) after publication is yanked:

- `cargo yank`.
- Docker image: not deleted (immutability) but tagged `:yanked-<reason>`.
- GitHub Release marked "yanked" with explanation.
- A patch release follows.

## CommitLLM pin changes

A change to `commitllm_pin` is treated as a schema event:

- If the upstream change is internal-only and our integration tests pass without modification, PATCH.
- If new behaviour or new fields appear that our CLI exposes, MINOR.
- If a binary layout we expose changes or a required tier disappears, MAJOR.

Every pin change includes a `CHANGELOG.md` entry under **Pin** with the old SHA, new SHA, and a one-line rationale.

## Pre-1.0 caveat

During v0.x, schemas may evolve without the deprecation window above. The first MINOR after v1.0 inherits the strict policy. v0.x → v1.0 is the inflection point; we plan for it but do not gate v0.x experimentation by it.

## Branch policy

- `main` is always green. PRs are squash-merged.
- Release branches `release/v<major>.<minor>` exist for the duration needed to backport fixes for that minor.
- v1 has no backport plan beyond critical security; minor releases supersede prior minors.
