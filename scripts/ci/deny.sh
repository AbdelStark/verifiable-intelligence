#!/usr/bin/env bash
# scripts/ci/deny.sh — workspace supply-chain + forbidden-edge gate.
#
# Runs `cargo deny check` against deny.toml (advisories, bans, licenses,
# sources) and the manifest-layer forbidden-edge checker that enforces
# RFC-0001 §"Forbidden edges". ci.yml will invoke this under #86.

set -euo pipefail

# The allowlist intentionally includes MIT-compatible licenses that are not in
# the tiny bootstrap graph yet; keep CI quiet without weakening actual denials.
cargo deny check -A license-not-encountered
python3 scripts/ci/forbidden_edges.py
python3 scripts/ci/test_forbidden_edges.py
