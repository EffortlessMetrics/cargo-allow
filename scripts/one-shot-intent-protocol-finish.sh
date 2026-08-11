#!/usr/bin/env bash
set -euo pipefail

branch="refactor/intent-protocol-canonical-repo-3387"

# Pull-request workflows check out a synthetic merge ref. Move onto the real
# branch before generating or committing repository artifacts.
git fetch origin "${branch}" main
git checkout -B one-shot-intent-protocol "origin/${branch}"

cargo fmt --all
cargo metadata --format-version 1 >/dev/null

# The preflight is branch-local automation, not product source. Restore the
# permanent guard and remove this script before creating the reviewed head.
git checkout origin/main -- scripts/check-msrv-consistency.sh
rm -f scripts/one-shot-intent-protocol-finish.sh

git add Cargo.lock crates/intent-protocol scripts/check-msrv-consistency.sh scripts/one-shot-intent-protocol-finish.sh
git diff --cached --check

git config user.name "EffortlessSteven"
git config user.email "git@effortlesssteven.com"

git commit -m "refactor(intent-protocol): consume canonical repository contracts"
git push origin HEAD:"${branch}"
