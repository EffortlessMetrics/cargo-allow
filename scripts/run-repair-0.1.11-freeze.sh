#!/usr/bin/env bash
# One-shot merge of current main (#2379) into the exact 0.1.11 candidate.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
mkdir -p target/release-repair
log="target/release-repair/repair.log"

resolve_merge() {
  if git merge --no-commit --no-ff origin/main; then
    return 0
  fi

  # Both the release freeze and post-freeze package proof own these projections.
  # Keep the selected release versions, then add the new ExactCandidate evidence
  # explicitly below. Every other conflict remains a hard failure.
  for path in CHANGELOG.md docs/release/0.1.11-readiness.md; do
    if git diff --name-only --diff-filter=U | grep -Fxq "${path}"; then
      git checkout --ours "${path}"
      git add "${path}"
    fi
  done

  unresolved="$(git diff --name-only --diff-filter=U)"
  if [[ -n "${unresolved}" ]]; then
    printf 'unresolved merge paths:\n%s\n' "${unresolved}" >&2
    return 1
  fi
}

patch_release_projection() {
  python3 - <<'PY'
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    if old in text:
        text = text.replace(old, new, 1)
    elif new not in text:
        raise SystemExit(f"{path}: expected release projection text not found")
    target.write_text(text, encoding="utf-8", newline="\n")


changelog_path = Path("CHANGELOG.md")
changelog = changelog_path.read_text(encoding="utf-8")
exact_bullet = '''- `scripts/exact-candidate-package-set.sh` emits
  `cargo-allow.exact-candidate-package-set.v1` after packaging the shared
  ten-crate set, installing from extracted packages with `[patch.crates-io]`,
  and running omit-crate / workspace-path negatives (#2372 / #2277 Stage A;
  full local-registry index and remaining negatives deferred).
'''
if exact_bullet not in changelog:
    anchor = "## [0.1.11] - 2026-07-16\n\n### Added\n\n"
    if changelog.count(anchor) != 1:
        raise SystemExit("could not locate 0.1.11 Added section")
    changelog = changelog.replace(anchor, anchor + exact_bullet, 1)
changelog = changelog.replace(
    "  temporary consumer repo (#2278 Stage A+; ExactCandidatePackageSet isolation\n  remains #2277).- Hosted `shallow-diff-smoke` CI job",
    "  temporary consumer repo (#2278 Stage A+). The hosted release lane runs it\n  against the ExactCandidatePackageSet install when available.\n- Hosted `shallow-diff-smoke` CI job",
)
changelog_path.write_text(changelog, encoding="utf-8", newline="\n")

replace_once(
    "docs/release/0.1.11-readiness.md",
    "| Package/install basics | Supported for narrow Linux claim | hosted package-candidate smoke packages all ten crates and the path-installed SourceCandidateSmokeReceiptV1 proves the first-hour journey; exact isolated package-set consumption remains a 0.2.0 follow-up |",
    "| Package/install basics | Supported for narrow Linux claim | hosted ExactCandidatePackageSetV1 packages and patch-isolates all ten crates, then SourceCandidateSmokeReceiptV1 runs the installed first-hour journey; full local-registry index and remaining package-set negatives are explicit limitations |",
)

record_path = Path("docs/release/0.1.11.md")
record = record_path.read_text(encoding="utf-8")
needle = "- dedicated Rust 1.85 CI plus hosted Linux package/install candidate smoke.\n"
addition = (
    "- dedicated Rust 1.85 CI plus hosted Linux package/install candidate smoke;\n"
    "- `ExactCandidatePackageSetV1` installs from extracted ten-crate packages via\n"
    "  `[patch.crates-io]`, followed by the installed first-hour journey.\n"
)
if addition not in record:
    if record.count(needle) != 1:
        raise SystemExit("0.1.11 release highlights changed unexpectedly")
    record = record.replace(needle, addition, 1)
known = "- no signed registry manifest for the ten-crate set.\n"
known_new = (
    "- ExactCandidate proof uses extracted packages plus `[patch.crates-io]`; a\n"
    "  full local-registry index and remaining negative controls are deferred;\n"
    "- no signed registry manifest for the ten-crate set.\n"
)
if known_new not in record:
    if record.count(known) != 1:
        raise SystemExit("0.1.11 known limitations changed unexpectedly")
    record = record.replace(known, known_new, 1)
record_path.write_text(record, encoding="utf-8", newline="\n")

notes_path = Path("docs/release/github/v0.1.11.md")
notes = notes_path.read_text(encoding="utf-8")
needle = "- Proves the 0.1.x Rust 1.85 claim in CI and packages/installs the candidate in a\n  hosted Linux smoke job.\n"
addition = (
    "- Proves the 0.1.x Rust 1.85 claim in CI and packages/installs the candidate in a\n"
    "  hosted Linux smoke job.\n"
    "- Adds `ExactCandidatePackageSetV1`: extracted ten-crate package installation\n"
    "  with patch isolation, checksums, and negative controls before first-hour smoke.\n"
)
if addition not in notes:
    if notes.count(needle) != 1:
        raise SystemExit("GitHub release highlights changed unexpectedly")
    notes = notes.replace(needle, addition, 1)
notes_path.write_text(notes, encoding="utf-8", newline="\n")
PY
}

{
  set -x
  git config user.name "github-actions[bot]"
  git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
  git fetch origin main
  resolve_merge
  patch_release_projection
  git rm -f scripts/run-repair-0.1.11-freeze.sh
  git add -A
  git commit -m "release: merge exact package-set proof into 0.1.11 candidate"
  candidate_head="$(git rev-parse HEAD)"
  echo "candidate_head=${candidate_head}"

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
  mkdir -p target/exact-candidate-package-set/packages
  cp target/package-candidate-smoke/packages/*.crate \
    target/exact-candidate-package-set/packages/
  SKIP_PACKAGE=1 bash scripts/exact-candidate-package-set.sh
  bin="target/exact-candidate-package-set/install/bin/cargo-allow"
  if [[ ! -x "${bin}" && -x "${bin}.exe" ]]; then
    bin="${bin}.exe"
  fi
  CARGO_ALLOW_BIN="${bin}" bash scripts/source-candidate-smoke.sh
  bash scripts/shallow-diff-base-smoke.sh

  echo "qualification_result=pass"
  git push origin HEAD:release/0.1.11
} > >(tee "${log}") 2>&1
