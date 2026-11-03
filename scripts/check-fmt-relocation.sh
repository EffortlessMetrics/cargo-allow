#!/usr/bin/env bash
# #4222: prove the repository-owned rustfmt.toml terminates ancestor config
# discovery, so formatting evidence is independent of the checkout location.
#
# Demonstrates both halves with a real toolchain:
#   hazard    — a copy of the tree WITHOUT the repo config, placed under a
#               poisoned parent rustfmt.toml (max_width = 100), fails
#               `cargo fmt --all -- --check`.
#   protection— the identical copy WITH the committed empty repo config
#               passes under the same poisoned parent.
# No files outside the temp directory are modified; the checkout itself is
# never formatted.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

poisoned="${work}/poisoned-parent"
mkdir -p "${poisoned}"
printf 'max_width = 100\n' >"${poisoned}/rustfmt.toml"

hazard_copy="${poisoned}/hazard"
protected_copy="${poisoned}/protected"
mkdir -p "${hazard_copy}" "${protected_copy}"
git -C "${ROOT}" archive HEAD | tar -x -C "${hazard_copy}"
git -C "${ROOT}" archive HEAD | tar -x -C "${protected_copy}"

if [ ! -f "${protected_copy}/rustfmt.toml" ]; then
  echo 'fmt-relocation: the committed rustfmt.toml is missing from the archive' >&2
  exit 1
fi

if (cd "${hazard_copy}" && cargo fmt --all -- --check >/dev/null 2>&1); then
  echo 'fmt-relocation: hazard not demonstrated — the poisoned ancestor config did not change the unpinned check' >&2
  exit 1
fi
echo 'fmt-relocation: hazard demonstrated (unpinned copy fails under the poisoned ancestor)'

if (cd "${protected_copy}" && cargo fmt --all -- --check >/dev/null 2>&1); then
  echo 'fmt-relocation: protected copy passes under the poisoned ancestor; repo config terminates discovery'
else
  echo 'fmt-relocation: PROTECTION FAILED — the committed config did not stop ancestor discovery' >&2
  exit 1
fi
