#!/usr/bin/env bash
# Hosted/local characterization: shallow checkout makes diff --base fail closed,
# then deepened history makes the same base succeed (#2366 / #2355).
#
# Does not download crates.io. Builds or uses a local cargo-allow binary.
#
# Usage:
#   scripts/shallow-diff-base-smoke.sh
#
# Optional:
#   WORK_DIR=<path>          work root (default: target/shallow-diff-base-smoke)
#   CARGO_ALLOW_BIN=<path>   prebuilt binary (default: build via cargo)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${WORK_DIR:-${ROOT}/target/shallow-diff-base-smoke}"
receipt="${work_dir}/shallow-diff-base-smoke.receipt.txt"
shallow_repo="${work_dir}/shallow-clone"
artifact_dir="${work_dir}/artifacts"

log() {
  printf 'shallow-diff-base-smoke: %s\n' "$*"
}

fail() {
  printf 'shallow-diff-base-smoke: error: %s\n' "$*" >&2
  exit 1
}

command -v git >/dev/null 2>&1 || fail "git is required"
command -v cargo >/dev/null 2>&1 || fail "cargo is required"

git -C "${ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
  || fail "must run inside the cargo-allow git work tree"

# Need at least one parent commit in the full repository to use as --base.
parent_sha="$(git -C "${ROOT}" rev-parse HEAD^)" \
  || fail "repository HEAD has no parent; need history for the base ref"

head_sha="$(git -C "${ROOT}" rev-parse HEAD)"
head_short="$(git -C "${ROOT}" rev-parse --short HEAD)"

rm -rf "${work_dir}"
mkdir -p "${artifact_dir}"
: >"${receipt}"
{
  echo "schema_id=cargo-allow.shallow-diff-base-smoke.v1"
  echo "root=${ROOT}"
  echo "head_sha=${head_sha}"
  echo "base_sha=${parent_sha}"
  echo "started_unix=$(date +%s)"
} >>"${receipt}"

if [[ -n "${CARGO_ALLOW_BIN:-}" ]]; then
  bin="${CARGO_ALLOW_BIN}"
  [[ -x "${bin}" ]] || fail "CARGO_ALLOW_BIN is not executable: ${bin}"
else
  log "building cargo-allow binary"
  cargo build -p cargo-allow --locked
  bin="${ROOT}/target/debug/cargo-allow"
  [[ -x "${bin}" ]] || fail "missing built binary at ${bin}"
fi
{
  echo "cargo_allow_bin=${bin}"
  echo "cargo_allow_version=$("${bin}" --version | tr -d '\r')"
} >>"${receipt}"

log "creating depth-1 clone at ${shallow_repo}"
git clone --depth 1 "file://${ROOT}" "${shallow_repo}"
# file:// clones of a worktree may still resolve objects via alternates; force a
# truly shallow tip-only view by verifying the parent is absent.
if git -C "${shallow_repo}" cat-file -e "${parent_sha}^{commit}" 2>/dev/null; then
  # Some git versions keep parent objects reachable via the file:// source.
  # Re-clone from a bundle that contains only HEAD to guarantee absence.
  bundle="${work_dir}/head-only.bundle"
  git -C "${ROOT}" bundle create "${bundle}" HEAD
  rm -rf "${shallow_repo}"
  git clone --depth 1 "${bundle}" "${shallow_repo}"
fi

if git -C "${shallow_repo}" cat-file -e "${parent_sha}^{commit}" 2>/dev/null; then
  fail "shallow clone unexpectedly contains base ${parent_sha}"
fi
echo "shallow_missing_base=1" >>"${receipt}"

shallow_fail_out="${artifact_dir}/shallow-missing-base.md"
shallow_fail_log="${artifact_dir}/shallow-missing-base.stderr.txt"
log "expecting diff --base ${parent_sha} to fail in shallow clone"
set +e
(
  cd "${shallow_repo}"
  "${bin}" diff \
    --base "${parent_sha}" \
    --format markdown \
    --output "${shallow_fail_out}"
) >"${artifact_dir}/shallow-missing-base.stdout.txt" 2>"${shallow_fail_log}"
shallow_code=$?
set -e
echo "shallow_exit=${shallow_code}" >>"${receipt}"
if [[ "${shallow_code}" -eq 0 ]]; then
  fail "shallow diff unexpectedly succeeded (exit 0)"
fi
if ! grep -Eiq 'could not be resolved|invalid revision|unknown revision|failed to resolve|not a valid' \
  "${shallow_fail_log}" "${artifact_dir}/shallow-missing-base.stdout.txt" 2>/dev/null; then
  # Still accept any non-zero exit as fail-closed; record that the message was soft.
  echo "shallow_diagnostic=exit_nonzero_without_matched_phrase" >>"${receipt}"
else
  echo "shallow_diagnostic=matched_revision_failure" >>"${receipt}"
fi
echo "shallow_negative=pass" >>"${receipt}"

log "deepening shallow clone to include base ${parent_sha}"
# Prefer unshallow when the remote has more history; otherwise fetch the SHA.
if ! git -C "${shallow_repo}" fetch --unshallow origin; then
  log "unshallow fetch unavailable; trying a bounded depth fetch"
  git -C "${shallow_repo}" fetch --depth=50 origin HEAD \
    || fail "could not deepen shallow clone for base ${parent_sha}"
fi
# Ensure the exact parent object exists: fetch from the full repo by SHA.
git -C "${ROOT}" bundle create "${work_dir}/with-parent.bundle" "${parent_sha}" HEAD \
  || fail "could not create a bundle containing base ${parent_sha}"
git -C "${shallow_repo}" fetch "${work_dir}/with-parent.bundle" "${parent_sha}" "${head_sha}" \
  || fail "could not fetch base ${parent_sha} from the local bundle"
git -C "${shallow_repo}" cat-file -e "${parent_sha}^{commit}" \
  || fail "deepened clone still missing base ${parent_sha}"
echo "deepened_has_base=1" >>"${receipt}"

full_out="${artifact_dir}/full-history.md"
full_log="${artifact_dir}/full-history.stderr.txt"
log "expecting diff --base ${parent_sha} to succeed after history is available"
set +e
(
  cd "${shallow_repo}"
  "${bin}" diff \
    --base "${parent_sha}" \
    --format markdown \
    --output "${full_out}"
) >"${artifact_dir}/full-history.stdout.txt" 2>"${full_log}"
full_code=$?
set -e
echo "full_exit=${full_code}" >>"${receipt}"
if [[ "${full_code}" -ne 0 ]]; then
  echo "full_history_positive=fail" >>"${receipt}"
  echo "full_history_error_artifact=${full_log}" >>"${receipt}"
  echo "result=fail" >>"${receipt}"
  cat "${artifact_dir}/full-history.stdout.txt"
  cat "${full_log}" >&2
  fail "full-history diff failed (exit ${full_code}); see ${full_log}"
fi
[[ -f "${full_out}" ]] || fail "missing full-history markdown artifact"
echo "full_history_positive=pass" >>"${receipt}"

{
  echo "head_short=${head_short}"
  echo "artifact_dir=${artifact_dir}"
  echo "finished_unix=$(date +%s)"
  echo "result=pass"
} >>"${receipt}"

log "ok receipt=${receipt}"
