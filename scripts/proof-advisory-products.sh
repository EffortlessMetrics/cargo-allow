#!/usr/bin/env bash
# Advisory direct-floor proof for the #3903 PR C product rows: the
# shared, cargo-intent, and cargo-proof product sets, proved check-only
# (report-only, advisory — test/package enforcement requires the owning
# package-family authority).
#
# Each product's product identity, receipt path, and class selection are
# pinned here so an inherited CI_PROOF_* environment can never make one
# product's proof write another product's receipt.
#
# Read-only over the live tree; mutates only the temporary worktree.

set -euo pipefail

export CI_PROOF_MSRV="${CI_PROOF_MSRV:-1.95}"
export CI_PROOF_ADVISORY=true
export CI_PROOF_CLASSES="check"
ROOT="$(git rev-parse --show-toplevel)"
RECEIPTS="$ROOT/docs/ci/receipts"

for product in shared cargo-intent cargo-proof; do
  export CI_PROOF_PRODUCT="$product"
  export CI_PROOF_OUT="$RECEIPTS/direct-floor-proof-$product-v1.json"
  bash "$ROOT/scripts/proof-direct-floors.sh"
done

echo "advisory product rows complete: shared, cargo-intent, cargo-proof"
