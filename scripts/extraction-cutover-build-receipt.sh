#!/usr/bin/env bash
# Independent build/package receipt producer for extraction cutover stages
# (#3469 slice A / #3552).
#
# Supplies the fail-closed `cargo-allow extraction-parity --cutover-evidence`
# flow with a real independent build/package receipt for one stage, sourced
# from the exact-candidate package-set smoke artifacts (offline packaging with
# the workspace checkout stashed — the smoke's negative controls prove
# source-checkout denial).
#
# The producer binds the receipt to the CURRENT source identity and parity
# digest and refuses stale or incomplete inputs:
#   - package-set receipt git_head must equal the current HEAD
#   - every expected stage package must have a packaged crate with a
#     matching sha256
#
# Usage:
#   bash scripts/extraction-cutover-build-receipt.sh <repo-snapshot|repo-edit>
# Optional:
#   PACKAGE_SET_DIR=<path>   package-set smoke work dir
#                            (default target/exact-candidate-package-set)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

stage_arg="${1:-}"
case "${stage_arg}" in
  repo-snapshot) stage="RepoSnapshot" ;;
  repo-edit) stage="RepoEdit" ;;
  *)
    printf 'usage: %s <repo-snapshot|repo-edit>\n' "$0" >&2
    exit 2
    ;;
esac

package_set_dir="${PACKAGE_SET_DIR:-${ROOT}/target/exact-candidate-package-set}"
package_set_receipt="${package_set_dir}/exact-candidate-package-set.receipt.json"
stage_dir="${ROOT}/target/extraction-cutover/${stage_arg}"
parity_output="${stage_dir}/parity.json"
build_receipt="${stage_dir}/build-package.json"

log() {
  printf 'cutover-build-receipt: %s\n' "$*"
}

fail() {
  printf 'cutover-build-receipt: error: %s\n' "$*" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || fail "cargo is required"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"
command -v git >/dev/null 2>&1 || fail "git is required"

[[ -f "${package_set_receipt}" ]] \
  || fail "missing package-set receipt ${package_set_receipt}; run scripts/exact-candidate-package-set.sh first"

mkdir -p "${stage_dir}"

log "stage ${stage}: run runtime parity for binding"
cargo run -q -p cargo-allow -- extraction-parity --stage "${stage_arg}" --output "${parity_output}" \
  || fail "runtime parity failed for stage ${stage}"

EXTRACTION_STAGE="${stage}" \
PACKAGE_SET_RECEIPT="${package_set_receipt}" \
PARITY_OUTPUT="${parity_output}" \
BUILD_RECEIPT="${build_receipt}" \
python3 <<'PY'
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

stage = os.environ["EXTRACTION_STAGE"]
package_set = json.loads(Path(os.environ["PACKAGE_SET_RECEIPT"]).read_text(encoding="utf-8"))
parity = json.loads(Path(os.environ["PARITY_OUTPUT"]).read_text(encoding="utf-8"))

def fail(message):
    print(f"cutover-build-receipt: error: {message}", file=sys.stderr)
    raise SystemExit(1)

if parity.get("result") != "Passed":
    fail(f"runtime parity result is {parity.get('result')!r}; refusing to bind receipts")

expectation = parity.get("cutover_expectation")
if not isinstance(expectation, dict):
    fail("parity output is missing the cutover_expectation block")

source_identity = parity.get("source_identity")
parity_digest = parity.get("parity_result_digest")
if not source_identity or not parity_digest:
    fail("parity output is missing source identity or digest")

current_head = subprocess.run(
    ["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True
).stdout.strip()
receipt_head = (package_set.get("candidate") or {}).get("git_head")
if receipt_head != current_head:
    fail(
        f"package-set receipt is stale: built at {receipt_head!r}, current HEAD is {current_head!r}"
    )
if not source_identity.startswith(f"commit:{current_head}/"):
    fail("parity source identity does not match the current commit")

root = Path(os.environ["PACKAGE_SET_RECEIPT"]).parents[2]
crates = {record["name"]: record for record in package_set.get("package_set", {}).get("crates", [])}
expected_packages = sorted(expectation.get("package_names") or [])
if not expected_packages:
    fail("cutover expectation lists no packages")

package_records = []
for name in expected_packages:
    record = crates.get(name)
    if record is None:
        fail(f"package-set receipt has no packaged crate for expected package `{name}`")
    crate_path = Path(os.environ["PACKAGE_SET_RECEIPT"]).parent / "packages" / record["crate_file"]
    if not crate_path.is_file():
        fail(f"packaged crate missing on disk: {crate_path}")
    digest = hashlib.sha256(crate_path.read_bytes()).hexdigest()
    if record.get("sha256") != digest:
        fail(f"packaged crate sha256 drift for `{name}`: receipt {record.get('sha256')} vs disk {digest}")
    package_records.append(
        {
            "package_name": name,
            "path": crate_path.relative_to(root).as_posix(),
            "sha256": f"sha256:v1:{digest}" if not digest.startswith("sha256") else digest,
            "result": "Passed",
        }
    )

set_receipt_path = Path(os.environ["PACKAGE_SET_RECEIPT"])
set_receipt_digest = hashlib.sha256(set_receipt_path.read_bytes()).hexdigest()
receipt = {
    "schema_id": "cargo-allow.extraction-cutover-build-package.v1",
    "schema_version": 1,
    "stage": stage,
    "source_identity": source_identity,
    "architecture_manifest_digest": expectation["architecture_manifest_digest"],
    "parity_result_digest": parity_digest,
    "result": "Passed",
    "independent": True,
    "source_checkout_denied": True,
    "package_records": package_records,
    "build_records": [
        {
            "artifact_name": f"{stage.lower()}-exact-candidate-package-set",
            "path": set_receipt_path.relative_to(root).as_posix(),
            "sha256": f"sha256:v1:{set_receipt_digest}",
            "result": "Passed",
        }
    ],
    "claim_boundary": (
        "Packages packaged by the exact-candidate package-set smoke "
        "(offline local-registry install; workspace checkout stashed during decisive "
        "install — source_checkout_denied_during_decisive_install); bound to the "
        "exact git commit/tree identity and the runtime parity digest of this stage."
    ),
}
Path(os.environ["BUILD_RECEIPT"]).write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
print(f"cutover-build-receipt: wrote {os.environ['BUILD_RECEIPT']} for {stage} "
      f"({len(package_records)} packages)")
PY

log "stage ${stage}: build receipt ready at ${build_receipt}"
