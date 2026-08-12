#!/usr/bin/env bash
# Deterministic characterization for extraction-cutover-status.sh (#3469).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p "${ROOT}/target"
work="$(mktemp -d "${ROOT}/target/extraction-cutover-status-test.XXXXXX")"
outside="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-cutover-outside.XXXXXX")"
cleanup() { rm -rf "${work}" "${outside}"; }
trap cleanup EXIT

fake="${work}/cargo-allow-fixture"
cat >"${fake}" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
stage=""
output=""
evidence=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --stage) stage="$2"; shift 2 ;;
    --output) output="$2"; shift 2 ;;
    --cutover-evidence) evidence="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [[ -n "${evidence}" ]]; then
  printf 'fixture adapter rejection for %s\n' "${stage}" >&2
  exit 17
fi
mkdir -p "$(dirname "${output}")"
python3 - "${output}" "${stage}" "${FAKE_PARITY_MODE:-valid}" <<'PY'
import json
import subprocess
import sys
import tomllib
from pathlib import Path

output, stage_arg, mode = sys.argv[1:]
stage = {"repo-snapshot": "RepoSnapshot", "repo-edit": "RepoEdit"}[stage_arg]
source = "commit:{}/tree:{}".format(
    subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], text=True).strip(),
)
registry = tomllib.loads(Path("policy/extraction-parity.toml").read_text(encoding="utf-8"))
case_ids = sorted(case["id"] for case in registry["case"] if case["stage"] == stage)
records = [{
    "case_id": case_id,
    "result": "SemanticallyEquivalent",
    "source_identity": source,
    "old_output": "fixture-old",
    "new_output": "fixture-new",
} for case_id in case_ids]
payload = {
    "schema_id": "cargo-allow.extraction-parity-runtime.v1",
    "schema_version": 1,
    "tool": "cargo-allow extraction-parity",
    "result": "Passed",
    "completeness": "Complete",
    "source_identity": source,
    "stage": stage,
    "parity_result_digest": "sha256:v1:" + "a" * 64,
    "records": records,
    "expected_case_count": len(case_ids),
    "emitted_case_count": len(case_ids),
    "missing_case_ids": [],
    "unexpected_case_ids": [],
    "claim_boundary": ["fixture-runtime-parity"],
}
if mode == "wrong-stage":
    payload["stage"] = "RepoEdit" if stage == "RepoSnapshot" else "RepoSnapshot"
elif mode == "missing-records":
    payload.pop("records")
elif mode == "stale-identity":
    payload["source_identity"] = "commit:" + "0" * 40 + "/tree:" + "0" * 40
elif mode == "malformed":
    Path(output).write_text("{not-json\n", encoding="utf-8")
    raise SystemExit(0)
Path(output).write_text(json.dumps(payload) + "\n", encoding="utf-8")
PY
SH
chmod +x "${fake}"

missing_dir="${work}/missing"
mkdir -p "${missing_dir}/repo-snapshot"
printf 'stale\n' >"${missing_dir}/repo-snapshot/cutover-receipt.json"
EXTRACTION_CARGO_ALLOW_BIN="${fake}" EXTRACTION_CUTOVER_DIR="${missing_dir}" \
  bash scripts/extraction-cutover-status.sh >/dev/null
[[ -f "${missing_dir}/repo-snapshot/ownership.json" ]]
[[ ! -e "${missing_dir}/repo-snapshot/cutover-evidence.json" ]]
[[ ! -e "${missing_dir}/repo-snapshot/cutover-receipt.json" ]]

default_dir="${work}/default"
mkdir -p "${default_dir}/repo-snapshot" "${default_dir}/repo-edit"
printf '{}\n' >"${default_dir}/repo-snapshot/build-package.json"
printf '{}\n' >"${default_dir}/repo-edit/build-package.json"
EXTRACTION_CARGO_ALLOW_BIN="${fake}" EXTRACTION_CUTOVER_DIR="${default_dir}" \
  bash scripts/extraction-cutover-status.sh >/dev/null
