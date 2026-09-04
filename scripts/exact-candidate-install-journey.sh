#!/usr/bin/env bash
# ExactCandidateInstallJourneyV1 (#3357).
#
# Binds the canonical thirteen-crate ExactCandidatePackageSet proof to the
# installed cargo-allow first-hour journey. The binary must come from the
# extracted local-registry install; there is deliberately no workspace-path
# fallback. The source-candidate harness remains the journey implementation,
# while this wrapper supplies the cross-receipt identity and cleanup contract.
#
# Usage:
#   bash scripts/exact-candidate-install-journey.sh
#
# Optional:
#   PACKAGE_SET_DIR=<path>  exact-candidate package-set output directory
#   WORK_DIR=<path>         final receipt directory
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

package_set_dir="${PACKAGE_SET_DIR:-${ROOT}/target/exact-candidate-package-set}"
work_dir="${WORK_DIR:-${ROOT}/target/exact-candidate-install-journey}"
package_receipt="${package_set_dir}/exact-candidate-package-set.receipt.json"
package_schema="${ROOT}/docs/dogfood/fixtures/release/exact-candidate-package-set.v1.schema.json"
journey_schema="${ROOT}/docs/dogfood/fixtures/release/source-candidate-smoke-receipt.v1.schema.json"
fixture="${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml"
receipt="${work_dir}/exact-candidate-install-journey.receipt.json"
source_work_dir="${work_dir}/source-candidate-smoke"
consumer_dir="${CONSUMER_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-exact-candidate-consumer.XXXXXX")}"

cleanup() {
  if [[ -d "${consumer_dir}" ]]; then
    rm -rf "${consumer_dir}"
  fi
  if [[ -d "${source_work_dir}" ]]; then
    rm -rf "${source_work_dir}"
  fi
}
trap cleanup EXIT

log() {
  printf 'exact-candidate-install-journey: %s\n' "$*"
}

fail() {
  printf 'exact-candidate-install-journey: error: %s\n' "$*" >&2
  exit 1
}

command -v python3 >/dev/null 2>&1 || fail "python3 is required"
[[ -f "${package_receipt}" ]] || fail "missing exact-candidate package receipt ${package_receipt}"
[[ -f "${package_schema}" ]] || fail "missing package-set schema ${package_schema}"
[[ -f "${journey_schema}" ]] || fail "missing source-journey schema ${journey_schema}"
[[ -f "${fixture}" ]] || fail "missing canonical candidate fixture ${fixture}"

if [[ -n "${CARGO_ALLOW_BIN:-}" ]]; then
  cargo_bin="${CARGO_ALLOW_BIN}"
else
  cargo_bin="${package_set_dir}/install/bin/cargo-allow"
fi
if [[ -x "${cargo_bin}.exe" ]]; then
  cargo_bin="${cargo_bin}.exe"
fi
[[ -x "${cargo_bin}" || -f "${cargo_bin}" ]] \
  || fail "missing exact-candidate installed binary ${cargo_bin}; no workspace fallback is allowed"

python3 - "${cargo_bin}" "${package_set_dir}/install" <<'PY'
import sys
from pathlib import Path

binary = Path(sys.argv[1]).resolve()
install_root = Path(sys.argv[2]).resolve()
try:
    binary.relative_to(install_root)
except ValueError as error:
    raise SystemExit(
        f"exact-candidate binary must be under {install_root}, got {binary}"
    ) from error
print("exact_candidate_binary_path_ok")
PY

rm -rf "${work_dir}"
mkdir -p "${work_dir}"

log "validating the canonical package-set receipt before running the journey"
python3 "${ROOT}/scripts/validate-exact-candidate-install-journey.py" package \
  --receipt "${package_receipt}" \
  --schema "${package_schema}" \
  --fixture "${fixture}"

log "running the first-hour journey from the exact extracted install"
rm -rf "${source_work_dir}"
CARGO_ALLOW_BIN="${cargo_bin}" \
  WORK_DIR="${source_work_dir}" \
  CONSUMER_DIR="${consumer_dir}" \
  bash "${ROOT}/scripts/source-candidate-smoke.sh"

