#!/usr/bin/env bash
# ThreeProductSimplificationAuditV1 (#2208).
#
# Validates the simplification inventory and records removed MvpSimplify items.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

inventory="${ROOT}/policy/three-product-simplification.toml"
receipt="${WORK_DIR:-${ROOT}/target/three-product-simplification}/simplification-audit.receipt.json"
schema_id="cargo-allow.three-product-simplification-audit.v1"

log() {
  printf 'three-product-simplification-audit: %s\n' "$*"
}

fail() {
  printf 'three-product-simplification-audit: error: %s\n' "$*" >&2
  exit 1
}

[[ -f "${inventory}" ]] || fail "missing inventory ${inventory}"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"

if [[ -f "${ROOT}/crates/cargo-allow/src/io.rs" ]]; then
  fail "cargo-allow io shim must be removed (folded into command_support)"
fi

mkdir -p "$(dirname "${receipt}")"

python3 - "${inventory}" "${receipt}" "${schema_id}" <<'PY'
import json
import sys
from pathlib import Path

inventory_path = Path(sys.argv[1])
receipt_path = Path(sys.argv[2])
schema_id = sys.argv[3]
text = inventory_path.read_text(encoding="utf-8")

classifications = [
    "MvpRequired",
    "MvpSimplify",
    "PostMvpValidatedNeed",
    "PostMvpUnproven",
    "DeferUntilSecondAdopter",
    "RejectOrReplace",
]
for label in classifications:
    if label not in text:
        raise SystemExit(f"inventory missing classification {label}")

entries = text.count("[[entry]]")
if entries < 10:
    raise SystemExit(f"expected at least 10 inventory entries, got {entries}")

if 'action = "removed"' not in text:
    raise SystemExit("inventory must record at least one removed simplification")

if "cargo-allow-io-shim" not in text:
    raise SystemExit("inventory missing cargo-allow-io-shim entry")

receipt = {
    "schema_version": 1,
    "schema_id": schema_id,
    "tool": "three-product-simplification-audit",
    "result": "Passed",
    "claim_boundary": [
        "inventory_complete",
        "io_shim_removed",
        "no_crate_deletions_in_this_packet",
    ],
    "inventory": {
        "path": str(inventory_path),
        "entry_count": entries,
        "schema_id": "cargo-allow.three-product-simplification.v1",
    },
    "limitations": [
        "classification_only_no_automated_usage_proof",
        "no_physical_repository_extraction",
    ],
}

receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
print("inventory_ok")
PY

log "receipt: ${receipt}"
log "ThreeProductSimplificationAuditV1 Passed"
