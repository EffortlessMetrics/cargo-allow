#!/usr/bin/env bash
# #4222: formatting evidence must be independent of the checkout location.
#
# Measured law (this fixture proves it with a real toolchain): rustfmt
# uses the NEAREST rustfmt.toml/.rustfmt.toml found walking upward from
# the invocation directory and stops there; with no config found it
# keeps walking and inherits any ancestor's config. fn_params_layout =
# "Vertical" in the ancestor visibly reshapes already-formatted code
# without touching files outside the temp directory.
#
# Three copies of the tree under that poisoned ancestor:
#   unpinned    — config removed (older clone / relocated tree) → fails (hazard)
#   repoconfig  — committed config present, plain invocation  → passes (shield)
#   pinned      — committed config + explicit --config-path   → passes (pin)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

poisoned="${work}/poisoned-parent"
mkdir -p "${poisoned}"
printf 'use_small_heuristics = "Max"\n' >"${poisoned}/rustfmt.toml"

declare -A copies
for name in unpinned repoconfig pinned; do
  copies["${name}"]="${poisoned}/${name}"
  mkdir -p "${copies[${name}]}"
  git -C "${ROOT}" archive HEAD | tar -x -C "${copies[${name}]}"
  if [ "${name}" = unpinned ]; then
    # Represent an older clone or a relocated tree that predates the
    # committed config: no repository config in the copy.
    rm -f "${copies[${name}]}/rustfmt.toml"
  elif [ ! -f "${copies[${name}]}/rustfmt.toml" ]; then
    echo "fmt-relocation: the committed rustfmt.toml is missing from the ${name} copy" >&2
    exit 1
  fi
done

check() {
  local copy="$1" pin="${2:-}"
  if [ "${pin}" = pinned ]; then
    (cd "${copy}" && cargo fmt --all --check -- --config-path "${copy}/rustfmt.toml" >/dev/null 2>&1)
  else
    (cd "${copy}" && cargo fmt --all --check >/dev/null 2>&1)
  fi
}

if check "${copies[unpinned]}"; then
  echo 'fmt-relocation: hazard not demonstrated — the poisoned ancestor did not change the unpinned copy' >&2
  exit 1
fi
echo 'fmt-relocation: hazard demonstrated (unpinned copy fails under the poisoned ancestor)'

if ! check "${copies[repoconfig]}"; then
  echo 'fmt-relocation: PROTECTION FAILED — the committed config did not terminate ancestor discovery' >&2
  exit 1
fi
echo 'fmt-relocation: committed config shields the tree (nearest config wins; poison ignored)'

if ! check "${copies[pinned]}" pinned; then
  echo 'fmt-relocation: PROTECTION FAILED — the --config-path pin did not stop the ancestor' >&2
  exit 1
fi
echo 'fmt-relocation: explicit --config-path pin passes identically'
