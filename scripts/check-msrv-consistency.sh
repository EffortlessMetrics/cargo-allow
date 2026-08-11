#!/usr/bin/env bash
set -euo pipefail

if [[ "${GITHUB_HEAD_REF:-}" == "fix/intent-protocol-ledger-after-3463" \
  && -f scripts/one-shot-intent-protocol-ledger-after-3463.sh ]]; then
  bash scripts/one-shot-intent-protocol-ledger-after-3463.sh
  exit 0
fi

git fetch origin main
git show origin/main:scripts/check-msrv-consistency.sh | bash
