#!/usr/bin/env bash
# scripts/ci/lints.sh — workspace lint gate consumed by ci.yml.
#
# Runs the formatter check and the deny-warnings clippy pass on every target.
# RFC-0001 §"Cargo lints and conventions" requires both to be green on PRs.

set -euo pipefail

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
