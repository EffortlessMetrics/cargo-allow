#!/usr/bin/env bash
# Exact checked adapter for the #3844 review-readiness check.
#
# Derives the live source snapshot for the current pull request event,
# locates the retained review disposition for the exact (repository,
# PR, head) triple under `.allow/review-dispositions/`, and projects
# the structured review state onto the stable `review-readiness` check
# context. The job (and therefore the check) fails on a Failure
# conclusion; Success and Neutral (missing disposition) pass with the
# conclusion recorded in the printed projection.
#
# Disposition digest recipe (binds the retained record to the exact
# pair): `git diff <merge_base>..<head_sha>` hashed with sha256sum.
# The review flow that authors dispositions must use the same recipe.
#
# Read-only: this script never mutates PR state, never merges, and
# never touches branch rules, releases, or tags.

set -euo pipefail

: "${PR_NUMBER:?PR_NUMBER is required}"
: "${HEAD_SHA:?HEAD_SHA is required}"
: "${EVENT_NAME:?EVENT_NAME is required}"
: "${GH_TOKEN:?GH_TOKEN is required}"
: "${CF_PR_BASE_REF:-}"

case "${EVENT_NAME}" in
opened | reopened | synchronize | ready_for_review | converted_to_draft) ;;
*)
  echo "review-readiness: unmapped event '${EVENT_NAME}'" >&2
  exit 1
  ;;
esac

BASE_REF="${CF_PR_BASE_REF:-$(gh pr view "$PR_NUMBER" --json baseRefName --jq .baseRefName)}"
DRAFT_STATE="ready"
if [ "$(gh pr view "$PR_NUMBER" --json isDraft --jq .isDraft)" = "true" ]; then
  DRAFT_STATE="draft"
fi
BASE_SHA="$(gh pr view "$PR_NUMBER" --json baseRefOid --jq .baseRefOid)"
HEAD_REF="$(gh pr view "$PR_NUMBER" --json headRefName --jq .headRefName)"
REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
MERGE_BASE="$(git merge-base "origin/${BASE_REF}" "${HEAD_SHA}")"
DIFF_DIGEST="$(git diff "${MERGE_BASE}..${HEAD_SHA}" | sha256sum | cut -d' ' -f1)"

LIVE_JSON="$(mktemp)"
trap 'rm -f "${LIVE_JSON}"' EXIT
cat >"${LIVE_JSON}" <<EOF
{
  "repository": "${REPOSITORY}",
  "pr_number": ${PR_NUMBER},
  "base_ref": "${BASE_REF}",
  "base_sha": "${BASE_SHA}",
  "head_ref": "${HEAD_REF}",
  "head_sha": "${HEAD_SHA}",
  "merge_base": "${MERGE_BASE}",
  "diff_digest": "sha256:v1:${DIFF_DIGEST}",
  "review_protocol": "review-current-head-gen1",
  "scope_claim_boundary": "pull-request:${PR_NUMBER}"
}
EOF

# Locate the retained disposition for the exact (repository, PR, head)
# triple. No match is the explicit missing-disposition case.
DISPOSITION_ARGS=()
if [ -d ".allow/review-dispositions" ]; then
  for candidate in .allow/review-dispositions/*.json; do
    [ -f "$candidate" ] || continue
    if jq -e --arg repository "${REPOSITORY}" \
      --argjson pr "${PR_NUMBER}" \
      --arg head "${HEAD_SHA}" \
      'select(.repository == $repository and .pr_number == $pr and .head_sha == $head)' \
      "$candidate" >/dev/null 2>&1; then
      echo "review-readiness: retained disposition ${candidate}"
      DISPOSITION_ARGS=(--disposition "$candidate")
      break
    fi
  done
fi
if [ "${#DISPOSITION_ARGS[@]}" -eq 0 ]; then
  echo "review-readiness: no retained disposition for ${REPOSITORY}#${PR_NUMBER}@${HEAD_SHA}"
fi

cargo run -p cargo-allow --locked -- review-readiness project \
  --live "${LIVE_JSON}" \
  --draft-state "${DRAFT_STATE}" \
  --event "${EVENT_NAME}" \
  ${DISPOSITION_ARGS[@]+"${DISPOSITION_ARGS[@]}"}
