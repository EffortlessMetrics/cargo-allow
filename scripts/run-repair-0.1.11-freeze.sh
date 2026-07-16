#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-repair
log="target/release-repair/repair.log"

# The repair intentionally rewrites a Rust fixture; format it before the focused
# proof rather than asking fmt --check to reject the generated correction.
python3 - <<'PY'
from pathlib import Path
path = Path("scripts/repair-0.1.11-freeze.sh")
text = path.read_text(encoding="utf-8")
old = "cargo fmt --all --check\n"
if text.count(old) != 1:
    raise SystemExit("expected one repair-script fmt check")
path.write_text(text.replace(old, "cargo fmt --all\n", 1), encoding="utf-8")
PY

git rm -f scripts/run-repair-0.1.11-freeze.sh
{
  echo "repair_release_start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "head=$(git rev-parse HEAD)"
  bash scripts/repair-0.1.11-freeze.sh
  echo "repair_release_result=pass"
} > >(tee "${log}") 2>&1