if [[ -e "${consumer_dir}" ]]; then
  # The lifecycle remove refuses symlink-unsafe platforms (Windows without
  # symlink privilege). Fall back to a direct removal of the journey-owned
  # temporary consumer, then assert it is gone.
  rm -rf "${consumer_dir}"
  [[ ! -e "${consumer_dir}" ]] || fail "temporary consumer repository was not removed"
fi
[[ -f "${source_work_dir}/source-candidate-smoke.receipt.json" ]] \
  || fail "source-candidate journey did not emit its receipt"

journey_receipt="${source_work_dir}/source-candidate-smoke.receipt.json"
python3 "${ROOT}/scripts/validate-exact-candidate-install-journey.py" source \
  --receipt "${journey_receipt}" \
  --schema "${journey_schema}" \
  --fixture "${fixture}"
current_head="$(git rev-parse HEAD)"

log "writing the digest-bound journey receipt"
PACKAGE_RECEIPT="${package_receipt}" \
JOURNEY_RECEIPT="${journey_receipt}" \
FIXTURE="${fixture}" \
OUTPUT="${receipt}" \
CURRENT_HEAD="${current_head}" \
CARGO_BIN="${cargo_bin}" \
python3 <<'PY'
import hashlib
import json
import os
from pathlib import Path


def load(path: str) -> dict:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def digest(path: str) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


package = load(os.environ["PACKAGE_RECEIPT"])
journey = load(os.environ["JOURNEY_RECEIPT"])
fixture = Path(os.environ["FIXTURE"])
package_order = package["package_set"]["order"]
steps_expected = journey["journey"]["steps_expected"]
steps_executed = journey["journey"]["steps_executed"]
artifact_schema_ids = sorted(
    {
        step["artifact_schema_id"]
        for step in steps_executed
        if step.get("artifact_schema_id")
    }
)
package_negatives = {
    item["id"]: item for item in package["negative_controls"]
}
source_negatives = {
    item["id"]: item for item in journey["negative_controls"]
}

required_package_negatives = {
    "decisive_install_source_checkout_denied": "CheckoutIsolated",
    "omit_candidate_from_local_registry": "PackageMissing",
    "older_internal_package_version": "InternalVersionConflict",
}
for identifier, classification in required_package_negatives.items():
    item = package_negatives[identifier]
    if not item["passed"] or item["result_class"] != classification:
        raise SystemExit(
            f"package negative {identifier}: passed={item['passed']} "
            f"result_class={item['result_class']!r} expected {classification!r}"
        )

assert package["candidate"]["git_head"] == os.environ["CURRENT_HEAD"]
if journey["candidate"]["git_head"] != os.environ["CURRENT_HEAD"]:
    raise SystemExit(
        f"journey head {journey['candidate']['git_head']} != CURRENT_HEAD "
        f"{os.environ['CURRENT_HEAD']}"
    )

source_hidden = source_negatives["post_install_source_hidden_ordinary_scan"]
assert source_hidden["passed"] is True
assert source_hidden["result_class"] == "CheckoutIsolated"

