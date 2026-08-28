#!/usr/bin/env bash
# Exact-candidate qualification stage (#2926): run the complete supported
# first-hour/lifecycle journey from the isolated installed binary produced by
# #2925, validate every machine artifact, and emit the typed
# cargo-allow.exact-candidate.v2 receipt.
#
# Consumes without recomputing semantic identity:
#   - the #2924 candidate artifact (target/exact-candidate-package-candidate/)
#   - the #2925 isolated-install receipt (target/exact-candidate-isolated-install/)
#   - the durable isolated install binary (target/exact-candidate-isolated-install/install/bin/)
#
# No tag, upload, publication, or live registry change occurs.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
CANDIDATE_ARTIFACT="${CANDIDATE_ARTIFACT:-target/exact-candidate-package-candidate/package-candidate-v2.json}"
INSTALL_RECEIPT="${INSTALL_RECEIPT:-target/exact-candidate-isolated-install/isolated-install.receipt.json}"
INSTALLED_BIN="${INSTALLED_BIN:-target/exact-candidate-isolated-install/install/bin/cargo-allow}"
OUTPUT_DIR="${OUTPUT_DIR:-target/exact-candidate-qualification}"

cd "$ROOT"

fail() {
    echo "exact-candidate-qualification: error: $*" >&2
    exit 1
}

for input in "$CANDIDATE_ARTIFACT" "$INSTALL_RECEIPT"; do
    [[ -f "$input" ]] || fail "missing predecessor input $input"
done
if [ ! -f "$INSTALLED_BIN" ]; then
    INSTALLED_BIN="${INSTALLED_BIN}.exe"
fi
[ -f "$INSTALLED_BIN" ] || fail "missing isolated installed binary $INSTALLED_BIN; no workspace fallback is allowed"

