# RFC-0002: `vi` CLI surface

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.1

## Summary

The CLI is a single binary `vi` with four subcommands: `keygen`, `chat`, `verify`, `tui`. Default output is JSON to stdout, error output is JSON to stderr, exit codes are stable per [RFC-0014](./RFC-0014-error-taxonomy.md), and every public surface — flag names, subcommand names, JSON field names, exit code values — is part of the API contract.

## Motivation

The primary v1 persona is the Integrating Developer who pipes `vi` output into scripts and CI. The default must be machine-readable. Human-readable output is a `--pretty` opt-in. CommitLLM upstream ships scripts; we ship a CLI that turns those scripts into a one-command developer surface.

## Goals

- Each subcommand is a single shippable verb that maps cleanly to a user mental model.
- JSON output is the default, with a stable schema and `schema_version` field.
- Exit codes are exhaustive: every error path maps to one of the codes in [RFC-0014](./RFC-0014-error-taxonomy.md).
- Help text is part of the public API and is regression-tested.

## Non-Goals

- No interactive prompt unless the user asks for it (`vi tui`).
- No global config file in v1. Environment variables and flags only. A `~/.verifiable-intelligence/config.toml` is a v1.x consideration if usage shows it's needed.
- No multi-command pipelines built into the CLI. `vi chat | vi verify --stdin` is not in scope for v1.

## Proposed Design

See [02-public-api.md §1](../spec/02-public-api.md) for the full subcommand and flag enumeration. This RFC locks the decisions behind that surface.

### Argument parsing

- `clap` 4.x with derive macros.
- Subcommand discovery is explicit (no plugins).
- `--help` and `--version` everywhere.
- Long flags only for non-trivial options; short flags for very common ones (`-p` for `--prompt`, `-o` for `--output`, `-e` for `--endpoint`).

### Environment variables

| Var | Default | Effect |
|-----|---------|--------|
| `VI_ENDPOINT` | unset | Default for `--endpoint` on `chat`, `verify`, `tui` |
| `VI_API_KEY` | unset | Sent as `Authorization: Bearer ...` if set; never logged |
| `VI_LOG` / `RUST_LOG` | unset | Verbosity control ([05-observability.md](../spec/05-observability.md)) |
| `VI_NO_COLOR` / `NO_COLOR` | unset | Disables ANSI color in `--pretty` output |
| `VI_TRACE_ID` | unset | Override the auto-generated trace_id (debugging) |

`--api-key` flag is supported but discouraged; it ends up in shell history. `VI_API_KEY` is the documented path.

### Output discipline

- `stdout` is reserved for the structured result of the operation. `--pretty` formats it; the underlying object is the same.
- `stderr` is reserved for logs (when enabled) and the error envelope.
- A subcommand emits exactly one JSON object on stdout in `--pretty=false` mode (or none, if it errors).
- The error envelope on stderr is emitted exactly once per failed invocation.

### Schema evolution

Every subcommand's JSON object carries `schema_version: <u16>`. v1 ships `1` on all subcommands. Adding optional fields is non-breaking; adding required fields is breaking; renaming or removing is breaking. See [09-release-and-versioning.md](../spec/09-release-and-versioning.md).

### Backpressure and streaming

- `vi chat` does not stream tokens to stdout in v1. The full text is delivered in the JSON object. Streaming is a v1.x consideration; receipts are end-of-response in CommitLLM's protocol so streaming the receipt is intrinsically end-bound.
- `vi tui` does stream within its own UI; it owns terminal output, not stdout/stderr semantics.

### Help text

Help text uses the project's voice: direct, dense, no marketing. Examples in the help are real commands that actually work against the public CI fixtures (or the documented self-hosted recipe).

Snapshot tests in CI compare `vi <subcommand> --help` output to a fixture. Drift fails the build. This is part of why help text is in the public API.

## Alternatives Considered

**Default to `--pretty`, opt-in to JSON.** Common in many CLIs (git, kubectl). Rejected: primary persona is CI integration. Forcing them to remember `--json` is a paper cut they hit dozens of times a day.

**Sub-subcommands** (`vi keygen rotate`, `vi receipt inspect`, etc.). Rejected: v1 surface is small enough that one-level dispatch is sufficient. Adding depth is reversible later.

**A REPL mode for the CLI.** Rejected: the TUI exists for the demonstration use case; a REPL adds complexity without serving a distinct persona.

**Config file in v1.** Rejected: env vars + flags cover every documented workflow. Adding a config file invites disagreement about its location, format, and precedence; defer until usage demands it.

## Drawbacks

- `clap` derives are heavy on compile time. Worth it for the type-safe help-text generation.
- JSON-by-default surprises first-time CLI users on the command line; mitigation is a clear README example and a prominent `--pretty` callout in `--help`.

## Migration / Rollout

- No backward compatibility burden; new project.
- The CLI surface lands incrementally per the issue plan: `keygen` and `verify` first (the smallest end-to-end loop), `chat` second, `tui` last.

## Testing Strategy

- `--help` snapshot tests per subcommand.
- Exit code tests: each category has at least one path producing it.
- JSON schema validation against fixtures.
- Property tests on argument parsing (invalid inputs always produce `input` errors, never panics).
- An "evidence test" in CI: run each subcommand against a fixture, capture JSON, validate it against the published schema, compare to a stored golden output modulo timing fields.

## Open Questions

None.

## References

- [02-public-api.md](../spec/02-public-api.md)
- [RFC-0014](./RFC-0014-error-taxonomy.md)
- [RFC-0008](./RFC-0008-tui-architecture.md)
