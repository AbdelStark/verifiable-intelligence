#!/usr/bin/env bash
# scripts/ci/test-actionlint.sh — verifies the workflow lint gate catches errors.

set -euo pipefail

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

broken_workflow="$tmpdir/broken-workflow.yml"
cat > "$broken_workflow" <<'YAML'
name: Broken
on: push
jobs:
  broken:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{"
YAML

if scripts/ci/actionlint.sh "$broken_workflow" >"$tmpdir/actionlint.log" 2>&1; then
  echo "expected actionlint to reject the deliberately broken workflow" >&2
  cat "$tmpdir/actionlint.log" >&2
  exit 1
fi

scripts/ci/actionlint.sh .github/workflows/*.yml
echo "actionlint self-test passed"
