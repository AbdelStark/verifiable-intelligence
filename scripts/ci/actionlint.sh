#!/usr/bin/env bash
# scripts/ci/actionlint.sh — GitHub Actions workflow lint gate.

set -euo pipefail

ACTIONLINT_VERSION="${ACTIONLINT_VERSION:-v1.7.7}"

if command -v actionlint >/dev/null 2>&1; then
  exec actionlint "$@"
fi

if command -v go >/dev/null 2>&1; then
  exec go run "github.com/rhysd/actionlint/cmd/actionlint@${ACTIONLINT_VERSION}" "$@"
fi

echo "actionlint is not installed and go is unavailable for fallback install" >&2
exit 127