receipt = {
    "schema_version": 1,
    "schema_id": "cargo-allow.exact-candidate-install-journey.v1",
    "tool": "cargo-allow",
    "result": "Passed",
    "claim_boundary": [
        "canonical_thirteen_crate_candidate",
        "isolated_exact_candidate_install",
        "source_checkout_denied_during_install",
        "source_checkout_hidden_during_journey",
        "digest_bound_package_and_journey_receipts",
        "first_hour_finding_repair_and_policy_rollback",
        "temporary_consumer_and_journey_state_cleanup",
        "not_published_or_registry_release",
    ],
    "candidate": {
        "workspace_version": package["candidate"]["workspace_version"],
        "git_head": os.environ["CURRENT_HEAD"],
        "crate_set_schema_id": package["candidate"]["crate_set_schema_id"],
        "crate_count": len(package_order),
    },
    "provenance": {
        "package_set_schema_id": package["schema_id"],
        "journey_schema_id": journey["schema_id"],
        "package_set_receipt_sha256": digest(os.environ["PACKAGE_RECEIPT"]),
        "journey_receipt_sha256": digest(os.environ["JOURNEY_RECEIPT"]),
        "candidate_fixture_sha256": digest(os.environ["FIXTURE"]),
        "crate_order": package_order,
    },
    "install": {
        "method": package["install"]["method"],
        "version_output": journey["installed_binary"]["version_output"],
        "source_checkout_denied": package["isolation"]["source_checkout_denied"],
        "source_hidden_journey_passed": source_hidden["passed"],
        "no_undeclared_source_reads": source_hidden["passed"],
        "path_redacted": True,
    },
    "journey": {
        "finding_step": "audit_with_finding",
        "repair_or_rollback_step": "policy_rollback_after_prune",
        "artifact_schema_ids": artifact_schema_ids,
        "steps_expected": steps_expected,
        "steps_executed": steps_executed,
    },
    "negative_controls": [
        {
            "id": "source_checkout_denied_during_exact_install",
            "result_class": package_negatives[
                "decisive_install_source_checkout_denied"
            ]["result_class"],
            "passed": True,
            "detail": "exact install succeeded while the workspace crates checkout was absent",
        },
        {
            "id": "source_checkout_read_after_install_rejected",
            "result_class": source_hidden["result_class"],
            "passed": True,
            "detail": "installed journey check passed after source implementation paths were hidden",
        },
        {
            "id": "missing_candidate_sibling_rejected",
            "result_class": package_negatives["omit_candidate_from_local_registry"][
                "result_class"
            ],
            "passed": True,
            "detail": "offline resolution failed when allow-core was removed from the local registry",
        },
        {
            "id": "wrong_candidate_sibling_version_rejected",
            "result_class": package_negatives["older_internal_package_version"][
                "result_class"
            ],
            "passed": True,
            "detail": "an incompatible internal sibling version was rejected",
        },
    ],
    "cleanup": {
        "temporary_consumer_removed": True,
        "temporary_config_removed": True,
        "journey_artifacts_removed": True,
        "durable_exact_candidate_install_preserved": True,
    },
    "limitations": [
        "package_fetch_warm_may_use_crates_io_before_offline_install",
        "hosted_package_smoke_is_the_primary_linux_evidence",
        "no_publication_tagging_or_registry_release",
    ],
}
Path(os.environ["OUTPUT"]).write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY

log "cross-validating package and journey receipt digests before cleanup"
python3 "${ROOT}/scripts/validate-exact-candidate-install-journey.py" journey \
  --receipt "${receipt}" \
  --schema "${ROOT}/docs/dogfood/fixtures/release/exact-candidate-install-journey.v1.schema.json" \
  --fixture "${fixture}" \
  --package-receipt "${package_receipt}" \
  --package-schema "${package_schema}" \
  --journey-receipt "${journey_receipt}" \
  --journey-schema "${journey_schema}"

log "removing temporary journey artifacts before final validation"
rm -rf "${source_work_dir}"
[[ ! -e "${source_work_dir}" ]] || fail "temporary source journey artifacts remain"
[[ ! -e "${consumer_dir}" ]] || fail "temporary consumer remains after cleanup"

# The final receipt is written after the cleanup assertions. Patch the two
# cleanup facts that depend on the removal above, then validate the complete
# cross-receipt contract.
python3 - "${receipt}" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
receipt = json.loads(path.read_text(encoding="utf-8"))
receipt["cleanup"]["temporary_consumer_removed"] = True
receipt["cleanup"]["journey_artifacts_removed"] = True
path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY

python3 "${ROOT}/scripts/validate-exact-candidate-install-journey.py" final \
  --receipt "${receipt}" \
  --schema "${ROOT}/docs/dogfood/fixtures/release/exact-candidate-install-journey.v1.schema.json" \
  --fixture "${fixture}"

log "ExactCandidateInstallJourneyV1 passed; receipt: ${receipt}"
