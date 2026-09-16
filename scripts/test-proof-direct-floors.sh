#!/usr/bin/env bash
# Characterization checks for scripts/proof-direct-floors.sh and
# scripts/proof-advisory-products.sh.
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
  'floor_product_workspace.py' \
  "proof constructs the selected product workspace"
contains "$proof_text" \
  'product-workspace-identity.json' \
  "proof retains the workspace projection identity"
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
contains "$collector_text" 'rm -f -- "$CI_PROOF_OUT" "$selection_out"' \
  "collector clears prior retained outputs"
contains "$collector_text" '[[ ! -s "$CI_PROOF_OUT" ]]' \
  "collector requires a current advisory receipt"
contains "$collector_text" '[[ ! -s "$selection_out" ]]' \
  "collector requires a current selection companion"
contains "$collector_text" 'statuses+=("$product:$status")' \
  "collector retains each advisory product result"

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

# A producer that fails before emitting its current outputs must not inherit
# the retained receipt or selection companion from an earlier run. A producer
# that emits both typed artifacts may remain non-zero because advisory rows are
# report-only and carry their non-clean disposition in the receipt itself.
fixture="${work}/advisory-collector"
mkdir -p "${fixture}/scripts" "${fixture}/docs/ci/receipts"
cp "$COLLECTOR" "${fixture}/scripts/proof-advisory-products.sh"
cat >"${fixture}/scripts/proof-direct-floors.sh" <<'STUB'
#!/usr/bin/env bash
set -u
selection_out="${CI_PROOF_OUT%.json}.selection.md"
mode="${STUB_MODE:-partial}"
if [[ "$mode" == "typed" || "$CI_PROOF_PRODUCT" == "cargo-proof" ]]; then
  printf '{"product":"%s","result":"instrument_failure"}\n' "$CI_PROOF_PRODUCT" >"$CI_PROOF_OUT"
  printf '# current %s selection\n' "$CI_PROOF_PRODUCT" >"$selection_out"
  exit 7
fi
if [[ "$CI_PROOF_PRODUCT" == "cargo-intent" ]]; then
  printf '{"product":"cargo-intent","result":"instrument_failure"}\n' >"$CI_PROOF_OUT"
  exit 8
fi
exit 9
STUB
git -C "$fixture" init -q

for product in shared cargo-intent cargo-proof; do
  printf 'stale %s receipt\n' "$product" >
    "${fixture}/docs/ci/receipts/direct-floor-proof-${product}-v1.json"
  printf 'stale %s selection\n' "$product" >
    "${fixture}/docs/ci/receipts/direct-floor-proof-${product}-v1.selection.md"
done

if (
  cd "$fixture"
  bash scripts/proof-advisory-products.sh >"${work}/collector-partial.log" 2>&1
); then
  fail "stale advisory outputs cannot satisfy a failed producer"
fi
[[ ! -e "${fixture}/docs/ci/receipts/direct-floor-proof-shared-v1.json" ]] ||
  fail "failed shared producer cannot retain a stale receipt"
[[ ! -e "${fixture}/docs/ci/receipts/direct-floor-proof-shared-v1.selection.md" ]] ||
  fail "failed shared producer cannot retain a stale selection"
[[ -s "${fixture}/docs/ci/receipts/direct-floor-proof-cargo-intent-v1.json" ]] ||
  fail "cargo-intent current receipt remains visible"
[[ ! -e "${fixture}/docs/ci/receipts/direct-floor-proof-cargo-intent-v1.selection.md" ]] ||
  fail "missing cargo-intent selection cannot inherit a stale companion"
[[ -s "${fixture}/docs/ci/receipts/direct-floor-proof-cargo-proof-v1.json" ]] ||
  fail "typed cargo-proof receipt is retained"
[[ -s "${fixture}/docs/ci/receipts/direct-floor-proof-cargo-proof-v1.selection.md" ]] ||
  fail "typed cargo-proof selection is retained"
grep -q 'shared emitted no current typed receipt' "${work}/collector-partial.log" ||
  fail "missing current receipt is diagnosed"
grep -q 'cargo-intent emitted no current selection companion' "${work}/collector-partial.log" ||
  fail "missing current selection is diagnosed"
grep -q 'shared:9 cargo-intent:8 cargo-proof:7' "${work}/collector-partial.log" ||
  fail "all advisory statuses remain visible"
ok "stale advisory outputs cannot satisfy a failed producer"

if ! (
  cd "$fixture"
  STUB_MODE=typed bash scripts/proof-advisory-products.sh >"${work}/collector-typed.log" 2>&1
); then
  cat "${work}/collector-typed.log" >&2
  fail "typed non-clean advisory receipts remain report-only"
fi
for product in shared cargo-intent cargo-proof; do
  [[ -s "${fixture}/docs/ci/receipts/direct-floor-proof-${product}-v1.json" ]] ||
    fail "typed $product receipt is retained"
  [[ -s "${fixture}/docs/ci/receipts/direct-floor-proof-${product}-v1.selection.md" ]] ||
    fail "typed $product selection is retained"
done
grep -q 'shared:7 cargo-intent:7 cargo-proof:7' "${work}/collector-typed.log" ||
  fail "typed non-clean statuses remain visible"
ok "typed non-clean advisory receipts remain report-only"

bash scripts/test-proof-direct-floors-protocol.sh
