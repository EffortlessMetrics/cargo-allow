#!/usr/bin/env bash
# Exact checked adapter for the #3844 review-readiness check.
#
# Derives the live source snapshot for the current pull request event,
# locates the retained review disposition for the exact (repository,
# PR, head) triple under `.allow/review-dispositions/`, projects the
# structured review state onto the stable `review-readiness` check
# context, and PUBLISHES the result as a GitHub check run at the PR
# head so the typed conclusion (success / neutral / failure) is
# visible at the check boundary — a missing disposition publishes
# `neutral`, never a green check.
#
# On pushes to the base branch (no pull_request event fires), the
# adapter iterates open pull requests and republishes each readiness
# result bound to the new merge base, so base movement cannot leave a
# stale green behind.
#
# Disposition digest recipe (binds the retained record to the exact
# pair): `git diff <merge_base>..<head_sha>` hashed with sha256sum.
# The review flow that authors dispositions must use the same recipe.
#
# Read-only over review semantics: the only GitHub write is the
# review-readiness check run itself. No merge, no PR mutation, no
# branch-rule or release change.

set -euo pipefail

: "${PR_HEAD_SHA:?PR_HEAD_SHA is required}"
: "${GH_TOKEN:?GH_TOKEN is required}"
API="repos/${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
CHECK_NAME="review-readiness"
LEDGER_DIR=".allow/review-dispositions"

publish() {
  # publish <head_sha> <conclusion> <summary>
  local head_sha="$1" conclusion="$2" summary="$3"
  gh api "${API}/check-runs" -X POST \
    -f "name=${CHECK_NAME}" \
    -f "head_sha=${head_sha}" \
    -f "status=completed" \
    -f "conclusion=${conclusion}" \
    -f "output[title]=review-readiness: ${conclusion}" \
    -f "output[summary]=${summary}" >/dev/null
  printf 'review-readiness: published %s at %s\n' "${conclusion}" "${head_sha:0:12}"
}

# project <pr_number> <event> -> publishes the check run for the PR.
# Returns nonzero on adapter breakage or a Failure conclusion; the
# caller aggregates.
project_pr() {
  local pr_number="$1" event="$2"
  local pr_json base_ref base_sha head_ref head_sha draft_state merge_base diff_digest
  pr_json="$(gh pr view "$pr_number" --json baseRefName,baseRefOid,headRefName,headRefOid,isDraft)"
  base_ref="$(jq -r .baseRefName <<<"$pr_json")"
  base_sha="$(jq -r .baseRefOid <<<"$pr_json")"
  head_ref="$(jq -r .headRefName <<<"$pr_json")"
  head_sha="$(jq -r .headRefOid <<<"$pr_json")"
  if [ "$(jq -r .isDraft <<<"$pr_json")" = "true" ]; then
    draft_state="draft"
  else
    draft_state="ready"
  fi
  merge_base="$(git merge-base "origin/${base_ref}" "${head_sha}")"
  diff_digest="$(git diff "${merge_base}..${head_sha}" | sha256sum | cut -d' ' -f1)"

  local live_file
  live_file="$(mktemp)"
  jq -n \
    --arg repository "${GITHUB_REPOSITORY}" \
    --argjson pr_number "${pr_number}" \
    --arg base_ref "${base_ref}" \
    --arg base_sha "${base_sha}" \
    --arg head_ref "${head_ref}" \
    --arg head_sha "${head_sha}" \
    --arg merge_base "${merge_base}" \
    --arg diff_digest "sha256:v1:${diff_digest}" \
    '{repository: $repository, pr_number: $pr_number,
      base_ref: $base_ref, base_sha: $base_sha,
      head_ref: $head_ref, head_sha: $head_sha,
      merge_base: $merge_base, diff_digest: $diff_digest,
      review_protocol: "review-current-head-gen1",
      scope_claim_boundary: ("pull-request:" + ($pr_number | tostring))}' >"${live_file}"

  # Locate the retained disposition for the exact (repository, PR,
  # head) triple. Ambiguity fails closed; no match is the explicit
  # missing-disposition case.
  local matches=() candidate
  if [ -d "${LEDGER_DIR}" ]; then
    for candidate in "${LEDGER_DIR}"/*.json; do
      [ -f "$candidate" ] || continue
      if jq -e --arg repository "${GITHUB_REPOSITORY}" \
        --argjson pr "${pr_number}" \
        --arg head "${head_sha}" \
        'select(.repository == $repository and .pr_number == $pr and .head_sha == $head)' \
        "$candidate" >/dev/null 2>&1; then
        matches+=("$candidate")
      fi
    done
  fi

  local disposition_args=() delta_args=()
  if [ "${#matches[@]}" -gt 1 ]; then
    publish "${head_sha}" "failure" \
      "ambiguous retained dispositions for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}: ${matches[*]}; disposition discovery fails closed"
    rm -f "${live_file}"
    return 1
  elif [ "${#matches[@]}" -eq 1 ]; then
    echo "review-readiness: retained disposition ${matches[0]}"
    disposition_args=(--disposition "${matches[0]}")
    local bound_head
    bound_head="$(jq -r '.head_sha // ""' "${matches[0]}")"
    if [ -n "${bound_head}" ] && [ "${bound_head}" != "${head_sha}" ]; then
      # The retained-review-ledger bootstrap: pass the delta paths so
      # the projection can prove the head movement is disposition
      # records only.
      local delta
      while IFS= read -r delta; do
        [ -n "$delta" ] && delta_args+=(--head-delta-path "$delta")
      done < <(git diff --name-only "${bound_head}..${head_sha}")
    fi
  else
    echo "review-readiness: no retained disposition for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}"
  fi

  local projection_file exit_code=0
  projection_file="$(mktemp)"
  cargo run -p cargo-allow --locked -- review-readiness project \
    --live "${live_file}" \
    --draft-state "${draft_state}" \
    --event "${event}" \
    ${disposition_args[@]+"${disposition_args[@]}"} \
    ${delta_args[@]+"${delta_args[@]}"} \
    --format json >"${projection_file}" || exit_code=$?
  rm -f "${live_file}"

  local conclusion summary
  conclusion="$(jq -r '.conclusion' "${projection_file}")"
  summary="$(jq -r '[.conclusion_reasons[]] | join("; ")' "${projection_file}")"
  publish "${head_sha}" "${conclusion}" "${summary}"
  rm -f "${projection_file}"
  [ "${exit_code}" -eq 0 ] || return 1
  [ "${conclusion}" != "failure" ] || return 1
}

overall=0
if [ "${GITHUB_EVENT_NAME}" = "push" ]; then
  # Base-branch movement: no pull_request event fires, so recompute
  # every open pull request against the new base and republish.
  while IFS= read -r open_pr; do
    [ -n "$open_pr" ] || continue
    echo "review-readiness: base movement recompute for PR #${open_pr}"
    project_pr "$open_pr" "base_moved" || overall=1
  done < <(gh pr list --state open --json number --jq '.[].number')
else
  : "${PR_NUMBER:?PR_NUMBER is required}"
  : "${PR_EVENT_ACTION:?PR_EVENT_ACTION is required}"
  case "${PR_EVENT_ACTION}" in
    opened | reopened | synchronize | ready_for_review | converted_to_draft) ;;
    *)
      echo "review-readiness: unmapped event '${PR_EVENT_ACTION}'" >&2
      exit 1
      ;;
  esac
  project_pr "$PR_NUMBER" "$PR_EVENT_ACTION" || overall=1
fi
exit "$overall"
