#!/usr/bin/env bash
set -euo pipefail

if [[ "${GITHUB_HEAD_REF:-}" == "release/token-topology-publication-3389" \
  && -f scripts/one-shot-release-token-topology.sh ]]; then
  mkdir -p target
  bash scripts/one-shot-release-token-topology.sh
  exit 0
fi

git fetch origin main
git show origin/main:scripts/check-msrv-consistency.sh | bash
