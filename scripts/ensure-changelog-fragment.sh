#!/usr/bin/env bash
# Pre-commit check: require a staged Changie fragment for selected staged paths.
#
# This is a deliberately bounded convention check. It does not decide whether
# a change is user-facing and it does not validate fragment contents. It only
# proves that one exact Git-index candidate containing a selected repository
# surface also contains a staged root-level .changes/*.yaml fragment.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT_PATH="${SCRIPT_DIR}/$(basename "${BASH_SOURCE[0]}")"

run_expected() {
  local expected="$1"
  local label="$2"
  shift 2

  local output status
  set +e
  output="$("$@" 2>&1)"
  status=$?
  set -e

  if [[ "${status}" -ne "${expected}" ]]; then
    printf 'self-test %s: expected exit %s, got %s\n%s\n' \
      "${label}" "${expected}" "${status}" "${output}" >&2
    return 1
  fi

  SELF_TEST_OUTPUT="${output}"
}

run_self_test() {
  local tmp repo hook bad_index orphan_hook
  tmp="$(mktemp -d)"
  SELF_TEST_TMP="${tmp}"
  trap 'rm -rf "${SELF_TEST_TMP}"' EXIT
  repo="${tmp}/repo"

  git init -q "${repo}"
  git -C "${repo}" config user.name 'cargo-allow hook test'
  git -C "${repo}" config user.email 'hook-test@example.invalid'
  git -C "${repo}" config commit.gpgsign false

  mkdir -p \
    "${repo}/crates/demo/src" \
    "${repo}/crates/space dir/src" \
    "${repo}/.changes"
  printf '# hook fixture\n' > "${repo}/README.md"
  printf 'pub fn demo() {}\n' > "${repo}/crates/demo/src/lib.rs"
  printf 'pub fn spaced() {}\n' > "${repo}/crates/space dir/src/lib.rs"
  cat > "${repo}/.changes/Old.yaml" <<'FIXTURE'
kind: Fixed
body: Existing committed fragment.
FIXTURE

  git -C "${repo}" add -A
  git -C "${repo}" commit -qm 'initial fixture'

  hook="${repo}/.git/hooks/pre-commit"
  cp "${SCRIPT_PATH}" "${hook}"
  chmod +x "${hook}"

  reset_fixture() {
    git -C "${repo}" reset --hard -q HEAD
    git -C "${repo}" clean -fdq
  }

  printf 'irrelevant\n' >> "${repo}/README.md"
  git -C "${repo}" add README.md
  run_expected 0 irrelevant-path "${hook}"
  [[ -z "${SELF_TEST_OUTPUT}" ]] || {
    printf 'self-test irrelevant-path: expected no output, got %s\n' \
      "${SELF_TEST_OUTPUT}" >&2
    return 1
  }

  reset_fixture
  printf '// user-visible change\n' >> "${repo}/crates/demo/src/lib.rs"
  git -C "${repo}" add crates/demo/src/lib.rs
  run_expected 1 pre-existing-fragment "${hook}"
  grep -Fq 'no staged Changie fragment' <<<"${SELF_TEST_OUTPUT}"

  cat > "${repo}/.changes/Unstaged.yaml" <<'FIXTURE'
kind: Fixed
body: Unstaged fixture fragment.
FIXTURE
  run_expected 1 unstaged-fragment "${hook}"
  mkdir -p "${repo}/.changes/nested"
  cat > "${repo}/.changes/nested/Fixed.yaml" <<'FIXTURE'
kind: Fixed
body: Nested staged fixture fragment.
FIXTURE
  git -C "${repo}" add .changes/nested/Fixed.yaml
  run_expected 1 nested-fragment "${hook}"

  cat > "${repo}/.changes/Fixed-new.yaml" <<'FIXTURE'
kind: Fixed
body: Staged fixture fragment.
FIXTURE
  git -C "${repo}" add .changes/Fixed-new.yaml
  run_expected 0 staged-fragment "${hook}"

  reset_fixture
  git -C "${repo}" rm -q .changes/Old.yaml
  run_expected 0 staged-deletion-only "${hook}"

  reset_fixture
  printf '// spaced change\n' >> "${repo}/crates/space dir/src/lib.rs"
  git -C "${repo}" add 'crates/space dir/src/lib.rs'
  run_expected 1 spaced-path-without-fragment "${hook}"
  cat > "${repo}/.changes/Fixed with spaces.yaml" <<'FIXTURE'
kind: Fixed
body: Staged fragment with spaces in its path.
FIXTURE
  git -C "${repo}" add '.changes/Fixed with spaces.yaml'
  run_expected 0 spaced-path-with-fragment "${hook}"

  pushd "${tmp}" >/dev/null
  run_expected 0 invoked-outside-root "${hook}"
  popd >/dev/null

  orphan_hook="${tmp}/orphan-pre-commit"
  cp "${SCRIPT_PATH}" "${orphan_hook}"
  chmod +x "${orphan_hook}"
  pushd "${repo}" >/dev/null
  run_expected 1 orphan-hook-refuses-pwd "${orphan_hook}"
  popd >/dev/null
  grep -Fq 'unable to resolve the Git worktree' <<<"${SELF_TEST_OUTPUT}"

  reset_fixture
  printf '// staged before index failure\n' >> "${repo}/crates/demo/src/lib.rs"
  git -C "${repo}" add crates/demo/src/lib.rs
  bad_index="${tmp}/bad-index"
  printf 'not-a-git-index\n' > "${bad_index}"
  run_expected 1 failed-index env GIT_INDEX_FILE="${bad_index}" "${hook}"
  grep -Fq 'unable to inspect the staged candidate' <<<"${SELF_TEST_OUTPUT}"

  printf 'ensure-changelog-fragment self-test: passed\n'
}

