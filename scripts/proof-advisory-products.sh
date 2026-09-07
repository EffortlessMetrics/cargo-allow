#!/usr/bin/env bash
# Advisory direct-floor proof for the #3903 PR C product rows: the
# shared, cargo-intent, and cargo-proof product sets, proved check-only
# (report-only, advisory — test/package enforcement requires the owning
# package-family authority).
#
# Read-only over the live tree; mutates only the temporary worktree.

set -euo pipefail

export CI_PROOF_MSRV="${CI_PROOF_MSRV:-1.95}"
export CI_PROOF_PRODUCT="${CI_PROOF_PRODUCT:-shared}"
export CI_PROOF_ADVISORY=true
CI_PROOF_CLASSES="check"
ROOT="$(git rev-parse --show-toplevel)"
RECEIPTS="$ROOT/docs/ci/receipts"

export CI_PROOF_OUT="$RECEIPTS/direct-floor-proof-shared-v1.json"
bash "$ROOT/scripts/proof-direct-floors.sh"

export CI_PROOF_PRODUCT="cargo-intent"
export CI_PROOF_OUT="$RECEIPTS/direct-floor-proof-cargo-intent-v1.json"
bash "$ROOT/scripts/proof-direct-floors.sh"

export CI_PROOF_PRODUCT="cargo-proof"
export CI_PROOF_OUT="$RECEIPTS/direct-floor-proof-cargo-proof-v1.json"
bash "$ROOT/scripts/proof-direct-floors.sh"

echo "advisory product rows complete: shared, cargo-intent, cargo-proof"
