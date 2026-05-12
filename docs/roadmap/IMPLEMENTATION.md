# Implementation Tracker — 2026-05-12

Generated from the spec corpus in PR [#1](https://github.com/AbdelStark/verifiable-intelligence/pull/1). Every implementable unit of work in the spec is filed below as a GitHub issue. Each issue is independently shippable; cross-issue dependencies are noted inline and as issue comments.

Counts: 16 tracking issues, 97 implementation/test/docs/release/gate/open-question issues, total 113 issues filed (issues #2–#115; #1 is the spec PR).

## Tracking issues (subsystem dashboards)

| # | Subsystem | Milestone |
|---|-----------|-----------|
| #2 | CLI (`vi-cli`) | v1.0 |
| #3 | TUI (`vi-tui`) | v1.0 |
| #4 | Verifier (`vi-verifier`) | v1.0 |
| #5 | Receipt codec (`vi-receipt`) | v1.0 |
| #6 | HTTP client (`vi-client`) | v1.0 |
| #7 | Keygen (`vi-keygen`) | v1.0 |
| #8 | Observability (`vi-log`) | v1.0 |
| #9 | Error taxonomy (`vi-errors`) | v1.0 |
| #10 | Provider image | v1.0 |
| #11 | Deployment recipes | v1.0 |
| #12 | Corridor measurement | v1.0 |
| #13 | Tamper fuzz harness | v1.0 |
| #14 | CI pipeline | v1.0 |
| #15 | JSON schemas & fixtures | v1.0 |
| #16 | Documentation | v1.0 |
| #17 | Release engineering | v1.0 |

## Milestone: v0.1 (provider + keygen/chat/verify against fixtures + corridor script + CI green)

| # | Title | Area | Priority | Effort | RFC |
|---|-------|------|----------|--------|-----|
| #18 | workspace: scaffold Cargo workspace and 8 crates | cli | p0 | s | RFC-0001 |
| #19 | workspace: cargo deny config and forbidden-edge check | ci | p0 | s | RFC-0001 |
| #20 | workspace: rustfmt + clippy + lints config | ci | p1 | s | RFC-0001 |
| #21 | workspace: `tui` feature flag for CLI-only build | cli | p2 | s | RFC-0001 |
| #22 | pin: set initial CommitLLM upstream pin | commitllm-pin | p0 | s | RFC-0011 |
| #23 | vi-errors: define ViError enum, exit-code map, category strings | errors | p0 | m | RFC-0014 |
| #24 | vi-errors: ErrorEnvelope JSON serialization + remediation | errors | p0 | s | RFC-0014 |
| #25 | vi-errors/vi-cli: main boundary, panic catch, clap-error mapping | errors | p0 | s | RFC-0014 |
| #26 | vi-receipt: envelope codec (magic + ver + flags) | receipt | p0 | m | RFC-0003 |
| #27 | vi-receipt: VIKY binding header codec + CRC32C | receipt | p0 | m | RFC-0003 |
| #28 | vi-receipt: VIRC receipt header codec + identity check | receipt | p0 | m | RFC-0003 |
| #29 | vi-receipt: VIAU audit header codec + binding check | receipt | p0 | m | RFC-0003 |
| #30 | vi-receipt: property/fuzz tests for envelope robustness | receipt | p0 | s | RFC-0003 |
| #31 | vi-keygen: canonical checkpoint hashing | keygen | p0 | m | RFC-0004 |
| #32 | vi-keygen: mirror download + resume + cache | keygen | p0 | m | RFC-0004 |
| #33 | vi-keygen: orchestrator + VIKY envelope emission | keygen | p0 | m | RFC-0004 |
| #34 | vi-keygen: determinism integration test (FR-17) | keygen | p0 | s | RFC-0004 |
| #35 | vi-keygen: key-size budget gate in CI | keygen | p1 | s | RFC-0004 |
| #36 | vi-client: HTTPS chat client + auth + trace header | client | p0 | m | RFC-0006 |
| #37 | vi-client: multipart/mixed response parsing + receipt extraction | client | p0 | m | RFC-0006 |
| #38 | vi-client: audit endpoint client | client | p0 | s | RFC-0006 |
| #40 | vi-client: mock-server integration tests | client | p0 | s | RFC-0006 |
| #41 | vi-verifier: dispatch pipeline + tier handling | verifier | p0 | l | RFC-0003 |
| #43 | vi-verifier: byte-deterministic VerifyReport tests | verifier | p0 | s | RFC-0003 |
| #44 | vi-verifier: identity-mismatch enforcement tests | verifier | p0 | s | RFC-0003 |
| #45 | vi-cli: clap skeleton + env vars + subcommand dispatch | cli | p0 | m | RFC-0002 |
| #46 | vi-cli: vi keygen subcommand wiring | cli | p0 | m | RFC-0002 |
| #47 | vi-cli: vi chat subcommand wiring | cli | p0 | m | RFC-0002 |
| #48 | vi-cli: vi verify subcommand wiring + tier validation | cli | p0 | m | RFC-0002 |
| #52 | vi-cli: schema_version field on every subcommand output | cli | p1 | s | RFC-0002 |
| #59 | vi-log: tracing subscriber + JSON formatter + EnvFilter | log | p1 | m | RFC-0015 |
| #60 | vi-log: RedactionLayer + field map enforcement | log | p0 | m | RFC-0015 |
| #61 | vi-log: span model + project-owned events emission | log | p1 | m | RFC-0015 |
| #63 | provider: multi-stage Dockerfile skeleton | provider | p0 | m | RFC-0005 |
| #64 | provider: bake W8A8 weights into image with hash gate | provider | p0 | s | RFC-0005 |
| #65 | provider: entrypoint.sh with vLLM launch + healthz | provider | p0 | m | RFC-0005 |
| #66 | provider: /healthz endpoint + readiness contract | provider | p0 | s | RFC-0005 |
| #67 | provider: rate limits + body size cap + max-tokens clamp | provider | p1 | m | RFC-0005 |
| #68 | provider: image-size budget gate | provider | p1 | s | RFC-0005 |
| #69 | deploy: provider/compose.yaml for self-hosted | provider | p0 | s | RFC-0007 |
| #73 | ci: CHANGELOG-pin lint for commitllm.lock | ci | p1 | s | RFC-0011 |
| #76 | quantize: scripts/quantize/quantize.py + recipe | provider | p0 | m | RFC-0012 |
| #77 | quantize: upload W8A8 mirror to HF + model card | provider | p0 | s | RFC-0012 |
| #78 | quantize: pin EXPECTED_CHECKPOINT_HASH constant | keygen | p0 | s | RFC-0012 |
| #79 | corridor: workload JSONLs | corridor | p0 | m | RFC-0010 |
| #80 | corridor: measure.py + aggregate.py + JSON output | corridor | p0 | l | RFC-0010 |
| #83 | fuzz: canonical receipt + key fixtures | verifier | p0 | s | RFC-0009 |
| #86 | ci: ci.yml end-to-end pipeline | ci | p0 | m | RFC-0013 |
| #89 | ci: provider image smoke build job | ci | p1 | s | RFC-0013 |
| #91 | ci: gitleaks secret scan job | ci | p1 | s | RFC-0013 |
| #93 | ci: actionlint on workflows | ci | p2 | s | RFC-0013 |
| #94 | schemas: keygen-output.schema.json | schemas | p1 | s | RFC-0002 |
| #95 | schemas: chat-output.schema.json | schemas | p1 | s | RFC-0002 |
| #96 | schemas: verify-report.schema.json | schemas | p1 | s | RFC-0002 |
| #97 | schemas: error-envelope.schema.json | schemas | p1 | s | RFC-0014 |
| #98 | schemas: CI schema-validation runner | schemas | p1 | s | RFC-0013 |
| #100 | docs: CHANGELOG.md initial | docs | p1 | s | RFC-0011 |
| #102 | docs: SECURITY.md | docs | p1 | s | RFC-0001 |
| #105 | docs: CODE_OF_CONDUCT.md | docs | p2 | s | — |
| #107 | release: cut v0.1.0 pre-release | release | p1 | s | — |

## Milestone: v0.2 (TUI, tamper fuzz, HF deploy recipe, comprehension prep)

| # | Title | Area | Priority | Effort | RFC |
|---|-------|------|----------|--------|-----|
| #39 | vi-client: /healthz preflight + pin-mismatch warn | client | p2 | s | RFC-0011 |
| #42 | vi-verifier: phase-event callback channel for TUI | verifier | p1 | m | RFC-0008 |
| #49 | vi-cli: vi tui dispatch into vi-tui library | cli | p1 | s | RFC-0002 |
| #50 | vi-cli: --pretty formatter + color discipline | cli | p1 | s | RFC-0002 |
| #51 | vi-cli: --help snapshot tests | cli | p1 | s | RFC-0002 |
| #53 | vi-tui: ratatui scaffold + event loop + layout | tui | p0 | m | RFC-0008 |
| #54 | vi-tui: chat pane with streaming | tui | p0 | m | RFC-0008 |
| #55 | vi-tui: verification phase-walk renderer | tui | p0 | m | RFC-0008 |
| #56 | vi-tui: --tamper byte-flip + F2 toggle | tui | p0 | s | RFC-0008 |
| #57 | vi-tui: --no-color symbol fallback | tui | p2 | s | RFC-0008 |
| #58 | vi-tui: frame snapshot tests for key states | tui | p1 | s | RFC-0008 |
| #62 | vi-log: trace_id propagation via X-* header | log | p1 | s | RFC-0015 |
| #70 | deploy: scripts/deploy/hf.sh | provider | p0 | m | RFC-0007 |
| #71 | deploy: docs/deployment/hf.md and self-hosted.md | docs | p1 | s | RFC-0007 |
| #72 | deploy: deploy-hf.yml workflow_dispatch | ci | p2 | s | RFC-0013 |
| #74 | tooling: scripts/regenerate-fixtures.sh | commitllm-pin | p1 | s | RFC-0011 |
| #75 | ci: watch CommitLLM rename merge; auto-open bump issue | commitllm-pin | p2 | s | RFC-0011 |
| #81 | corridor: corridor.yml workflow_dispatch | ci | p1 | s | RFC-0013 |
| #82 | corridor: tiny CPU smoke test in CI | corridor | p2 | s | RFC-0010 |
| #84 | fuzz: tamper-fuzz harness per-PR at N=100 | verifier | p0 | s | RFC-0009 |
| #85 | fuzz: nightly N=1000 with auto-issue | ci | p1 | s | RFC-0009 |
| #87 | ci: nightly.yml | ci | p1 | s | RFC-0013 |
| #90 | ci: fresh-environment end-to-end timing (SM-1) | ci | p0 | m | RFC-0013 |
| #92 | ci: size-budget gate + nightly perf benchmarks | ci | p1 | s | RFC-0013 |
| #99 | docs: rewrite README with quickstart + preamble | docs | p0 | m | RFC-0002 |
| #101 | docs: CONTRIBUTING.md | docs | p1 | s | RFC-0001 |
| #103 | docs: docs/measurements/corridor.md template | docs | p1 | s | RFC-0010 |
| #104 | docs: docs/ci/README.md + red-build.md + gpu-runners.md | docs | p2 | s | RFC-0013 |
| #106 | docs: docs/user-guide.md | docs | p1 | s | RFC-0002 |
| #108 | release: cut v0.2.0 pre-release | release | p1 | s | — |
| #114 | open-question: OQ-5 HF Endpoint limits spike | provider | p0 | m | RFC-0007 |

## Milestone: v1.0 (public release; all SM gates green)

| # | Title | Area | Priority | Effort |
|---|-------|------|----------|--------|
| #88 | ci: release.yml | release | p0 | m |
| #109 | release: cut v1.0.0 public release (gated) | release | p0 | m |
| #110 | release: docs/release/yank.md | release | p2 | s |
| #111 | gate: SM-5 README comprehension review | docs | p0 | m |
| #112 | gate: SM-6 TUI non-cryptographer comprehension | tui | p0 | m |
| #113 | open-question: OQ-1 repo organization | — | p1 | s |
| #115 | open-question: OQ-6 public demo endpoint | provider | p1 | s |

## Cross-cutting dependencies

- **#18 (workspace bootstrap)** blocks the entire downstream graph: error/crate/CI work cannot land before the workspace exists.
- **#22 (CommitLLM pin)** blocks #41 (verifier dispatch), #63 (Dockerfile), #73 (pin lint).
- **#23 (ViError enum)** blocks every leaf crate's typed error conversion.
- **#33 (keygen orchestrator)** depends on #31, #32, #27, #22, #78.
- **#41 (verifier dispatch)** depends on #26, #27, #28, #22, #23; blocks #43, #44, #48.
- **#65 (provider entrypoint)** depends on #63 and #22; blocks #66, #69, #70, #114.
- **#90 (fresh-env timing job)** depends on #45-#48 (full CLI loop) and #69 (compose for service container); gates SM-1 on #109.
- **#109 (v1.0 cut)** is gated by all SM gates: #90 (SM-1), #92 (SM-2 perf), #84 (SM-3 fuzz over time), #80 (SM-4 corridor), #111 (SM-5), #112 (SM-6), and resolutions of #113 (OQ-1) and #115 (OQ-6).
- **#114 (OQ-5)** blocks #70 finalization and gates whether HF is the README's recommended deployment path.

## Open questions tracker

| OQ | Issue | Default | Trigger |
|----|-------|---------|---------|
| OQ-1 | #113 | personal org | Before v1.0 |
| OQ-5 | #114 | self-hosted as reference; HF "may require tuning" | 1-week build-phase spike |
| OQ-6 | #115 | self-hosted-only at launch | 2 weeks before v1.0 |

OQ-2, OQ-3, OQ-4, OQ-7 are resolved by RFCs (RFC-0012, RFC-0006, RFC-0011, RFC-0010 respectively).
