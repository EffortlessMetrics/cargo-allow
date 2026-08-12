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
import os
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
source_identity_value = (
    f"commit:{source_identity['commit']}/tree:{source_identity['tree']}"
)
registry = tomllib.loads(
    (root / "policy/extraction-parity.toml").read_text(encoding="utf-8")
)
ledger = tomllib.loads(
    (root / "policy/product-move-ledger.toml").read_text(encoding="utf-8")
)
architecture = tomllib.loads(
    (root / "policy/product-crates-v2.toml").read_text(encoding="utf-8")
)
architecture_paths = {
    item["cargo_package_name"]: item["workspace_path"]
    for item in architecture.get("crate_identity", [])
}
cases = registry.get("case", [])
entries = {entry["id"]: entry for entry in ledger.get("entry", [])}
blockers = []
stages = []


def expected_ownership(stage: str, selected_entries: list[dict]) -> dict[str, list[str]]:
    package_names = sorted(
        {
            package
            for entry in selected_entries
            for package in (entry.get("current_crate"), entry.get("target_crate"))
            if package
        }
    )
    package_paths = [
        f"{architecture_paths[package]}/Cargo.toml"
        for package in package_names
        if package in architecture_paths
    ]
    missing_package_names = sorted(
        package for package in package_names if package not in architecture_paths
    )
    if stage == "RepoSnapshot":
        asset_paths = [
            "tests/fixtures/repo-snapshot/parity-committed-head-v1.toml",
            "tests/fixtures/repo-snapshot/parity-staged-index-v1.toml",
            "tests/fixtures/repo-snapshot/parity-staged-deletion-dirty-replacement-v1.toml",
            "tests/fixtures/repo-snapshot/parity-source-view-staged-v1.toml",
        ]
        stage_doc = "docs/architecture/repo-snapshot.md"
    else:
        asset_paths = [
            "tests/fixtures/repo-edit/parity-mutation-lock-alias-v1.toml",
            "tests/fixtures/repo-edit/parity-path-containment-v1.toml",
            "tests/fixtures/repo-edit/parity-atomic-write-v1.toml",
            "tests/fixtures/repo-edit/parity-apply-receipt-v1.toml",
            "tests/fixtures/repo-edit/parity-init-command-v1.toml",
            "tests/fixtures/repo-edit/parity-refresh-command-v1.toml",
            "tests/fixtures/repo-edit/parity-prune-command-v1.toml",
            "tests/fixtures/repo-edit/parity-apply-backup-mode-v1.toml",
            "tests/fixtures/repo-edit/parity-add-command-v1.toml",
            "tests/fixtures/repo-edit/parity-migrate-command-v1.toml",
            "tests/fixtures/repo-edit/parity-propose-command-v1.toml",
        ]
        stage_doc = "docs/architecture/repo-edit.md"
    return {
        "package_paths": sorted(package_paths),
        "asset_paths": sorted(asset_paths),
        "docs_paths": sorted(
            [
                "docs/architecture/extraction-parity.md",
                stage_doc,
                "policy/extraction-parity.toml",
                "policy/product-move-ledger.toml",
            ]
        ),
        "ci_paths": [
            ".github/workflows/ci.yml",
            "scripts/extraction-cutover-status.sh",
        ],
        "package_names": package_names,
        "missing_package_names": missing_package_names,
    }


