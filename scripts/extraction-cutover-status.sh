#!/usr/bin/env bash
# Extraction cutover status artifact (#3469).
#
# Runs both current runtime parity stages and emits a truthful, fail-closed
# status artifact. This lane is observational: a blocked status is expected
# until parity, old-path, ownership, and package/build prerequisites are proven.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

output_dir="${EXTRACTION_CUTOVER_DIR:-${ROOT}/target/extraction-cutover}"
mkdir -p "${output_dir}"

snapshot_exit=0
cargo run -p cargo-allow --locked -- extraction-parity \
  --stage repo-snapshot \
  --output "${output_dir}/repo-snapshot-parity.json" || snapshot_exit=$?

edit_exit=0
cargo run -p cargo-allow --locked -- extraction-parity \
  --stage repo-edit \
  --output "${output_dir}/repo-edit-parity.json" || edit_exit=$?

python3 - "${ROOT}" "${output_dir}" "${snapshot_exit}" "${edit_exit}" <<'PY'
import json
import subprocess
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
output_dir = Path(sys.argv[2])
exit_codes = {"RepoSnapshot": int(sys.argv[3]), "RepoEdit": int(sys.argv[4])}
stage_paths = {
    "RepoSnapshot": output_dir / "repo-snapshot-parity.json",
    "RepoEdit": output_dir / "repo-edit-parity.json",
}


def git_value(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


source_identity = {
    "commit": git_value("rev-parse", "HEAD"),
    "tree": git_value("rev-parse", "HEAD^{tree}"),
}
registry = tomllib.loads(
    (root / "policy/extraction-parity.toml").read_text(encoding="utf-8")
)
ledger = tomllib.loads(
    (root / "policy/product-move-ledger.toml").read_text(encoding="utf-8")
)
cases = registry.get("case", [])
entries = {entry["id"]: entry for entry in ledger.get("entry", [])}
blockers = [
    "package_and_build_evidence_not_supplied",
    "cutover_receipt_not_requested_until_prerequisites_are_proven",
]
stages = []

for stage, path in stage_paths.items():
    payload = None
    if path.is_file():
        payload = json.loads(path.read_text(encoding="utf-8"))
    stage_cases = [case for case in cases if case.get("stage") == stage]
    stage_ids = {case.get("id") for case in stage_cases}
    if exit_codes[stage] != 0 or not payload:
        blockers.append(f"runtime_parity_not_complete:{stage}")
    else:
        if (
            payload.get("result") != "Passed"
            or payload.get("expected_case_count")
            != payload.get("emitted_case_count")
        ):
            blockers.append(f"runtime_parity_not_complete:{stage}")
    contract_only = [
        case.get("id") for case in stage_cases if case.get("disposition") != "Proven"
    ]
    if contract_only:
        blockers.append(f"policy_disposition_not_proven:{stage}")
    selected_entries = [
        entries[case["move_ledger_entry"]]
        for case in stage_cases
        if case.get("move_ledger_entry") in entries
    ]
    if len(selected_entries) != len(stage_cases):
        blockers.append(f"missing_ledger_entry:{stage}")
    old_path_not_closed = [
        entry["id"]
        for entry in selected_entries
        if entry.get("old_path_reachability_disposition")
        not in {
            "Deleted",
            "CompileUnreachable",
            "FeatureUnreachableInSupportedCandidate",
        }
    ]
    if old_path_not_closed:
        blockers.append(f"old_path_still_reachable:{stage}")
    stages.append(
        {
            "stage": stage,
            "command_exit_code": exit_codes[stage],
            "parity_artifact": (
                str(path.relative_to(root)) if path.is_file() else None
            ),
            "registered_case_count": len(stage_cases),
            "registered_case_ids": sorted(stage_ids),
            "runtime_result": payload.get("result") if payload else None,
            "policy_disposition": "Proven" if not contract_only else "contract_only",
            "old_path_reachability": (
                "closed" if not old_path_not_closed else "OldPathStillReachable"
            ),
        }
    )

status = {
    "schema_id": "cargo-allow.extraction-cutover-status.v1",
    "schema_version": 1,
    "source_identity": source_identity,
    "result": "Blocked" if blockers else "ReadyForReceipt",
    "stages": stages,
    "blockers": sorted(set(blockers)),
    "claim_boundary": [
        "runtime_parity_execution_and_artifacts",
        "fail_closed_cutover_readiness_status",
        "exact_source_identity",
        "policy_derived_stage_inventory",
        "no_cutover_receipt_or_policy_promotion",
    ],
    "limitations": [
        "package_assets_docs_ci_ownership_not_proven",
        "independent_build_package_evidence_not_proven",
        "old_paths_and_contract_only_policy_remain_until_follow_up_lanes_land",
    ],
}
(output_dir / "extraction-cutover-status.json").write_text(
    json.dumps(status, indent=2) + "\n", encoding="utf-8"
)
print(json.dumps(status, indent=2))
PY
