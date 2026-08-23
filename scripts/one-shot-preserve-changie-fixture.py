#!/usr/bin/env python3
"""Move the release-fragment test input to a stable package fixture before batching."""

from pathlib import Path

root = Path(__file__).resolve().parent.parent
source = root / ".changes/Added-20260811-intent-subject-resolution.yaml"
fixture = root / "crates/allow-files/tests/fixtures/changie/valid-real-fragment.yaml"
test = root / "crates/allow-files/src/changie_lint/fragment_rules_tests.rs"

if not source.is_file():
    raise SystemExit(f"missing source fragment: {source}")
fixture.parent.mkdir(parents=True, exist_ok=True)
fixture.write_bytes(source.read_bytes())

text = test.read_text(encoding="utf-8")
old = 'include_str!("../../../../.changes/Added-20260811-intent-subject-resolution.yaml")'
new = 'include_str!("../../tests/fixtures/changie/valid-real-fragment.yaml")'
if text.count(old) != 1:
    raise SystemExit("real-fragment include seam changed")
test.write_text(text.replace(old, new, 1), encoding="utf-8")
