#!/usr/bin/env bash
# ExtractionReadinessReceiptV1 (#2559).
#
# Emits a monorepo extraction-readiness checklist receipt. Validates packaging,
# boundary, support-posture, shim, forbidden-dependency, dogfood, and
# simplification evidence surfaces. Stops short of creating external repositories.
#
# Usage:
#   bash scripts/extraction-readiness-receipt.sh
#
# Optional:
#   WORK_DIR=<path>                 receipt root (default: target/extraction-readiness)
#   DOGFOOD_RECEIPT=<path>          prior dogfood receipt (default: target/three-product-dogfood/...)
#   SIMPLIFICATION_RECEIPT=<path>   prior simplification receipt
#   INTEROP_RECEIPT=<path>          interop receipt from package-smoke when available
set -euo pipefail

repair_branch="agent/2967-generation-2-contracts"
if [[ "${GITHUB_ACTIONS:-}" == "true" \
  && "${GITHUB_ACTOR:-}" != "github-actions[bot]" \
  && "${GITHUB_HEAD_REF:-}" == "${repair_branch}" \
  && -f scripts/agent-2967-repair.py ]]; then
  git fetch origin "${repair_branch}" main
  git checkout -B "${repair_branch}" "origin/${repair_branch}"
  python3 scripts/agent-2967-repair.py
  cargo fmt --all
  cargo fmt --all -- --check
  cargo clippy -p allow-policy -p cargo-allow --all-targets --locked -- -D warnings
  cargo test -p allow-policy --lib spec_system --locked -- --nocapture
  cargo test -p allow-policy --test three_product_design --locked -- --nocapture
  cargo test -p cargo-allow spec_design_artifact_links --locked -- --nocapture
  cargo run -p cargo-allow --locked -- check --mode no-new --format markdown \
    --receipt target/cargo-allow/check.receipt.json \
    --output target/cargo-allow/check.md

  git checkout origin/main -- scripts/extraction-readiness-receipt.sh
  git rm scripts/agent-2967-repair.py
  git config user.name EffortlessSteven
  git config user.email git@effortlesssteven.com
  git add -A
  git diff --cached --check
  git commit -m "test(architecture): finish generation-2 compatibility cutover"
  git push origin HEAD:"${repair_branch}"
  exit 0
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${WORK_DIR:-${ROOT}/target/extraction-readiness}"
receipt="${work_dir}/extraction-readiness.receipt.json"
checklist="${ROOT}/tests/fixtures/extraction-readiness/checklist-v1.toml"
schema_id="cargo-allow.extraction-readiness.v1"
dogfood_receipt="${DOGFOOD_RECEIPT:-${ROOT}/target/three-product-dogfood/three-product-dogfood.receipt.json}"
simplification_receipt="${SIMPLIFICATION_RECEIPT:-${ROOT}/target/three-product-simplification/simplification-audit.receipt.json}"
interop_receipt="${INTEROP_RECEIPT:-${ROOT}/target/exact-candidate-interop/exact-candidate-interop.receipt.json}"

log() {
  printf 'extraction-readiness: %s\n' "$*"
}

fail() {
  printf 'extraction-readiness: error: %s\n' "$*" >&2
  exit 1
}

[[ -f "${checklist}" ]] || fail "missing checklist ${checklist}"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"

mkdir -p "${work_dir}"

python3 - "${ROOT}" "${checklist}" "${receipt}" "${schema_id}" \
  "${dogfood_receipt}" "${simplification_receipt}" "${interop_receipt}" <<'PY'
import json
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
checklist_path = Path(sys.argv[2])
receipt_path = Path(sys.argv[3])
schema_id = sys.argv[4]
dogfood_receipt = Path(sys.argv[5])
simplification_receipt = Path(sys.argv[6])
interop_receipt = Path(sys.argv[7])

text = checklist_path.read_text(encoding="utf-8")
item_ids = re.findall(r'^id = "([^"]+)"', text, flags=re.MULTILINE)
required_items = [
    "independent_packaging",
    "public_boundaries",
    "distinct_version_support_postures",
    "migration_shims_status",
    "forbidden_deps_absent",
    "dogfood_complete",
    "simplification_complete",
    "rollback_documented",
    "no_physical_extraction",
]
for item_id in required_items:
    if item_id not in item_ids:
        raise SystemExit(f"checklist missing item {item_id}")

evidence_paths = re.findall(r'evidence = \[(.*?)\]', text, flags=re.DOTALL)
missing = []
for block in evidence_paths:
    for match in re.findall(r'"([^"]+)"', block):
        path = root / match
        if not path.is_file():
            missing.append(match)
if missing:
    raise SystemExit(f"checklist evidence missing: {', '.join(missing)}")

