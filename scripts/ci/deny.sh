#!/usr/bin/env bash
# scripts/ci/deny.sh — workspace supply-chain + forbidden-edge gate.
#
# Runs `cargo deny check` against deny.toml (advisories, bans, licenses,
# sources) and the manifest-layer forbidden-edge checker that enforces
# RFC-0001 §"Forbidden edges". ci.yml will invoke this under #86.

set -euo pipefail

cargo deny check
python3 scripts/ci/forbidden_edges.py
python3 scripts/ci/test_forbidden_edges.py