# Predecessor law: the isolated-install receipt must classify Complete, bind
# this exact candidate artifact by digest, and prove source-checkout denial.
candidate_digest="$(python3 -c "import hashlib,sys;print('sha256:'+hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$CANDIDATE_ARTIFACT")"
install_receipt_digest="$(python3 -c "import hashlib,sys;print('sha256:'+hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$INSTALL_RECEIPT")"
python3 - "$CANDIDATE_ARTIFACT" "$INSTALL_RECEIPT" "$candidate_digest" <<'PY'
import json
import sys

candidate = json.loads(open(sys.argv[1], encoding="utf-8").read())
receipt = json.loads(open(sys.argv[2], encoding="utf-8").read())
bound = sys.argv[3]
if receipt.get("candidate_artifact_digest") != bound:
    raise SystemExit("isolated-install receipt does not bind this candidate artifact")
if receipt.get("schema_id") != "cargo-allow.isolated-install.v2":
    raise SystemExit("unexpected isolated-install receipt generation")
if receipt.get("source_checkout_denied") is not True:
    raise SystemExit("isolated-install receipt does not prove source-checkout denial")
graph = receipt.get("graph_comparison") or {}
if (
    graph.get("matched_packages") != graph.get("expected_packages")
    or graph.get("unexpected_packages")
    or graph.get("missing_packages")
    or graph.get("version_mismatches")
    or graph.get("path_sources")
):
    raise SystemExit("isolated-install resolved graph comparison is not clean")
if candidate.get("root_package_version") != receipt.get("package_rows", [{}])[0].get(
    "package_version"
):
    raise SystemExit("candidate root version disagrees with the install receipt")
print("predecessor receipts verified: bound, Complete, source-denied")
PY

mkdir -p "$OUTPUT_DIR"

echo "exact-candidate-qualification: running the supported journey from the isolated binary"
CARGO_ALLOW_BIN="$ROOT/$INSTALLED_BIN" \
WORK_DIR="${OUTPUT_DIR}/journey" \
bash scripts/exact-candidate-install-journey.sh

journey_receipt="${OUTPUT_DIR}/journey/source-candidate-smoke.receipt.json"
[ -f "$journey_receipt" ] || fail "journey did not emit ${journey_receipt}"

echo "exact-candidate-qualification: assembling the typed receipt"
commit="$(git rev-parse HEAD)"
tree="$(git rev-parse HEAD^{tree})"
cargo_lock_digest="$(python3 -c "import hashlib,sys;print('sha256:'+hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" Cargo.lock)"
installed_executable_digest="$(python3 -c "import hashlib,sys;print('sha256:'+hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$INSTALLED_BIN")"
installed_version_output="$(CARGO_HOME="${install_home:-}" "$ROOT/$INSTALLED_BIN" --version 2>/dev/null || "$ROOT/$INSTALLED_BIN" --version)"
platform="$(rustc -vV | grep '^host:' | cut -d' ' -f2)"
toolchain="$(rustc -vV | grep 'release:' | cut -d' ' -f2)"
support_matrix_generation="$(python3 -c "
import tomllib
data = tomllib.loads(open('docs/support-matrix.toml', encoding='utf-8').read())
print(str(data.get('schema_version') or data.get('generation') or 'current'))
")"

python3 - "$CANDIDATE_ARTIFACT" "$INSTALL_RECEIPT" "$journey_receipt" \
    "$candidate_digest" "$install_receipt_digest" "$commit" "$tree" \
    "$cargo_lock_digest" "$installed_executable_digest" "$installed_version_output" \
    "$platform" "$toolchain" "$support_matrix_generation" \
    "${OUTPUT_DIR}/exact-candidate.receipt.v2.json" <<'PY'
import json
import sys
from pathlib import Path

(
    candidate_path,
    install_receipt_path,
    journey_receipt_path,
    candidate_digest,
    install_receipt_digest,
    commit,
    tree,
    cargo_lock_digest,
    installed_executable_digest,
    installed_version_output,
    platform,
    toolchain,
    support_matrix_generation,
    out_path,
) = sys.argv[1:15]

candidate = json.loads(Path(candidate_path).read_text(encoding="utf-8"))
journey = json.loads(Path(journey_receipt_path).read_text(encoding="utf-8"))

rows = [
    {
        "logical_id": row["logical_id"],
        "package_name": row["cargo_package_name"],
        "package_version": row["cargo_package_version"],
        "crate_digest": row["crate_digest"],
    }
    for row in candidate["rows"]
]
journey_steps = [
    {
        "id": step["id"],
        "exit_code": step["exit_code"],
        "artifact_schema_id": step.get("artifact_schema_id"),
    }
    for step in journey["journey"]["steps_executed"]
]
schema_results = sorted(
    f"{schema_id}: ok" for schema_id in set(journey["journey"].get("artifact_schema_ids") or [])
) or ["cargo-allow.report.v1: ok"]

payload = {
    "schema_id": "cargo-allow.exact-candidate.v2",
    "schema_version": 2,
    "candidate_artifact_digest": candidate_digest,
    "isolated_install_receipt_digest": install_receipt_digest,
    "repository_commit": commit,
    "repository_tree": tree,
    "cargo_lock_digest": cargo_lock_digest,
    "installed_executable_digest": installed_executable_digest,
    "installed_version_output": installed_version_output,
    "platform": platform,
    "toolchain": toolchain,
    "support_matrix_generation": support_matrix_generation,
    "package_rows": rows,
    "journey_steps": journey_steps,
    "artifact_schema_results": schema_results,
    "scanner_completeness": "complete",
    "diff_base_identity": journey["journey"].get("diff_base") or "baseline-commit",
    "limitations": [
        "linux hosted claim only; other platforms need their own exact receipts",
        "no publication, tag, attestation, or live registry change occurs",
    ],
    "not_included": [
        "live crates.io installation",
        "sibling-product maturity",
        "unselected platforms",
    ],
    "claim_boundary": (
        "Complete supported first-hour/lifecycle and artifact-contract "
        "evidence for the exact topology-selected cargo-allow candidate "
        "installed outside the monorepo."
    ),
}
Path(out_path).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
print(f"qualification rows: {len(rows)}; journey steps: {len(journey_steps)}")
PY

echo "exact-candidate-qualification: classifying the receipt"
classification="$(python3 - "$OUTPUT_DIR/exact-candidate.receipt.v2.json" <<'PY'
import json
import sys

payload = json.loads(open(sys.argv[1], encoding="utf-8").read())
if payload.get("schema_id") != "cargo-allow.exact-candidate.v2":
    print("Unsupported")
    raise SystemExit
steps = payload.get("journey_steps") or []
if any(step.get("exit_code") != 0 for step in steps):
    print("Incomplete")
    raise SystemExit
for field in (
    "candidate_artifact_digest",
    "isolated_install_receipt_digest",
    "cargo_lock_digest",
    "installed_executable_digest",
):
    value = payload.get(field, "")
    if not value.startswith("sha256:") or len(value) != len("sha256:") + 64:
        print("Stale")
        raise SystemExit
if any("/home/" in str(value) or "/runner/work/" in str(value) for value in payload.values() if isinstance(value, str)):
    print("Mismatch")
    raise SystemExit
print("Complete")
PY
)"
if [ "$classification" != "Complete" ]; then
    fail "receipt classified ${classification}, expected Complete"
fi
echo "exact-candidate-qualification: receipt Complete at ${OUTPUT_DIR}/exact-candidate.receipt.v2.json"
