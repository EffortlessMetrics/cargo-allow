#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/ci.yml"

fail() {
  printf 'windows cache contract: %s\n' "$1" >&2
  exit 1
}

[[ -f "$workflow" ]] || fail "missing ${workflow}"

section="$(awk '
  { sub(/\r$/, "") }
  /^  test-windows:$/ { in_job=1 }
  in_job { print }
  in_job && /^  [^ ]/ && $0 !~ /^  test-windows:$/ { exit }
' "$workflow")"

[[ -n "$section" ]] || fail "test-windows job not found"

contains() {
  grep -Fq -- "$1" <<<"$section" || fail "test-windows is missing: $1"
}

contains 'runs-on: windows-latest'
contains 'id: windows-cache-identity'
contains 'Get-Command cl.exe -ErrorAction SilentlyContinue'
contains 'ImageOS'
contains 'rustc -Vv'
contains 'Get-FileHash Cargo.lock -Algorithm SHA256'
contains 'cache_key=$hex'
contains "hashFiles('Cargo.lock', 'rust-toolchain.toml', '**/Cargo.toml')"
contains 'save-if:'
contains 'github.event.pull_request.head.repo.full_name == github.repository'
contains 'cargo test --locked -p cargo-allow'

identity_line=$(grep -n 'id: windows-cache-identity' <<<"$section" | cut -d: -f1)
cache_line=$(grep -n 'Swatinem/rust-cache@' <<<"$section" | cut -d: -f1)
test_line=$(grep -n 'cargo test --locked -p cargo-allow' <<<"$section" | cut -d: -f1)

(( identity_line < cache_line )) || fail "cache runs before identity is resolved"
(( cache_line < test_line )) || fail "tests run before cache restore"

printf 'windows cache contract: ok\n'
