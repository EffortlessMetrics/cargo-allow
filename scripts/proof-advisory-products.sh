#!/usr/bin/env bash
# Advisory direct-floor proof for the #3903 PR C product rows: the
# shared, cargo-intent, and cargo-proof product sets, proved check-only.
# Non-clean typed rows remain report-only and never gate cargo-allow;
# producer failures that emit no current receipt still fail this collector.

set -euo pipefail

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
  selection_out="${CI_PROOF_OUT%.json}.selection.md"

  # A failed producer must not inherit an earlier retained receipt. Remove both
  # outputs before this invocation; a non-clean proof is accepted only when the
  # current run emits its typed receipt and matching selection companion.
  if ! rm -f -- "$CI_PROOF_OUT" "$selection_out"; then
    echo "advisory product $product could not clear prior outputs" >&2
    overall=1
    statuses+=("$product:output_cleanup_failed")
    continue
  fi

  status=0
  bash "$ROOT/scripts/proof-direct-floors.sh" || status=$?
  if [[ ! -s "$CI_PROOF_OUT" ]]; then
    echo "advisory product $product emitted no current typed receipt" >&2
    overall=1
  fi
  if [[ ! -s "$selection_out" ]]; then
    echo "advisory product $product emitted no current selection companion" >&2
    overall=1
  fi
  statuses+=("$product:$status")
done

printf 'advisory product rows collected: %s\n' "${statuses[*]}"
exit "$overall"
