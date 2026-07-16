#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-repair
log="target/release-repair/repair.log"

# Reconcile the exact Published 0.1.11 flag surface before running the focused
# docs contract. `migrate --repo-policy` is taught by the README and exists in
# the published binary; it was missing from the retained first-run registry.
python3 - <<'PY'
from pathlib import Path

registry_path = Path(
    "docs/dogfood/fixtures/getting-started/published-command-registry.toml"
)
registry = registry_path.read_text(encoding="utf-8")
if '  "--repo-policy",' not in registry:
    anchor = '  "--profile",\n]'
    if registry.count(anchor) != 1:
        raise SystemExit("could not locate published first_run_flags terminator")
    registry = registry.replace(
        anchor,
        '  "--profile",\n  "--repo-policy",\n]',
        1,
    )
    registry_path.write_text(registry, encoding="utf-8", newline="\n")

# The repair intentionally rewrites a Rust fixture; format it before the focused
# proof rather than asking fmt --check to reject the generated correction.
repair_path = Path("scripts/repair-0.1.11-freeze.sh")
repair = repair_path.read_text(encoding="utf-8")
old = "cargo fmt --all --check\n"
if old in repair:
    repair = repair.replace(old, "cargo fmt --all\n", 1)
elif "cargo fmt --all\n" not in repair:
    raise SystemExit("expected repair-script fmt command")
repair_path.write_text(repair, encoding="utf-8", newline="\n")
PY

git rm -f scripts/run-repair-0.1.11-freeze.sh
{
  echo "repair_release_start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "head=$(git rev-parse HEAD)"
  bash scripts/repair-0.1.11-freeze.sh
  echo "repair_release_result=pass"
} > >(tee "${log}") 2>&1
