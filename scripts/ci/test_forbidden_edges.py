#!/usr/bin/env python3
"""Negative-path tests for `forbidden_edges.check`.

Synthesises manifest dicts that violate each RFC-0001 rule and asserts the
checker flags exactly the right edge. This is the test PR #19 calls for.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from forbidden_edges import check  # noqa: E402


def make_manifest(name: str, deps: dict[str, dict | str] | None = None) -> dict:
    return {"package": {"name": name}, "dependencies": deps or {}}


def base_manifests() -> dict[str, dict]:
    """A clean workspace skeleton matching RFC-0001 - check() returns []."""
    return {
        "vi-errors": make_manifest("vi-errors"),
        "vi-receipt": make_manifest("vi-receipt", {"vi-errors": {"path": "../vi-errors"}}),
        "vi-log": make_manifest("vi-log"),
        "vi-client": make_manifest(
            "vi-client",
            {"vi-errors": {"path": "../vi-errors"}, "vi-receipt": {"path": "../vi-receipt"}},
        ),
        "vi-verifier": make_manifest(
            "vi-verifier",
            {"vi-errors": {"path": "../vi-errors"}, "vi-receipt": {"path": "../vi-receipt"}},
        ),
        "vi-keygen": make_manifest(
            "vi-keygen",
            {"vi-errors": {"path": "../vi-errors"}, "vi-receipt": {"path": "../vi-receipt"}},
        ),
        "vi-tui": make_manifest(
            "vi-tui",
            {
                "vi-client": {"path": "../vi-client"},
                "vi-errors": {"path": "../vi-errors"},
                "vi-verifier": {"path": "../vi-verifier"},
            },
        ),
        "vi-cli": make_manifest(
            "vi-cli",
            {
                "vi-client": {"path": "../vi-client"},
                "vi-errors": {"path": "../vi-errors"},
                "vi-keygen": {"path": "../vi-keygen"},
                "vi-log": {"path": "../vi-log"},
                "vi-receipt": {"path": "../vi-receipt"},
                "vi-verifier": {"path": "../vi-verifier"},
                "vi-tui": {"path": "../vi-tui", "optional": True},
            },
        ),
        "verifiable-intelligence": make_manifest(
            "verifiable-intelligence", {"vi-cli": {"path": "../vi-cli"}}
        ),
    }


def expect_clean() -> None:
    violations = check(base_manifests())
    assert violations == [], f"baseline should be clean, got: {violations}"


def expect_violation(mutate, fragment: str) -> None:
    m = base_manifests()
    mutate(m)
    violations = check(m)
    assert violations, f"expected a violation matching '{fragment}', got none"
    assert any(fragment in v for v in violations), (
        f"expected '{fragment}' in {violations}"
    )


def mutate_receipt_pulls_client(m: dict[str, dict]) -> None:
    m["vi-receipt"]["dependencies"]["vi-client"] = {"path": "../vi-client"}


def mutate_verifier_pulls_client(m: dict[str, dict]) -> None:
    m["vi-verifier"]["dependencies"]["vi-client"] = {"path": "../vi-client"}


def mutate_random_crate_pulls_cli(m: dict[str, dict]) -> None:
    m["vi-log"]["dependencies"]["vi-cli"] = {"path": "../vi-cli"}


def mutate_cli_tui_not_optional(m: dict[str, dict]) -> None:
    m["vi-cli"]["dependencies"]["vi-tui"] = {"path": "../vi-tui"}


def main() -> int:
    expect_clean()
    expect_violation(mutate_receipt_pulls_client, "vi-receipt must depend only on vi-errors")
    expect_violation(mutate_verifier_pulls_client, "vi-verifier must not depend on vi-client")
    expect_violation(mutate_random_crate_pulls_cli, "must not depend on vi-cli")
    expect_violation(
        mutate_cli_tui_not_optional, "vi-cli must reach vi-tui only as an optional dependency"
    )
    print("test_forbidden_edges: 5/5 ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
