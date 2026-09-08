#!/usr/bin/env bash
# Characterization checks for scripts/proof-direct-floors.sh and
# scripts/proof-advisory-products.sh.
#
# Proves the proof lane's shell contracts stay in place: both scripts
# parse, the product registry keeps its entries and its fail-closed
# arm, the advisory collector pins product identity and the check-only
# class per product, and the floored test class still excludes the
# drift meta-tests by name (recorded verbatim in the receipt commands).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

fail() {
  printf 'fail %s\n' "$1" >&2
  exit 1
}

ok() {
  printf 'ok %s\n' "$1"
}

contains() {
  local haystack="$1" needle="$2" label="$3"
  case "$haystack" in
    *"$needle"*) ok "$label" ;;
    *) fail "$label" ;;
  esac
}

PROOF="scripts/proof-direct-floors.sh"
COLLECTOR="scripts/proof-advisory-products.sh"

bash -n "$PROOF" || fail "proof script parses"
ok "proof script parses"
bash -n "$COLLECTOR" || fail "collector parses"
ok "collector parses"

registry="$(sed -n '/^product_roots() {/,/^}/p' "$PROOF")"
[[ -n "$registry" ]] || fail "registry function present"
contains "$registry" '"cargo-allow"' "registry maps cargo-allow"
contains "$registry" '"effortless-repo-protocol"' "registry maps shared root protocol"
contains "$registry" '"effortless-repo-snapshot"' "registry maps shared root snapshot"
contains "$registry" '"effortless-repo-edit"' "registry maps shared root edit"
contains "$registry" '"effortless-rust-source-index"' "registry maps shared root source-index"
contains "$registry" '"cargo-intent"' "registry maps cargo-intent"
contains "$registry" '"cargo-proof"' "registry maps cargo-proof"
contains "$registry" 'return 1' "unregistered products fail closed"

proof_text="$(cat "$PROOF")"
contains "$proof_text" \
  '--skip minimum_direct_version_drift' \
  "floored test class skips the drift meta-tests"
contains "$proof_text" \
  '--skip check_exits_zero_when_every_release_set_receipt_is_current' \
  "floored test class skips the release-set check meta-test"
contains "$proof_text" \
  '--skip minimum_direct_version_fixtures_retained_proof_receipt_is_law_clean' \
  "floored test class skips the retained-receipt law meta-test"
contains "$proof_text" "tr -d '\r'" "digests are line-ending independent"

collector_text="$(cat "$COLLECTOR")"
contains "$collector_text" 'export CI_PROOF_CLASSES="check"' "collector pins the check-only class"
contains "$collector_text" 'for product in shared cargo-intent cargo-proof; do' \
  "collector iterates exactly the advisory products"
contains "$collector_text" 'export CI_PROOF_PRODUCT="$product"' \
  "collector pins the product identity per product"

# An unregistered product must fail closed before any worktree exists.
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
if CI_PROOF_PRODUCT="not-a-product" CI_PROOF_OUT="${work}/receipt.json" \
  bash "$PROOF" >"${work}/out.log" 2>&1; then
  fail "unregistered product fails closed"
fi
grep -q "unknown product 'not-a-product'" "${work}/out.log" ||
  fail "unregistered product error names the product"
ok "unregistered product fails closed"
