#!/usr/bin/env bash
# Exact checked adapter for the #3844 review-readiness check.
#
# Derives the live source snapshot for the current pull request event,
# locates the retained review disposition for the exact (repository,
# PR, head) triple under `.allow/review-dispositions/` (falling back to
# one unique ancestor-bound record whose remaining head delta is
# proven review-disposition records), projects the structured review
# state onto the stable `review-readiness` check context, and
# publishes the result as ONE authoritative GitHub check run at the PR
# head — updated in place when it already exists — so the typed
# conclusion (success / neutral / failure) is visible at the check
# boundary and a missing disposition publishes `neutral`, never a
# green check.
#
# On pushes to the base branch (no pull_request event fires), the
# adapter iterates ALL open pull requests and republishes each
# readiness result bound to the new merge base; base changes through a
# PR edit are recomputed through the `edited` event with the base
# change flag. Neither path can leave a stale green behind.
#
# Disposition digest recipe (binds the retained record to the exact
# pair): `git diff <merge_base>..<head_sha>` hashed with sha256sum.
# The review flow that authors dispositions must use the same recipe.
#
# Fork PRs receive a read-only token: when the check-run publish is
# refused, the adapter logs the refusal and exits nonzero so the job
# status itself carries the conclusion instead.
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
  # publish <head_sha> <conclusion> <summary> <base_sha> <merge_base> <diff_digest>
  # Update the authoritative run for this head when one exists; create
  # it otherwise. Pair identity is part of the published output so a
  # stale result can never masquerade as current.
  local head_sha="$1" conclusion="$2" summary="$3" base_sha="$4" merge_base="$5" diff_digest="$6"
  local output
  output="$(printf 'pair: base=%s merge-base=%s diff=sha256:v1:%s\n\n%s' \
    "${base_sha}" "${merge_base}" "${diff_digest}" "${summary}")"
  local existing_id
  existing_id="$(gh api "${API}/commits/${head_sha}/check-runs?check_name=${CHECK_NAME}" \
    --jq '.check_runs[] | select(.name == env.CHECK_NAME) | .id' 2>/dev/null | head -n 1 || true)"
  if [ -n "${existing_id}" ]; then
    gh api "${API}/check-runs/${existing_id}" -X PATCH \
      -f "status=completed" \
      -f "conclusion=${conclusion}" \
      -f "output[title]=review-readiness: ${conclusion}" \
      -f "output[summary]=${output}" >/dev/null
    printf 'review-readiness: updated run %s -> %s at %s\n' \
      "${existing_id}" "${conclusion}" "${head_sha:0:12}"
  else
    gh api "${API}/check-runs" -X POST \
      -f "name=${CHECK_NAME}" \
      -f "head_sha=${head_sha}" \
      -f "status=completed" \
      -f "conclusion=${conclusion}" \
      -f "output[title]=review-readiness: ${conclusion}" \
      -f "output[summary]=${output}" >/dev/null
    printf 'review-readiness: published %s at %s\n' "${conclusion}" "${head_sha:0:12}"
  fi
}

# project <pr_number> <event> -> publishes the check run for the PR.
# Returns nonzero on adapter breakage, a Failure conclusion, or a
# refused publish (fork token downgrade); the caller aggregates.
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

  # Disposition discovery: one exact-head record first; then one
  # unique ancestor-bound record (the retained-review-ledger
  # bootstrap). Ambiguity fails closed; no match is the explicit
  # missing-disposition case.
  local exact_matches=() ancestor_matches=() candidate bound_head
  if [ -d "${LEDGER_DIR}" ]; then
    for candidate in "${LEDGER_DIR}"/*.json; do
      [ -f "$candidate" ] || continue
      bound_head="$(jq -r --arg repository "${GITHUB_REPOSITORY}" \
        --argjson pr "${pr_number}" \
        'select(.repository == $repository and .pr_number == $pr) | .head_sha // ""' \
        "$candidate" 2>/dev/null || true)"
      [ -n "${bound_head}" ] || continue
      if [ "${bound_head}" = "${head_sha}" ]; then
        exact_matches+=("$candidate")
      elif git merge-base --is-ancestor "${bound_head}" "${head_sha}" 2>/dev/null; then
        ancestor_matches+=("$candidate")
      fi
    done
  fi

  local disposition_args=() delta_args=() selected=""
  if [ "${#exact_matches[@]}" -gt 1 ] || [ "${#ancestor_matches[@]}" -gt 1 ]; then
    publish "${head_sha}" "failure" \
      "ambiguous retained dispositions for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}: exact=${exact_matches[*]:-} ancestor=${ancestor_matches[*]:-}; disposition discovery fails closed" \
      "${base_sha}" "${merge_base}" "${diff_digest}" || return 1
    rm -f "${live_file}"
    return 1
  elif [ "${#exact_matches[@]}" -eq 1 ]; then
    selected="${exact_matches[0]}"
  elif [ "${#ancestor_matches[@]}" -eq 1 ]; then
    selected="${ancestor_matches[0]}"
  fi

  if [ -n "${selected}" ]; then
    echo "review-readiness: retained disposition ${selected}"
    disposition_args=(--disposition "${selected}")
    bound_head="$(jq -r '.head_sha // ""' "${selected}")"
    if [ -n "${bound_head}" ] && [ "${bound_head}" != "${head_sha}" ]; then
      # The retained-review-ledger bootstrap: pass the complete delta
      # so the projection can prove the head movement is disposition
      # records only and reject anything else.
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
  if ! publish "${head_sha}" "${conclusion}" "${summary}" \
    "${base_sha}" "${merge_base}" "${diff_digest}"; then
    echo "review-readiness: check-run publish refused (fork token downgrade?); the job status carries the conclusion" >&2
    rm -f "${projection_file}"
    return 1
  fi
  rm -f "${projection_file}"
  [ "${exit_code}" -eq 0 ] || return 1
  [ "${conclusion}" != "failure" ] || return 1
}

overall=0
if [ "${GITHUB_EVENT_NAME}" = "push" ]; then
  # Base-branch movement: no pull_request event fires, so recompute
  # every open pull request against the new base and republish. The
  # enumeration is captured (not process-substituted) so a failed
  # gh pr list fails the run instead of looking like an empty list,
  # and the limit is raised past the 30-PR default page.
  open_prs="$(gh pr list --state open --limit 1000 --json number --jq '.[].number')"
  while IFS= read -r open_pr; do
    [ -n "$open_pr" ] || continue
    echo "review-readiness: base movement recompute for PR #${open_pr}"
    project_pr "$open_pr" "base_moved" || overall=1
  done <<<"$open_prs"
else
  : "${PR_NUMBER:?PR_NUMBER is required}"
  : "${PR_EVENT_ACTION:?PR_EVENT_ACTION is required}"
  event="${PR_EVENT_ACTION}"
  if [ "${PR_EVENT_ACTION}" = "edited" ]; then
    # Only base-changing edits are readiness-relevant; title and body
    # edits cannot move the reviewed pair.
    if [ "${PR_BASE_CHANGED:-false}" != "true" ]; then
      echo "review-readiness: edit without a base change is not readiness-relevant"
      exit 0
    fi
    event="base_moved"
  fi
  case "${event}" in
    opened | reopened | synchronize | ready_for_review | converted_to_draft | base_moved) ;;
    *)
      echo "review-readiness: unmapped event '${event}'" >&2
      exit 1
      ;;
  esac
  project_pr "$PR_NUMBER" "$event" || overall=1
fi
exit "$overall"
