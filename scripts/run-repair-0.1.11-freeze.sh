#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-repair
log="target/release-repair/repair.log"

git rm -f scripts/run-repair-0.1.11-freeze.sh
{
  echo "repair_release_start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "head=$(git rev-parse HEAD)"
  bash scripts/repair-0.1.11-freeze.sh
  echo "repair_release_result=pass"
} > >(tee "${log}") 2>&1