def missing_paths(paths: list[str]) -> list[str]:
    return [path for path in paths if not (root / path).is_file()]

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
    ownership = expected_ownership(stage, selected_entries)
    missing_ownership = sorted(
        {
            path
            for field in ("package_paths", "asset_paths", "docs_paths", "ci_paths")
            for path in missing_paths(ownership[field])
        }
    )
    if ownership["missing_package_names"]:
        blockers.append(
            f"missing_package_identity:{stage}:{','.join(ownership['missing_package_names'])}"
        )
    stage_dir = output_dir / stage.lower().replace("repo", "repo-")
    stage_dir.mkdir(parents=True, exist_ok=True)
    ownership_receipt_path = stage_dir / "ownership.json"
    ownership_result = (
        "Passed"
        if not missing_ownership and not ownership["missing_package_names"]
        else "Blocked"
    )
    ownership_receipt = {
        "schema_id": "cargo-allow.extraction-cutover-ownership.v1",
        "schema_version": 1,
        "stage": stage,
        "source_identity": source_identity_value,
        "parity_result_digest": (payload or {}).get("parity_result_digest", "unavailable"),
        "result": ownership_result,
        "package_paths": ownership["package_paths"],
        "asset_paths": ownership["asset_paths"],
        "docs_paths": ownership["docs_paths"],
        "ci_paths": ownership["ci_paths"],
        "claim_boundary": "Current topology-derived package/assets/docs/CI ownership paths",
    }
    ownership_receipt_path.write_text(
        json.dumps(ownership_receipt, indent=2) + "\n", encoding="utf-8"
    )
    if missing_ownership:
        blockers.append(f"ownership_not_complete:{stage}")
    env_suffix = stage.replace("Repo", "Repo_").upper().strip("_")
    build_env = os.environ.get(f"EXTRACTION_BUILD_PACKAGE_RECEIPT_{env_suffix}")
    build_receipt_path = Path(build_env) if build_env else stage_dir / "build-package.json"
    if not build_receipt_path.is_absolute():
        build_receipt_path = root / build_receipt_path
    build_receipt_relative = None
    if build_receipt_path.is_file():
        try:
            build_receipt_relative = str(build_receipt_path.relative_to(root)).replace("\\", "/")
        except ValueError:
            blockers.append(f"build_package_receipt_outside_repo:{stage}")
    else:
        blockers.append(f"independent_build_package_receipt_missing:{stage}")
    manifest_path = stage_dir / "cutover-evidence.json"
    if build_receipt_relative is not None:
        manifest_path.write_text(
            json.dumps(
                {
                    "schema_id": "cargo-allow.extraction-cutover-evidence.v2",
                    "schema_version": 2,
                    "ownership_receipt": str(ownership_receipt_path.relative_to(root)).replace(
                        "\\", "/"
                    ),
                    "independent_build_package_receipt": build_receipt_relative,
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
    else:
        manifest_path.unlink(missing_ok=True)
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
            "ownership_receipt": str(ownership_receipt_path.relative_to(root)).replace(
                "\\", "/"
            ),
            "ownership_result": ownership_result,
            "missing_ownership_paths": missing_ownership,
            "missing_package_identities": ownership["missing_package_names"],
            "independent_build_package_receipt": build_receipt_relative,
            "cutover_evidence_manifest": (
                str(manifest_path.relative_to(root)).replace("\\", "/")
                if manifest_path.is_file()
                else None
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
        "source-derived-package-assets-docs-ci-ownership",
        "receipt-inputs-bound-to-source-and-parity-identity",
        "no-policy-promotion-publication-tagging-or-release-execution",
    ],
    "limitations": [
        "independent_build_package_receipt_is-required-and-validated-by-the-cli",
        "old_paths_and_contract_only_policy_remain_until_follow_up_lanes_land",
    ],
}
(output_dir / "extraction-cutover-status.json").write_text(
    json.dumps(status, indent=2) + "\n", encoding="utf-8"
)
print(json.dumps(status, indent=2))
PY

# The adapter is the final gate. A stage-specific cutover receipt is written
# only when exact source identity, runtime parity, topology-derived ownership,
# independent artifact digests, and policy prerequisites all pass. The current
# contract_only/old-path state therefore emits only status and evidence inputs.
for stage in repo-snapshot repo-edit; do
  manifest="${output_dir}/${stage}/cutover-evidence.json"
  receipt="${output_dir}/${stage}/cutover-receipt.json"
  if [[ -f "${manifest}" ]]; then
    cargo run -p cargo-allow --locked -- extraction-parity \
      --stage "${stage}" \
      --cutover-evidence "${manifest}" \
      --output "${receipt}" || rm -f "${receipt}"
  fi
done
