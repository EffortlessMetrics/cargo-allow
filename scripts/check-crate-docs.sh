#!/usr/bin/env bash
# Checked rustdoc proof for the published candidate crates (#3773).
#
# For every crate in the selected release closure, builds the crate docs
# with `rustdoc` warnings denied, proving selected rustdoc links are
# warning-clean under the documented feature sets. Offline-capable via
# --locked and the existing dependency cache. Emits a versioned receipt.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

RECEIPT="${CRATE_DOCS_RECEIPT:-target/cargo-allow/crate-docs.receipt.json}"
FAILURES=0

command -v python3 >/dev/null 2>&1 || { echo "check-crate-docs: python3 required" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "check-crate-docs: cargo required" >&2; exit 1; }

mapfile -t CRATES < <(python3 - "${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml" <<'PY'
import sys
import tomllib

with open(sys.argv[1], "rb") as source:
    crates = tomllib.load(source)["crates"]
# Binary stdout write: Windows text mode would translate \n to \r\n and the
# trailing \r would poison every crate name.
sys.stdout.buffer.write(("\n".join(crates) + "\n").encode("utf-8"))
PY
)

COMMIT="$(git rev-parse HEAD^{commit})"
mkdir -p "$(dirname "${RECEIPT}")"

RESULTS_JSON="["
FIRST=1
for crate in "${CRATES[@]}"; do
  echo "check-crate-docs: cargo doc --no-deps --locked -p ${crate} (RUSTDOCFLAGS=-D warnings)"
  if RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked -p "${crate}" >/dev/null 2>&1; then
    result="warning_clean"
  else
    result="doc_warnings"
    FAILURES=$((FAILURES + 1))
  fi
  if [[ ${FIRST} -eq 1 ]]; then FIRST=0; else RESULTS_JSON+=","; fi
  RESULTS_JSON+="$(python3 -c 'import json,sys; print(json.dumps({"name": sys.argv[1], "result": sys.argv[2]}))' "${crate}" "${result}")"
done
RESULTS_JSON+="]"

RECEIPT_JSON="$(python3 - "${RECEIPT}" "${COMMIT}" "${FAILURES}" "${RESULTS_JSON}" <<'PY'
import json
import sys

path, commit, failures, rows = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
receipt = {
    "schema": "cargo-allow.crate-docs.v1",
    "command": "RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked -p <crate>",
    "commit": commit,
    "rows": json.loads(rows),
    "doc_warnings_count": failures,
    "result": "warning_clean" if failures == 0 else "doc_warnings",
    "claim_boundary": (
        "Rustdoc warnings denied under the documented (default) feature set "
        "for every selected crate; registry state, unpacked-package "
        "execution, and docs.rs rendering are not proven here."
    ),
}
with open(path, "w", encoding="utf-8") as out:
    out.write(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
PY
)"

if [[ ${FAILURES} -ne 0 ]]; then
  echo "check-crate-docs: ${FAILURES} crate(s) with doc warnings" >&2
  exit 1
fi
echo "check-crate-docs: all selected crates documentation-clean; receipt ${RECEIPT}"
