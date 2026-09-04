#!/usr/bin/env bash
# Exact installed 0.1.11 -> candidate -> 0.1.11 journey (#3853).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_SET_DIR="${PACKAGE_SET_DIR:-${ROOT}/target/exact-candidate-package-set}"
WORK_DIR="${WORK_DIR:-${ROOT}/target/exact-upgrade-rollback-journey}"
PACKAGE_RECEIPT="${PACKAGE_SET_DIR}/exact-candidate-package-set.receipt.json"
CANDIDATE_BIN="${CANDIDATE_BIN:-}"
if [[ -z "${CANDIDATE_BIN}" ]]; then
  candidate_default="${PACKAGE_SET_DIR}/install/bin/cargo-allow"
  # MSYS stat aliases cargo-allow to cargo-allow.exe, so probe the .exe
  # first and let Windows Python see the real file name.
  if [[ -f "${candidate_default}.exe" ]]; then
    CANDIDATE_BIN="${candidate_default}.exe"
  elif [[ -f "${candidate_default}" ]]; then
    CANDIDATE_BIN="${candidate_default}"
  else
    fail "missing exact candidate package install binary under ${PACKAGE_SET_DIR}/install/bin"
  fi
fi
RECEIPT="${WORK_DIR}/exact-upgrade-rollback-journey.receipt.json"
SCHEMA="${ROOT}/docs/dogfood/fixtures/release/exact-upgrade-rollback-journey.v1.schema.json"
FIXTURE="${ROOT}/docs/dogfood/fixtures/release/upgrade-rollback-repository.toml"

fail() { printf 'exact-upgrade-rollback: error: %s\n' "$*" >&2; exit 1; }
[[ -f "${PACKAGE_RECEIPT}" ]] || fail "missing exact candidate package receipt"
[[ -f "${SCHEMA}" && -f "${FIXTURE}" ]] || fail "missing journey contract"
[[ -x "${CANDIDATE_BIN}" || -f "${CANDIDATE_BIN}" ]] || fail "missing candidate binary"
command -v cargo >/dev/null || fail "cargo is required"
command -v python3 >/dev/null || fail "python3 is required"
command -v git >/dev/null || fail "git is required"
python3 - "${CANDIDATE_BIN}" "${PACKAGE_SET_DIR}/install" <<'PY'
import sys
from pathlib import Path
binary = Path(sys.argv[1]).resolve(strict=True)
install = Path(sys.argv[2]).resolve(strict=True)
try:
    binary.relative_to(install)
except ValueError as error:
    raise SystemExit(f"candidate binary must be below exact install root {install}") from error
if binary.is_symlink():
    raise SystemExit("candidate binary must not be a symlink")
PY

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/old-install" "${WORK_DIR}/old-cargo-home" "${WORK_DIR}/repository"
old_root="${WORK_DIR}/old-install"
old_bin="${old_root}/bin/cargo-allow"
if [[ ! -f "${old_bin}" && -f "${old_bin}.exe" ]]; then
  old_bin="${old_bin}.exe"
fi
if [[ -n "${OLD_BIN:-}" ]]; then
  old_bin="$(python3 - "${OLD_BIN}" "${WORK_DIR}" <<'PY'
import sys
from pathlib import Path
p = Path(sys.argv[1]).resolve(strict=True)
root = Path(sys.argv[2]).resolve()
try:
    p.relative_to(root)
except ValueError as error:
    raise SystemExit(f"OLD_BIN must be under {root}") from error
if p.is_symlink() or not p.is_file():
    raise SystemExit("OLD_BIN must be a regular file")
print(p)
PY
)"
else
  CARGO_HOME="${WORK_DIR}/old-cargo-home" cargo install cargo-allow --version 0.1.11 --locked --root "${old_root}" --quiet
fi
[[ -f "${old_bin}" ]] || fail "exact 0.1.11 binary was not installed"

version() { "$1" --version | tr -d '\r'; }
old_version="$(version "${old_bin}")"
candidate_version="$(version "${CANDIDATE_BIN}")"
[[ "${old_version}" == cargo-allow\ 0.1.11* ]] || fail "old binary is not exact 0.1.11: ${old_version}"
[[ "${candidate_version}" == cargo-allow\ 0.2.0* ]] || fail "candidate binary is not exact 0.2.0: ${candidate_version}"

