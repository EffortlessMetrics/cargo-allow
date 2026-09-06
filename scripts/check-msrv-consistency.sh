#!/usr/bin/env bash
# Prove the MSRV claim is stated once and repeated consistently everywhere it
# is enforced or published.
#
# `rust-version` under `[workspace.package]` is the source of truth. This
# compares the declared value with the CI toolchain/cache and release
# attestation. It does not invoke rustc or prove a build on the MSRV.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

cargo_toml="${CARGO_TOML:-Cargo.toml}"
ci_workflow="${CI_WORKFLOW:-.github/workflows/ci.yml}"
release_workflow="${RELEASE_WORKFLOW:-.github/workflows/release.yml}"
# Optional: only repositories that ship a toolchain file need the workflow to
# outrank it. Checked for existence rather than required, so dropping the file
# relaxes the guard instead of breaking it.
toolchain_file="${TOOLCHAIN_FILE:-rust-toolchain.toml}"

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

for file in "${cargo_toml}" "${ci_workflow}" "${release_workflow}"; do
  [[ -f "${file}" ]] || fail "${file} not found"
done

msrv="$(awk '
  /^\[workspace\.package\]/ { in_section = 1; next }
  /^\[/ { if (in_section) exit }
  in_section && /^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/ {
    gsub(/^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/, "")
    gsub(/".*/, "")
    print
    exit
  }
' "${cargo_toml}")"

[[ -n "${msrv}" ]] || fail "no rust-version found under [workspace.package] in ${cargo_toml}"
msrv_ere="${msrv//./\\.}"
printf 'MSRV source of truth: %s rust-version = %s\n' "${cargo_toml}" "${msrv}"

# Every versioned toolchain pin in the workflow graph must match the
# MSRV: the #3836 pre-gate and other lanes each carry their own pin, and
# a single drifted pin would resolve that lane to the wrong compiler
# while a presence-only check still reported green.
# Active uses only: a commented-out pin is not configuration, so the
# comment lines are stripped before both the positive and the every-pin
# checks.
# Only `uses:` declarations count: a run-script string or inline
# comment naming the action is not an installed toolchain.
active_pins="$(grep -v "^[[:space:]]*#" "${ci_workflow}"   | grep "uses:" | grep -oE "dtolnay/rust-toolchain@[0-9][^ ]*" || true)"
if ! printf "%s\n" "${active_pins}" | grep -qF "dtolnay/rust-toolchain@${msrv}.0"; then
  fail "$(printf "%s carries no active toolchain pin on the declared MSRV %s." \
    "${ci_workflow}" "${msrv}")"
fi
off_msrv="$(printf "%s\n" "${active_pins}" \
  | grep -vF "dtolnay/rust-toolchain@${msrv}.0" || true)"
if [[ -n "${off_msrv}" ]]; then
  fail "$(printf "%s pins off-MSRV toolchain(s): %s." \
    "${ci_workflow}" "${off_msrv}")"
fi
printf 'ok %s pins dtolnay/rust-toolchain@%s.0 (all pins)\n' "${ci_workflow}" "${msrv}"

if ! grep -qF "key: msrv-${msrv}" "${ci_workflow}" \
  && ! grep -qE '^[[:space:]]+lane:[[:space:]]+msrv[[:space:]]*$' "${ci_workflow}"; then
  actual="$(grep -oE 'key: msrv-[^ ]*|lane: msrv[^[:space:]]*' "${ci_workflow}" | head -n 1 || true)"
  fail "$(printf '%s uses cache %s but the MSRV is %s.\n%s' \
    "${ci_workflow}" "${actual:-no msrv cache key}" "${msrv}" \
    "       Use the cache lane: msrv (or key: msrv-${msrv} for inline configuration).")"
fi
printf 'ok %s uses cache namespace msrv-%s\n' "${ci_workflow}" "${msrv}"

# The action tag above is stated configuration, not the resolved toolchain.
# rustup ranks `rust-toolchain.toml` above `rustup default`, and `rustup
# default` is how `dtolnay/rust-toolchain` applies its tag, so while this
# repository ships a `rust-toolchain.toml` the tag alone resolves to whatever
# the runner's channel happens to be. RUSTUP_TOOLCHAIN outranks the toolchain
# file and is what actually pins the lane; without it the MSRV proof is inert
# while every check above still reports green.
# The pin only counts if it is on the `msrv` job itself. Searching the whole
# file would accept a RUSTUP_TOOLCHAIN belonging to some other job while the
# msrv lane sits unpinned and silently resolves the wrong compiler — the exact
# failure this guard exists to prevent. So extract the job block first: from
# the `  msrv:` key to the next top-level job key (a line indented exactly two
# spaces ending in `:`), and match only within it.
if [[ -f "${toolchain_file}" ]]; then
  # Job headers may carry a trailing comment. Tolerating it matters most on the
  # terminator: a missed end key would run the block on into later jobs and
  # accept their RUSTUP_TOOLCHAIN, reopening exactly the hole this scoping
  # closes. A missed start key only fails closed, which is safe but noisy.
  msrv_job="$(awk '
    /^  msrv:[[:space:]]*(#.*)?$/ { in_job = 1; next }
    in_job && /^  [A-Za-z0-9_-]+:[[:space:]]*(#.*)?$/ { exit }
    in_job { print }
  ' "${ci_workflow}")"

  [[ -n "${msrv_job}" ]] || fail "$(printf 'no `msrv:` job found in %s.\n%s' \
    "${ci_workflow}" \
    "       This guard checks that job specifically; rename it here if it moved.")"

  if ! printf '%s\n' "${msrv_job}" \
    | grep -qE "RUSTUP_TOOLCHAIN:[[:space:]]*\"?${msrv_ere}\.[0-9]+\"?[[:space:]]*$"; then
    actual="$(printf '%s\n' "${msrv_job}" \
      | grep -oE 'RUSTUP_TOOLCHAIN: "?[^"[:space:]]+"?' | head -n 1 || true)"
    fail "$(printf '%s ships %s, which outranks the workflow toolchain tag.\n%s msrv job sets %s but the MSRV is %s.\n%s' \
      "${ROOT}" "${toolchain_file}" \
      "       ${ci_workflow}" "${actual:-no RUSTUP_TOOLCHAIN value}" "${msrv}" \
      "       Set RUSTUP_TOOLCHAIN: \"${msrv}.0\" on the msrv job.")"
  fi
  printf 'ok %s msrv job pins RUSTUP_TOOLCHAIN to the %s series over %s\n' \
    "${ci_workflow}" "${msrv}" "${toolchain_file}"
fi

if ! grep -qE "MSRV:[[:space:]]*\"?${msrv_ere}\"?[[:space:]]*$" "${release_workflow}"; then
  actual="$(grep -oE 'MSRV: "?[^"[:space:]]+"?' "${release_workflow}" | head -n 1 || true)"
  fail "$(printf '%s sets %s but the MSRV is %s.\n%s' \
    "${release_workflow}" "${actual:-no MSRV value}" "${msrv}" \
    "       Set MSRV: \"${msrv}\".")"
fi
printf 'ok %s attests MSRV %s\n' "${release_workflow}" "${msrv}"
printf 'MSRV consistency check passed (%s)\n' "${msrv}"
