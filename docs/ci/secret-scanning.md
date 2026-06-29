# Secret Scanning

CI runs `npm run test:secret-scan` on every pull request and push to `main`.
The scanner is a local `gitleaks`-equivalent for this repository: it scans the
tracked file set for high-signal credential shapes and fails closed on any
finding.

The scanner never prints matched secret values. It prints the file, line, rule
name, and a SHA-256 fingerprint so a maintainer can allowlist a confirmed
non-secret without copying the value into the repository.

## Allowlist Policy

False positives must be recorded in `secret-scan.allowlist.json` with:

- `path`: tracked repository path that contains the non-secret value.
- `type`: scanner rule name.
- `fingerprint`: the SHA-256 fingerprint printed by the failed scan.
- `reason`: why the value is safe to keep in the repository.

The allowlist has a repository-level `reviewed_at` date and
`review_interval_days`. CI fails when that review window is stale, even if the
allowlist is empty, so the false-positive list is reviewed periodically instead
of becoming permanent background state.

To refresh the review, inspect every entry, delete obsolete entries, then update
`reviewed_at` in the same pull request.
