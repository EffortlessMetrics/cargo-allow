#!/usr/bin/env bash
# Prove the compiler cargo actually resolves in this checkout is the declared
# MSRV.
#
# `rust-version` under `[workspace.package]` is the source of truth.
# `scripts/check-msrv-consistency.sh` proves the MSRV is *stated* consistently
# across the workflows; this proves the MSRV lane is *running* it. The two are
# separate obligations: a toolchain pin can be stated correctly everywhere and
# still be overridden at run time, because rustup resolves a toolchain in
# priority order:
#
#   1. `+toolchain` argument
#   2. RUSTUP_TOOLCHAIN
#   3. `rustup override` directory setting
#   4. rust-toolchain.toml
#   5. `rustup default`
#
# `dtolnay/rust-toolchain` selects via `rustup default` (5), and this
# repository ships a `rust-toolchain.toml` (4), so the action tag alone loses.
# This guard asks the resolved compiler what it is rather than trusting the
# configuration.
#
# It does not build the workspace or prove the MSRV compiles.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

cargo_toml="${CARGO_TOML:-Cargo.toml}"
# Overridable so the characterization guard can drive this with a stub
# compiler instead of installing real toolchains.
rustc_bin="${RUSTC_BIN:-rustc}"

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

[[ -f "${cargo_toml}" ]] || fail "${cargo_toml} not found"

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

version_output="$("${rustc_bin}" --version 2>/dev/null)" \
  || fail "could not run '${rustc_bin} --version' to resolve the active toolchain"

# `rustc 1.95.0 (59807616e 2026-04-14)` -> `1.95.0`
resolved="$(printf '%s\n' "${version_output}" \
  | awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^[0-9]+\.[0-9]+(\.[0-9]+)?$/) { print $i; exit } }')"

[[ -n "${resolved}" ]] \
  || fail "$(printf 'could not parse a version out of %s output: %s' "${rustc_bin}" "${version_output}")"

# The claim is `rust-version = "1.95"`, so any 1.95.x patch satisfies it. The
# failure this catches is a different minor, not a different patch.
resolved_series="$(printf '%s\n' "${resolved}" | cut -d. -f1,2)"

if [[ "${resolved_series}" != "${msrv}" ]]; then
  fail "$(printf 'resolved toolchain is %s but %s declares rust-version = "%s".\n%s\n%s' \
    "${resolved}" "${cargo_toml}" "${msrv}" \
    "       This lane is proving the wrong Rust version. rust-toolchain.toml" \
    "       outranks 'rustup default'; set RUSTUP_TOOLCHAIN=${msrv}.0 to pin it.")"
fi

printf 'MSRV source of truth: %s rust-version = %s\n' "${cargo_toml}" "${msrv}"
printf 'ok resolved toolchain is %s\n' "${resolved}"
printf 'MSRV resolved-toolchain check passed (%s)\n' "${msrv}"
