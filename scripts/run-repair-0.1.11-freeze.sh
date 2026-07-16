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

resolve_main_merge() {
  if git merge --no-commit --no-ff origin/main; then
    return 0
  fi

  # Main's #2374 readiness refresh and the release freeze both own this file.
  # Preserve the release-selected capability/version decision, then enrich it
  # through patch_release_oracle below. All other paths must merge normally.
  git checkout --ours docs/release/0.1.11-readiness.md
  git add docs/release/0.1.11-readiness.md

  unresolved="$(git diff --name-only --diff-filter=U)"
  if [[ -n "${unresolved}" ]]; then
    printf 'unresolved merge paths:\n%s\n' "${unresolved}" >&2
    return 1
  fi
}

patch_release_oracle() {
  python3 - <<'PY'
from pathlib import Path

# Promote the release-preparation executable oracle from the completed 0.1.10
# record to the exact 0.1.11 source candidate.
test_path = Path("crates/cargo-allow/src/release_prep_tests.rs")
test = test_path.read_text(encoding="utf-8")
replacements = {
    'const PUBLISHED_RELEASE_VERSION: &str = "0.1.10";':
        'const PUBLISHED_RELEASE_VERSION: &str = "0.1.11";',
    'const PREVIOUS_PUBLISHED_VERSION: &str = "0.1.9";':
        'const PREVIOUS_PUBLISHED_VERSION: &str = "0.1.10";',
    'const PUBLISHED_RELEASE_DOC: &str = "docs/release/0.1.10.md";':
        'const PUBLISHED_RELEASE_DOC: &str = "docs/release/0.1.11.md";',
    'const PREVIOUS_RELEASE_DOC: &str = "docs/release/0.1.9.md";':
        'const PREVIOUS_RELEASE_DOC: &str = "docs/release/0.1.10.md";',
    '    "Public install examples now pin the published `0.1.10` release";':
        '    "Public install examples now pin the published `0.1.11` release";',
}
for old, new in replacements.items():
    if old in test:
        test = test.replace(old, new, 1)
    elif new not in test:
        raise SystemExit(f"release_prep_tests.rs missing expected release constant: {old}")
test_path.write_text(test, encoding="utf-8", newline="\n")

# Keep the release record executable by the release-prep tests: exact version
# identity, public install pin, and the dependency-ordered ten-crate graph.
record_path = Path("docs/release/0.1.11.md")
record = record_path.read_text(encoding="utf-8")
if "## Version and Install Examples" not in record:
    old = '''## Version and package graph

All ten crates use 0.1.11 and publish in dependency order:

```text
allow-core
allow-policy
allow-inventory
allow-files
allow-rust
allow-match
allow-policy-legacy
allow-report
allow-diff
cargo-allow
```

Install after publication:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```
'''
    new = '''## Version and Install Examples

Workspace package versions were bumped to `0.1.11`:

```text
Cargo.toml
Cargo.lock
```

Public install examples now pin the published `0.1.11` release:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```

## Publish Order

Published internal crates in dependency order:

```text
1. allow-core
2. allow-policy
3. allow-inventory
4. allow-files
5. allow-rust
6. allow-match
7. allow-policy-legacy
8. allow-report
9. allow-diff
10. cargo-allow
```
'''
    if record.count(old) != 1:
        raise SystemExit("0.1.11 release record package graph changed unexpectedly")
    record = record.replace(old, new, 1)
record_path.write_text(record, encoding="utf-8", newline="\n")

# The committed step inventory now promotes `why` to the Published 0.1.11
# channel; keep the human guide's checked table on the same stable step ID.
guide_path = Path("docs/getting-started.md")
guide = guide_path.read_text(encoding="utf-8")
old = "| `why_candidate` | Source-candidate `why` (not ordinary on Published 0.1.11) |"
new = "| `why_published` | Published diagnosis with `cargo-allow why` |"
if old in guide:
    guide = guide.replace(old, new, 1)
elif new not in guide:
    raise SystemExit("getting-started checked step table is missing why row")
guide_path.write_text(guide, encoding="utf-8", newline="\n")

# Preserve #2374's installed-binary Stage A+ evidence in the release-selected
# readiness projection.
readiness_path = Path("docs/release/0.1.11-readiness.md")
readiness = readiness_path.read_text(encoding="utf-8")
old = "| Package/install basics | Supported for narrow Linux claim | hosted package-candidate smoke packages all ten crates, checks normalized manifests, installs isolated binary, and uploads a receipt |"
new = "| Package/install basics | Supported for narrow Linux claim | hosted package-candidate smoke packages all ten crates and the path-installed SourceCandidateSmokeReceiptV1 proves the first-hour journey; exact isolated package-set consumption remains a 0.2.0 follow-up |"
if old in readiness:
    readiness = readiness.replace(old, new, 1)
elif new not in readiness:
    raise SystemExit("readiness package/install row changed unexpectedly")
readiness_path.write_text(readiness, encoding="utf-8", newline="\n")
PY
}

{
  echo "qualification_start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "branch_head=${branch_head}"

  # Build the merge candidate directly from the current writer head and current
  # main. Connector-written commits do not always refresh refs/pull/*/merge, so
  # the synthetic PR ref is not an acceptable qualification authority here.
  git fetch origin main
  main_head="$(git rev-parse origin/main)"
  echo "main_head=${main_head}"
  git checkout --detach "${branch_head}"
  resolve_main_merge

  # Remove the one-shot staging wrapper from the qualification tree. ci.yml is
  # restored from current main so the tested tree matches the post-cleanup PR.
  rm -f scripts/run-repair-0.1.11-freeze.sh
  git checkout origin/main -- .github/workflows/ci.yml
  patch_release_oracle

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

  # Recreate the exact qualified merge on the writer branch, remove the staging
  # wrapper, and push an up-to-date mergeable candidate.
  git merge --abort || true
  git checkout "${branch}"
  resolve_main_merge
  patch_release_oracle
  git rm -f scripts/run-repair-0.1.11-freeze.sh
  git checkout origin/main -- .github/workflows/ci.yml
  git add -A
  git commit -m "release: merge current main and finalize cargo-allow 0.1.11"
  git push origin HEAD:"${branch}"
  echo "final_head=$(git rev-parse HEAD)"
} > >(tee "${log}") 2>&1