repo="${WORK_DIR}/repository"
mkdir -p "${repo}/src" "${repo}/policy"
cp "${FIXTURE}" "${repo}/policy/allow.toml"
printf 'fn main() {}\n' > "${repo}/src/main.rs"
printf 'unrelated-preserved\n' > "${repo}/unrelated.txt"
git -C "${repo}" init -q
git -C "${repo}" config user.name cargo-allow-proof
git -C "${repo}" config user.email cargo-allow-proof@localhost
git -C "${repo}" add src policy unrelated.txt
git -C "${repo}" commit -qm initial
cp -a "${repo}" "${WORK_DIR}/preimage"

run_readonly_leg() {
  local bin="$1" root="$2" label="$3"
  "${bin}" doctor --root "${root}" --config "${root}/policy/allow.toml" >/dev/null
  "${bin}" audit --root "${root}" --config "${root}/policy/allow.toml" --format json >/dev/null
  "${bin}" check --root "${root}" --config "${root}/policy/allow.toml" --mode no-new --format json >/dev/null
  printf '%s\n' "${label}:doctor" "${label}:audit" "${label}:check"
}

old_steps="$(run_readonly_leg "${old_bin}" "${repo}" old-0.1.11)"
candidate_steps="$(run_readonly_leg "${CANDIDATE_BIN}" "${repo}" candidate-0.2.0)"
[[ "$(cat "${repo}/unrelated.txt")" == unrelated-preserved ]] || fail "candidate changed unrelated control"
rm -rf "${repo}"
cp -a "${WORK_DIR}/preimage" "${repo}"
rollback_steps="$(run_readonly_leg "${old_bin}" "${repo}" rollback-0.1.11)"
[[ "$(cat "${repo}/unrelated.txt")" == unrelated-preserved ]] || fail "rollback lost unrelated control"

PACKAGE_RECEIPT="${PACKAGE_RECEIPT}" OLD_BIN="${old_bin}" CANDIDATE_BIN="${CANDIDATE_BIN}" \
WORK_DIR="${WORK_DIR}" RECEIPT="${RECEIPT}" FIXTURE="${FIXTURE}" \
OLD_VERSION="${old_version}" CANDIDATE_VERSION="${candidate_version}" \
OLD_STEPS="${old_steps}" CANDIDATE_STEPS="${candidate_steps}" ROLLBACK_STEPS="${rollback_steps}" \
python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

package = json.loads(Path(os.environ["PACKAGE_RECEIPT"]).read_text(encoding="utf-8"))
receipt = {
    "schema_version": 1,
    "schema_id": "cargo-allow.exact-upgrade-rollback-journey.v1",
    "tool": "cargo-allow",
    "result": "Passed",
    "claim_boundary": ["exact_public_0.1.11", "exact_candidate_0.2.0", "isolated_three_leg_journey", "bounded_repository_state_restore"],
    "from": {"version": os.environ["OLD_VERSION"], "binary_sha256": digest(os.environ["OLD_BIN"]), "install_root_id": "old-install"},
    "candidate": {"version": os.environ["CANDIDATE_VERSION"], "binary_sha256": digest(os.environ["CANDIDATE_BIN"]), "package_set_receipt_sha256": digest(os.environ["PACKAGE_RECEIPT"]), "git_head": package["candidate"]["git_head"]},
    "rollback": {"version": os.environ["OLD_VERSION"], "binary_sha256": digest(os.environ["OLD_BIN"]), "restored_preimage": True},
    "repository": {"fixture_sha256": digest(os.environ["FIXTURE"]), "unrelated_file_preserved": True, "source_checkout_not_used_as_binary": True},
    "steps": ([{"leg": "from", "id": x} for x in os.environ["OLD_STEPS"].splitlines()] + [{"leg": "candidate", "id": x} for x in os.environ["CANDIDATE_STEPS"].splitlines()] + [{"leg": "rollback", "id": x} for x in os.environ["ROLLBACK_STEPS"].splitlines()]),
    "negative_controls": [
        {"id": "old_binary_exact_version", "result_class": "ExactVersion", "passed": True},
        {"id": "candidate_binary_exact_version", "result_class": "ExactVersion", "passed": True},
        {"id": "checkout_binary_not_used", "result_class": "CheckoutIsolated", "passed": True},
        {"id": "unrelated_file_survives_rollback", "result_class": "UnrelatedStatePreserved", "passed": True},
    ],
    "limitations": ["candidate_leg_consumes_the_existing_exact_candidate_package_receipt", "migration_write_surfaces_are_not_claimed_by_this_read_only_slice"],
}
Path(os.environ["RECEIPT"]).write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY

python3 "${ROOT}/scripts/validate-upgrade-rollback-journey.py" --receipt "${RECEIPT}" --schema "${SCHEMA}" --fixture "${FIXTURE}"
printf 'exact-upgrade-rollback: passed; receipt: %s\n' "${RECEIPT}"
