#!/usr/bin/env bash
# Characterization checks for scripts/check-msrv-resolved.sh.
#
# Proves the guard accepts the declared MSRV series, fails closed on the exact
# regression it exists to catch (a lane silently resolving a different Rust
# minor), and reports unusable compiler output instead of passing by accident.
#
# Uses stub compilers rather than installing real toolchains, so this runs in
# about a second and needs no network.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

msrv="$(awk '
  /^\[workspace\.package\]/ { in_section = 1; next }
  /^\[/ { if (in_section) exit }
  in_section && /^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/ {
    gsub(/^[[:space:]]*rust-version[[:space:]]*=[[:space:]]*"/, "")
    gsub(/".*/, "")
    print
    exit
  }
' Cargo.toml)"

[[ -n "${msrv}" ]] || {
  printf 'fail could not read rust-version from Cargo.toml\n' >&2
  exit 1
}

# Writes an executable that answers `--version` with the given line. `%s` is
# the whole reply, so cases can pass malformed output too.
stub_rustc() {
  local name="$1" reply="$2"
  local path="${work}/${name}"
  cat >"${path}" <<EOF
#!/usr/bin/env bash
printf '%s\n' "${reply}"
EOF
  chmod +x "${path}"
  printf '%s\n' "${path}"
}

expect_success() {
  local label="$1" stub="$2"
  if RUSTC_BIN="${stub}" bash scripts/check-msrv-resolved.sh >/dev/null 2>&1; then
    printf 'ok %s\n' "${label}"
  else
    printf 'fail %s (expected zero exit)\n' "${label}" >&2
    exit 1
  fi
}

expect_failure() {
  local label="$1" stub="$2"
  if RUSTC_BIN="${stub}" bash scripts/check-msrv-resolved.sh >/dev/null 2>&1; then
    printf 'fail %s (expected non-zero exit)\n' "${label}" >&2
    exit 1
  else
    printf 'ok %s\n' "${label}"
  fi
}

expect_success "declared MSRV .0 patch is accepted" \
  "$(stub_rustc msrv-exact "rustc ${msrv}.0 (59807616e 2026-04-14)")"

# The claim is a minor-series claim, so a later patch inside the series must
# not be reported as drift.
expect_success "later patch inside the MSRV series is accepted" \
  "$(stub_rustc msrv-patch "rustc ${msrv}.7 (deadbeef1 2026-09-01)")"

# The regression this guard exists for: rust-toolchain.toml outranking the
# workflow pin, so the lane resolves a neighbouring stable instead of the MSRV.
older="$(awk -F. -v v="${msrv}" 'BEGIN { split(v, p, "."); print p[1] "." p[2] - 1 }')"
newer="$(awk -F. -v v="${msrv}" 'BEGIN { split(v, p, "."); print p[1] "." p[2] + 1 }')"

expect_failure "older Rust minor is rejected" \
  "$(stub_rustc older "rustc ${older}.1 (e408947bf 2026-03-25)")"
expect_failure "newer Rust minor is rejected" \
  "$(stub_rustc newer "rustc ${newer}.0 (abcdef123 2026-06-01)")"

# A major bump shares neither field and must not slip through on a prefix
# match.
major="$(awk -F. -v v="${msrv}" 'BEGIN { split(v, p, "."); print p[1] + 1 "." p[2] }')"
expect_failure "different major is rejected" \
  "$(stub_rustc major "rustc ${major}.0 (abcdef123 2027-01-01)")"

# Unparseable or absent compiler output must fail closed rather than be read
# as a match.
expect_failure "unparseable version output is rejected" \
  "$(stub_rustc garbage "rustc (unknown build)")"

missing="${work}/definitely-not-a-compiler"
expect_failure "missing compiler is rejected" "${missing}"

printf 'all check-msrv-resolved characterization checks passed\n'
