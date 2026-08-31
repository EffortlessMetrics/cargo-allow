#!/usr/bin/env python3
"""Self-test for scripts/feature-config-routing.py (#3905 PR D).

Runs the router as a subprocess against synthetic changed-file sets and
asserts the exact single-line JSON contract on stdout. The expected row
lists are the checked-in projection's declaration order; a matrix change
that reorders or renames rows must update this characterization in the
same PR. Exits non-zero with a clear message on the first mismatch.
Stdlib only.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROUTER = Path(__file__).resolve().parent / "feature-config-routing.py"

ALLOW_RUST_ROWS = [
    "allow-rust.default",
    "allow-rust.minimal-model",
    "allow-rust.syntax-explicit",
]
ALLOW_FILES_ROWS = [
    "allow-files.default",
    "allow-files.changie",
]
CARGO_PROOF_ROWS = [
    "cargo-proof.default",
    "cargo-proof.provider-cargo-allow",
    "cargo-proof.provider-hawk",
    "cargo-proof.provider-ripr",
    "cargo-proof.all-providers",
]
ALL_ROWS = ALLOW_RUST_ROWS + ALLOW_FILES_ROWS + CARGO_PROOF_ROWS


def run_router(stdin_text: str, argv: list[str] | None = None) -> str:
    completed = subprocess.run(
        [sys.executable, str(ROUTER), *(argv or [])],
        input=stdin_text,
        capture_output=True,
        text=True,
        timeout=60,
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"router exited {completed.returncode}, expected always-0:\n{completed.stderr}"
        )
    return completed.stdout.strip()


def check(name: str, stdin_text: str, expected: dict, argv: list[str] | None = None) -> None:
    actual_text = run_router(stdin_text, argv)
    try:
        actual = json.loads(actual_text)
    except ValueError as error:
        raise AssertionError(f"{name}: stdout is not one JSON object: {actual_text!r}") from error
    if actual != expected:
        raise AssertionError(
            f"{name}:\n  expected {json.dumps(expected)}\n  actual   {json.dumps(actual)}"
        )
    print(f"ok: {name}")


def main() -> int:
    check(
        "allow-rust touched",
        "crates/allow-rust/src/lib.rs\n",
        {"mode": "rows", "rows": ALLOW_RUST_ROWS},
    )
    check(
        "allow-files touched",
        "crates/allow-files/src/lib.rs\n",
        {"mode": "rows", "rows": ALLOW_FILES_ROWS},
    )
    check(
        "cargo-proof touched",
        "crates/cargo-proof/src/lib.rs\n",
        {"mode": "rows", "rows": CARGO_PROOF_ROWS},
    )
    check(
        "two product crates union rows",
        "crates/allow-rust/src/lib.rs\ncrates/cargo-proof/src/main.rs\n",
        {"mode": "rows", "rows": ALLOW_RUST_ROWS + CARGO_PROOF_ROWS},
    )
    check(
        "allow-report src touched -> ALL",
        "crates/allow-report/src/lib.rs\n",
        {"mode": "all", "rows": ALL_ROWS},
    )
    check(
        "Cargo.lock touched -> ALL",
        "Cargo.lock\n",
        {"mode": "all", "rows": ALL_ROWS},
    )
    check(
        "product Cargo.toml touched -> ALL",
        "crates/allow-files/Cargo.toml\n",
        {"mode": "all", "rows": ALL_ROWS},
    )
    check(
        "foreign crate touched -> ALL",
        "crates/allow-core/src/lib.rs\n",
        {"mode": "all", "rows": ALL_ROWS},
    )
    check(
        "workflow touched -> ALL",
        ".github/workflows/ci.yml\n",
        {"mode": "all", "rows": ALL_ROWS},
    )
    check(
        "unrecognized path -> ALL",
        "foo/whatever.txt\n",
        {"mode": "all", "rows": ALL_ROWS},
    )
    check(
        "docs-only -> empty rows",
        "docs/ci.md\nREADME.md\n",
        {"mode": "rows", "rows": []},
    )
    check(
        "changes-only -> empty rows",
        ".changes/Added-20260830-feature-configuration-routing.yaml\n",
        {"mode": "rows", "rows": []},
    )
    check(
        "empty stdin -> empty rows",
        "",
        {"mode": "rows", "rows": []},
    )
    check(
        "blank lines ignored, argv paths accepted",
        "\n   \n",
        {"mode": "rows", "rows": ALLOW_FILES_ROWS},
        argv=["crates/allow-files/src/lib.rs"],
    )
    check(
        "mixed docs and product narrows to product rows",
        "docs/ci.md\ncrates/allow-rust/src/lib.rs\n",
        {"mode": "rows", "rows": ALLOW_RUST_ROWS},
    )
    print(f"all routing self-tests passed ({len(ALL_ROWS)} matrix rows characterized)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
