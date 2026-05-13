#!/usr/bin/env python3
"""Enforce RFC-0001 §"Forbidden edges" by reading workspace `Cargo.toml` files.

Walks every member of the Cargo workspace and asserts the manifest-level
dependency graph matches RFC-0001:

  vi-receipt   may depend only on vi-errors.
  vi-verifier  must NOT depend on vi-client.
  vi-cli       must NOT depend on vi-tui as a non-optional dep
                (vi-tui is reached as an optional library behind the `tui`
                feature; see RFC-0001 §"Crate responsibilities").
  Any crate    must NOT depend on vi-cli.

The script intentionally inspects `Cargo.toml`, not `cargo metadata`, because
this is a manifest-layer invariant the RFC pins. A transitive-graph check
would silently allow a crate to add `vi-cli` as a manifest dep so long as no
function call exercised it; that is not the bar RFC-0001 sets.

Exit code 0 on success, 1 on the first forbidden edge encountered.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CRATES_DIR = REPO_ROOT / "crates"

# Forbidden-edge rules. Each entry: (crate, predicate, message).
ALLOWED_RECEIPT_DEPS = {"vi-errors"}


def deps_of(manifest: dict) -> set[str]:
    """Return the union of dependency names declared at the manifest layer."""
    names: set[str] = set()
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        names.update((manifest.get(section) or {}).keys())
    # Target-specific deps: `[target.'cfg(...)'.dependencies]`.
    for target in (manifest.get("target") or {}).values():
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            names.update((target.get(section) or {}).keys())
    return names


def load_member_manifests() -> dict[str, dict]:
    """Return {crate_name: manifest_dict} for every workspace member."""
    out: dict[str, dict] = {}
    for cargo_toml in CRATES_DIR.glob("*/Cargo.toml"):
        with cargo_toml.open("rb") as fh:
            manifest = tomllib.load(fh)
        name = manifest["package"]["name"]
        out[name] = manifest
    return out


def check(manifests: dict[str, dict]) -> list[str]:
    """Return the list of forbidden-edge violations, empty on success."""
    violations: list[str] = []

    # 1. vi-receipt may depend only on vi-errors.
    if "vi-receipt" in manifests:
        bad = deps_of(manifests["vi-receipt"]) - ALLOWED_RECEIPT_DEPS
        # Strip non-workspace crates so we only police *intra-workspace* edges.
        bad = {d for d in bad if d in manifests}
        if bad:
            violations.append(
                "vi-receipt must depend only on vi-errors; "
                f"found forbidden workspace dep(s): {sorted(bad)}"
            )

    # 2. vi-verifier must not depend on vi-client.
    if "vi-verifier" in manifests and "vi-client" in deps_of(manifests["vi-verifier"]):
        violations.append("vi-verifier must not depend on vi-client (no network in the verifier)")

    # 3. Nothing may depend on vi-cli.
    for name, manifest in manifests.items():
        if name == "vi-cli":
            continue
        if name == "verifiable-intelligence":
            # The umbrella *is* the published wrapper around vi-cli per RFC-0001.
            continue
        if "vi-cli" in deps_of(manifest):
            violations.append(f"{name} must not depend on vi-cli; only the umbrella may")

    # 4. vi-cli must reach vi-tui only as an *optional* dep (the `tui` feature).
    if "vi-cli" in manifests:
        deps = manifests["vi-cli"].get("dependencies") or {}
        tui = deps.get("vi-tui")
        if tui is not None and not (isinstance(tui, dict) and tui.get("optional")):
            violations.append(
                "vi-cli must reach vi-tui only as an optional dependency "
                "(behind the `tui` feature)"
            )

    return violations


def main() -> int:
    manifests = load_member_manifests()
    if not manifests:
        print("forbidden_edges: no workspace members found under crates/", file=sys.stderr)
        return 1
    violations = check(manifests)
    if violations:
        for v in violations:
            print(f"forbidden_edges: {v}", file=sys.stderr)
        return 1
    print(f"forbidden_edges: ok ({len(manifests)} crates)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
