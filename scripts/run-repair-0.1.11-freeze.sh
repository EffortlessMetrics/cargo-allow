#!/usr/bin/env bash
# One-shot repair for final PR #2376 review findings.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-repair
log="target/release-repair/repair.log"

{
  set -x
  # Restore the complete qualified policy after a connector-side partial-file
  # edit, then apply the exact review correction to the readiness receipt.
  git checkout f5ca942150045016cb1b50f3c4a7e9a5b312ee75 -- policy/allow.toml

  python3 - <<'PY'
from pathlib import Path

policy_path = Path("policy/allow.toml")
policy = policy_path.read_text(encoding="utf-8")
old = 'reason = "Records the current ReleaseQualificationV1 capability, platform, MSRV, safety, package, authentication, and provenance dispositions without claiming candidate or tag readiness."'
new = 'reason = "Records the selected 0.1.11 ReleaseQualificationV1 capability, platform, MSRV, package, authentication, provenance, candidate-readiness, and tag-readiness decisions."'
if policy.count(old) != 1:
    raise SystemExit("readiness allow-entry reason changed unexpectedly")
policy_path.write_text(policy.replace(old, new, 1), encoding="utf-8", newline="\n")

record_path = Path("docs/release/0.1.11.md")
record = record_path.read_text(encoding="utf-8")
old = "#2371 owns 0.2.0: Rust 1.95, #2336 matching parity, safe active-ledger `add`,"
new = "Issue #2371 owns 0.2.0: Rust 1.95, #2336 matching parity, safe active-ledger `add`,"
if old in record:
    record = record.replace(old, new, 1)
elif new not in record:
    raise SystemExit("0.1.11 next-train line changed unexpectedly")
record_path.write_text(record, encoding="utf-8", newline="\n")
PY

  git rm -f scripts/run-repair-0.1.11-freeze.sh

  cargo fmt --all --check
  cargo test -p cargo-allow --bins --locked
  cargo test -p cargo-allow --test published_quick_start --locked
  cargo run -p cargo-allow -- check --mode no-new \
    --format markdown \
    --receipt target/cargo-allow/check.receipt.json \
    --output target/cargo-allow/check.md
  bash scripts/release-version-preflight.sh 0.1.11

  git config user.name "github-actions[bot]"
  git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
  git add -A
  git commit -m "docs(release): reconcile final 0.1.11 review findings"
  git push origin HEAD:release/0.1.11
} > >(tee "${log}") 2>&1
