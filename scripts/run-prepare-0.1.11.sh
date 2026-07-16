#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-prepare
log="target/release-prepare/prepare.log"

# The public notes file is created during generation and is not yet tracked when
# the second real `add` command selects it. Patch the one-shot script to include
# untracked inventory for exactly that generated finding.
python3 - <<'PY'
from pathlib import Path
path = Path("scripts/prepare-0.1.11.sh")
lines = path.read_text(encoding="utf-8").splitlines()
out = []
in_github_notes_add = False
inserted = False
for line in lines:
    if "--path docs/release/github/v0.1.11.md" in line:
        in_github_notes_add = True
    if in_github_notes_add and "--write policy/allow.toml" in line and not inserted:
        out.append("  --include-untracked \\")
        inserted = True
    out.append(line)
    if in_github_notes_add and line.strip() == "--force":
        in_github_notes_add = False
if not inserted:
    raise SystemExit("could not patch generated GitHub notes add command")
path.write_text("\n".join(out) + "\n", encoding="utf-8")
PY

# Preserve complete generation output as a workflow artifact while keeping the
# release candidate free of this staging wrapper.
git rm -f scripts/run-prepare-0.1.11.sh
{
  echo "prepare_release_start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "head=$(git rev-parse HEAD)"
  bash scripts/prepare-0.1.11.sh
  echo "prepare_release_result=pass"
} > >(tee "${log}") 2>&1