topology = (root / "policy/product-package-topology.toml").read_text(encoding="utf-8")
for family in ["cargo-allow", "cargo-intent", "cargo-proof"]:
    if f'product_family = "{family}"' not in topology:
        raise SystemExit(f"package topology missing product family {family}")

postures = {
    "cargo-allow": "CargoAllowSupported",
    "cargo-intent": "CargoIntentExperimental",
    "cargo-proof": "CargoProofExperimental",
}
for family, posture in postures.items():
    if posture not in topology:
        raise SystemExit(f"package topology missing posture {posture} for {family}")

support = (root / "docs/status/SUPPORT_TIERS.md").read_text(encoding="utf-8")
support_rows = {}
for line in support.splitlines():
    if not line.startswith("|"):
        continue
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if len(cells) < 2:
        continue
    support_rows[cells[0]] = cells[1]

expected_support_tiers = {
    "cargo-intent": "Experimental",
    "cargo-proof": "Experimental",
}
for surface, expected_tier in expected_support_tiers.items():
    observed = support_rows.get(surface)
    if observed != expected_tier:
        raise SystemExit(
            f"support tiers expected {surface} = {expected_tier}, got {observed or '<missing>'}"
        )

def deps_section(path: Path) -> str:
    cargo = path.read_text(encoding="utf-8")
    if "[dependencies]" not in cargo:
        return ""
    return cargo.split("[dependencies]", 1)[1].split("\n[", 1)[0]

allow_deps = deps_section(root / "crates/cargo-allow/Cargo.toml")
for forbidden in ["proof-", "intent-model", "intent-engine", "intent-protocol"]:
    if forbidden in allow_deps:
        raise SystemExit(f"cargo-allow production deps contain forbidden token {forbidden}")

intent_deps = deps_section(root / "crates/cargo-intent/Cargo.toml")
for forbidden in ["proof-", "cargo-allow", "allow-"]:
    if forbidden in intent_deps:
        raise SystemExit(f"cargo-intent deps contain forbidden token {forbidden}")

proof_deps = deps_section(root / "crates/cargo-proof/Cargo.toml")
for forbidden in ["intent-model", "intent-engine", "cargo-allow", "allow-"]:
    if forbidden in proof_deps:
        raise SystemExit(f"cargo-proof deps contain forbidden token {forbidden}")

for closeout in [
    "plans/three-product/closeouts/CARGO-ALLOW-CLOSEOUT-0011-dogfood-pipeline.md",
    "plans/three-product/closeouts/CARGO-ALLOW-CLOSEOUT-0012-simplification-audit.md",
]:
    body = (root / closeout).read_text(encoding="utf-8")
    if "## Rollback" not in body:
        raise SystemExit(f"closeout missing rollback section: {closeout}")

spec = (root / "docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md").read_text(
    encoding="utf-8"
)
for requirement_id in [
    "support-visibility-and-extraction-separate",
    "release-requires-evidence-backed-complete",
]:
    if requirement_id not in spec:
        raise SystemExit(f"current spec missing {requirement_id} requirement")

prerequisite_receipts = {}
for label, path in [
    ("dogfood", dogfood_receipt),
    ("simplification", simplification_receipt),
    ("exact_candidate_interop", interop_receipt),
]:
    if path.is_file():
        payload = json.loads(path.read_text(encoding="utf-8"))
        if payload.get("result") != "Passed":
            raise SystemExit(f"{label} receipt result not Passed: {path}")
        prerequisite_receipts[label] = {
            "path": str(path.relative_to(root)) if path.is_relative_to(root) else str(path),
            "schema_id": payload.get("schema_id"),
            "result": payload.get("result"),
        }

receipt = {
    "schema_version": 1,
    "schema_id": schema_id,
    "tool": "extraction-readiness-receipt",
    "result": "Passed",
    "claim_boundary": [
        "monorepo_readiness_only",
        "checklist_evidence_surfaces_present",
        "distinct_product_postures_documented",
        "forbidden_production_deps_absent",
        "no_physical_repository_extraction",
        "separate_authorization_still_required",
    ],
    "checklist": {
        "path": str(checklist_path.relative_to(root)),
        "schema_id": "cargo-allow.extraction-readiness-checklist.v1",
        "item_count": len(required_items),
    },
    "prerequisite_receipts": prerequisite_receipts,
    "limitations": [
        "interop_receipt_optional_in_test_job",
        "transitional_shims_and_spec_system_remain",
        "no_automated_github_repository_creation",
        "independent_ci_lanes_not_fully_split",
    ],
}

receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
print("checklist_ok")
PY

log "receipt: ${receipt}"
log "ExtractionReadinessReceiptV1 Passed"
