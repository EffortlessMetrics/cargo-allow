#!/usr/bin/env bash
# Advisory direct-floor proof for the #3903 PR C product rows: the
# shared, cargo-intent, and cargo-proof product sets, proved check-only.
# Non-clean typed rows remain report-only and never gate cargo-allow;
# producer failures that emit no receipt still fail this collector.

set -uo pipefail

export CI_PROOF_MSRV="${CI_PROOF_MSRV:-1.95}"
export CI_PROOF_ADVISORY=true
export CI_PROOF_CLASSES="check"
ROOT="$(git rev-parse --show-toplevel)"
RECEIPTS="$ROOT/docs/ci/receipts"
overall=0
statuses=()

for product in shared cargo-intent cargo-proof; do
  export CI_PROOF_PRODUCT="$product"
  export CI_PROOF_OUT="$RECEIPTS/direct-floor-proof-$product-v1.json"
  status=0
  bash "$ROOT/scripts/proof-direct-floors.sh" || status=$?
  if [[ ! -s "$CI_PROOF_OUT" ]]; then
    echo "advisory product $product emitted no typed receipt" >&2
    overall=1
  fi
  statuses+=("$product:$status")
done

printf 'advisory product rows collected: %s\n' "${statuses[*]}"
exit "$overall"
