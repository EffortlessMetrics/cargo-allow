#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-prepare
log="target/release-prepare/prepare.log"

# Preserve complete generation output as a workflow artifact while keeping the
# release candidate free of this staging wrapper.
git rm -f scripts/run-prepare-0.1.11.sh
{
  echo "prepare_release_start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "head=$(git rev-parse HEAD)"
  bash scripts/prepare-0.1.11.sh
  echo "prepare_release_result=pass"
} > >(tee "${log}") 2>&1