[[ -f "${default_dir}/repo-snapshot/cutover-evidence.json" ]]
[[ -f "${default_dir}/repo-edit/cutover-evidence.json" ]]

configured_dir="${work}/configured"
mkdir -p "${configured_dir}"
printf '{}\n' >"${configured_dir}/independent.json"
EXTRACTION_CARGO_ALLOW_BIN="${fake}" EXTRACTION_CUTOVER_DIR="${configured_dir}" \
  EXTRACTION_BUILD_PACKAGE_RECEIPT_REPO_SNAPSHOT="${configured_dir}/independent.json" \
  EXTRACTION_BUILD_PACKAGE_RECEIPT_REPO_EDIT="${configured_dir}/independent.json" \
  bash scripts/extraction-cutover-status.sh >/dev/null
for stage in repo-snapshot repo-edit; do
  [[ -f "${configured_dir}/${stage}/cutover-evidence.json" ]]
  [[ -f "${configured_dir}/${stage}/cutover-receipt.log" ]]
  [[ ! -e "${configured_dir}/${stage}/cutover-receipt.json" ]]
done
python3 - "${configured_dir}/extraction-cutover-status.json" <<'PY'
import json
import sys
status = json.load(open(sys.argv[1], encoding="utf-8"))
assert status["result"] == "Blocked"
assert all(stage["cutover_receipt_exit_code"] == 17 for stage in status["stages"])
assert all(stage["cutover_receipt_log"].startswith("target/") for stage in status["stages"])
PY

python3 - "${ROOT}" "${configured_dir}" <<'PY'
import json
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
output = Path(sys.argv[2])
sources = {
    "repo-snapshot": root / "crates/effortless-repo-snapshot/src/parity/mod.rs",
    "repo-edit": root / "crates/effortless-repo-edit/src/parity/mod.rs",
}
for stage, source in sources.items():
    expected = sorted(set(re.findall(
        r'root\.join\("(tests/fixtures/repo-(?:snapshot|edit)/parity-[^"]+\.toml)"\)',
        source.read_text(encoding="utf-8"),
    )))
    ownership = json.loads((output / stage / "ownership.json").read_text(encoding="utf-8"))
    assert expected
    assert ownership["asset_paths"] == expected, (stage, expected, ownership["asset_paths"])
    assert ownership["ci_paths"] == [
        ".github/workflows/ci.yml",
        "scripts/extraction-cutover-status.sh",
        "scripts/test-extraction-cutover-status.sh",
    ]
PY

if EXTRACTION_CARGO_ALLOW_BIN="${fake}" EXTRACTION_CUTOVER_DIR="${outside}" \
  bash scripts/extraction-cutover-status.sh >"${work}/outside.log" 2>&1; then
  printf 'outside output directory unexpectedly succeeded\n' >&2
  exit 1
fi
grep -q 'must be inside the repository root' "${work}/outside.log"

for mode in wrong-stage missing-records stale-identity malformed; do
  negative_dir="${work}/${mode}"
  FAKE_PARITY_MODE="${mode}" EXTRACTION_CARGO_ALLOW_BIN="${fake}" \
    EXTRACTION_CUTOVER_DIR="${negative_dir}" \
    bash scripts/extraction-cutover-status.sh >/dev/null
  python3 - "${negative_dir}/extraction-cutover-status.json" "${mode}" <<'PY'
import json
import sys
status = json.load(open(sys.argv[1], encoding="utf-8"))
assert status["result"] == "Blocked"
assert any(blocker.startswith("runtime_parity_invalid:") for blocker in status["blockers"])
assert all(stage["ownership_result"] == "Blocked" for stage in status["stages"])
assert all(stage["cutover_evidence_manifest"] is None for stage in status["stages"])
PY
done

printf 'extraction cutover status contract: passed\n'