if [[ "${1:-}" == "--self-test" ]]; then
  run_self_test
  exit 0
fi

resolve_root() {
  local candidate root
  for candidate in "${SCRIPT_DIR}/.." "${SCRIPT_DIR}/../.."; do
    if root="$(git -C "${candidate}" rev-parse --show-toplevel 2>/dev/null)"; then
      printf '%s\n' "${root}"
      return 0
    fi
  done
  return 1
}

if ! ROOT="$(resolve_root)"; then
  echo 'error: unable to resolve the Git worktree for the changelog-fragment hook' >&2
  exit 1
fi
cd "${ROOT}"

staged_paths="$(mktemp)"
trap 'rm -f "${staged_paths}"' EXIT
if ! git diff --cached --name-only --diff-filter=ACMR -z -- > "${staged_paths}"; then
  echo 'error: unable to inspect the staged candidate for changelog-fragment coverage' >&2
  exit 1
fi

relevant_staged=0
fragment_staged=0
while IFS= read -r -d '' path; do
  case "${path}" in
    crates/*|scripts/*|.github/*)
      relevant_staged=1
      ;;
  esac

  case "${path}" in
    .changes/*.yaml)
      fragment_name="${path#.changes/}"
      if [[ "${fragment_name}" != */* ]]; then
        fragment_staged=1
      fi
      ;;
  esac
done < "${staged_paths}"

if [[ "${relevant_staged}" -eq 0 || "${fragment_staged}" -eq 1 ]]; then
  exit 0
fi

cat <<'HINT' >&2
error: no staged Changie fragment found for the selected staged candidate

The Git index contains an added, copied, modified, or renamed path under
crates/, scripts/, or .github/, but it does not contain an added, copied,
modified, or renamed root-level .changes/*.yaml fragment. Pre-existing,
unstaged, deleted, nested, Markdown, and version-marker files do not satisfy
this per-commit convention.

For a user-facing change, create and stage a fragment:
  changie new
  git add .changes/<fragment>.yaml

Then validate all fragments without mutating the repository:
  changie batch <next-version> --dry-run

For an intentionally non-user-facing change, bypass this optional hook with:
  git commit --no-verify
HINT

exit 1
