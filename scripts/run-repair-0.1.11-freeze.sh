#!/usr/bin/env bash
# One-shot exact merged-tree qualification for PR #2376.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-repair
log="target/release-repair/repair.log"

branch="release/0.1.11"
branch_head="$(git rev-parse HEAD)"

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

{
  echo "qualification_start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "branch_head=${branch_head}"

  # Validate the exact tree GitHub would merge: current release head plus the
  # current main commit (including #2374 SourceCandidateSmoke work).
  git fetch origin refs/pull/2376/merge
  merge_sha="$(git rev-parse FETCH_HEAD)"
  echo "merge_ref_sha=${merge_sha}"
  git checkout --detach "${merge_sha}"

  # Remove the one-shot staging wrapper from the qualification tree. ci.yml is
  # restored from current main so the tested tree matches the post-cleanup PR.
  rm -f scripts/run-repair-0.1.11-freeze.sh
  git checkout origin/main -- .github/workflows/ci.yml

  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --locked
  cargo test --doc --workspace
  cargo doc --workspace --no-deps
  cargo package --workspace --locked
  cargo run -p cargo-allow -- check --mode no-new \
    --format markdown \
    --receipt target/cargo-allow/check.receipt.json \
    --output target/cargo-allow/check.md
  bash scripts/release-version-preflight.sh 0.1.11
  bash scripts/package-candidate-smoke.sh
  bin="target/package-candidate-smoke/install/bin/cargo-allow"
  if [[ -x "${bin}.exe" ]]; then
    bin="${bin}.exe"
  fi
  CARGO_ALLOW_BIN="${bin}" bash scripts/source-candidate-smoke.sh
  bash scripts/shallow-diff-base-smoke.sh
  echo "qualification_result=pass"

  # Return to the writer branch and remove this staging wrapper. The resulting
  # branch tree is the one qualified above; the cleanup commit has no product
  # content beyond deleting this temporary file.
  git checkout "${branch}"
  git rm -f scripts/run-repair-0.1.11-freeze.sh
  git checkout origin/main -- .github/workflows/ci.yml
  git add -A
  git commit -m "chore(release): finalize qualified 0.1.11 source tree"
  git push origin HEAD:"${branch}"
  echo "cleanup_head=$(git rev-parse HEAD)"
} > >(tee "${log}") 2>&1
