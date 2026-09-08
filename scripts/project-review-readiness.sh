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
  # These functions run in conditional contexts, where Bash disables
  # errexit. Check every API status explicitly, and keep raw tool
  # diagnostics (which can contain credentials or response bodies) out
  # of the publication log.
  local existing_id api_status
  if existing_id="$(gh api "${API}/commits/${head_sha}/check-runs?check_name=${CHECK_NAME}" \
    --jq '.check_runs | map(select(.name == "review-readiness")) | first | .id // empty' 2>/dev/null)"; then
    if [ -n "${existing_id}" ] && ! [[ "${existing_id}" =~ ^[0-9]+$ ]]; then
      echo "review-readiness: instrument failure: check-run lookup returned an invalid id" >&2
      return 1
    fi
  else
    api_status=$?
    printf 'review-readiness: instrument failure: check-run lookup failed (exit %s)\n' "${api_status}" >&2
    return 1
  fi
  if [ -n "${existing_id}" ]; then
    if gh api "${API}/check-runs/${existing_id}" -X PATCH \
      -f "status=completed" \
      -f "conclusion=${conclusion}" \
      -f "output[title]=review-readiness: ${conclusion}" \
      -f "output[summary]=${output}" >/dev/null 2>&1; then
      printf 'review-readiness: updated run %s -> %s at %s\n' \
        "${existing_id}" "${conclusion}" "${head_sha:0:12}"
    else
      api_status=$?
      printf 'review-readiness: check-run PATCH failed (exit %s); no update confirmed\n' "${api_status}" >&2
      return 1
    fi
  else
    if gh api "${API}/check-runs" -X POST \
      -f "name=${CHECK_NAME}" \
      -f "head_sha=${head_sha}" \
      -f "status=completed" \
      -f "conclusion=${conclusion}" \
      -f "output[title]=review-readiness: ${conclusion}" \
      -f "output[summary]=${output}" >/dev/null 2>&1; then
      printf 'review-readiness: published %s at %s\n' "${conclusion}" "${head_sha:0:12}"
    else
      api_status=$?
      printf 'review-readiness: check-run POST failed (exit %s); no publication confirmed\n' "${api_status}" >&2
      return 1
    fi
  fi
}

validate_projection() {
  # Validate the v1 transport envelope, not review semantics. Only the
  # typed projector decides readiness. Slurping requires exactly one
  # complete document, including on empty output or a failed process.
  jq -se --slurpfile live "$2" --arg event "$3" --argjson exit_code "$4" '
    def nonblank: type == "string" and test("\\S");
    length == 1 and (.[0] |
      type == "object" and
      .schema_id == "cargo-allow.review-readiness-check.v1" and
      .schema_version == 1 and .check_context == "review-readiness" and
      .repository == $live[0].repository and .pr_number == $live[0].pr_number and
      .event == $event and
      (.conclusion == "success" or .conclusion == "neutral" or .conclusion == "failure") and
      (.conclusion_reasons | type == "array" and length > 0 and all(.[]; nonblank)) and
      (.required_posture == "draft" or .required_posture == "ready") and
      (.stale_green_invalidated | type == "boolean") and
      (.head_ledger_bootstrap | type == "boolean") and
      (.claim_boundary | nonblank) and
      (.binding | type == "object" and
        .repository == $live[0].repository and .pr_number == $live[0].pr_number and
        .base_ref == $live[0].base_ref and .base_sha == $live[0].base_sha and
        .head_ref == $live[0].head_ref and .head_sha == $live[0].head_sha and
        .merge_base == $live[0].merge_base and .diff_digest == $live[0].diff_digest and
        (.disposition_identity | type == "string")) and
      # The CLI exits 1 after emitting a typed Failure. A compile error,
      # crash, or contradictory success is not a review conclusion.
      (($exit_code == 0 and .conclusion != "failure") or
       ($exit_code == 1 and .conclusion == "failure")))
  ' "$1" >/dev/null 2>&1
}

# project <pr_number> <event> -> publishes the check run for the PR.
# Returns nonzero on adapter breakage, a Failure conclusion, or a
# refused publish (fork token downgrade); the caller aggregates.
project_pr() {
  local pr_number="$1" event="$2"
  local pr_json base_ref base_sha head_ref head_sha draft_state merge_base diff_digest input_status
  if pr_json="$(gh pr view "$pr_number" --json baseRefName,baseRefOid,headRefName,headRefOid,isDraft 2>/dev/null)"; then
    :
  else
    input_status=$?
    printf 'review-readiness: instrument failure: PR snapshot read failed (exit %s)\n' "${input_status}" >&2
    return 1
  fi
  if ! base_ref="$(jq -er '.baseRefName | select(type == "string" and test("\\S"))' <<<"$pr_json" 2>/dev/null)" ||
    ! base_sha="$(jq -er '.baseRefOid | select(type == "string" and test("\\S"))' <<<"$pr_json" 2>/dev/null)" ||
    ! head_ref="$(jq -er '.headRefName | select(type == "string" and test("\\S"))' <<<"$pr_json" 2>/dev/null)" ||
    ! head_sha="$(jq -er '.headRefOid | select(type == "string" and test("\\S"))' <<<"$pr_json" 2>/dev/null)" ||
    ! draft_state="$(jq -er 'if .isDraft == true then "draft" elif .isDraft == false then "ready" else empty end' <<<"$pr_json" 2>/dev/null)"; then
    echo "review-readiness: instrument failure: invalid PR snapshot" >&2
    return 1
  fi
  if merge_base="$(git merge-base "origin/${base_ref}" "${head_sha}" 2>/dev/null)" && [ -n "${merge_base}" ]; then
    :
  else
    input_status=$?
    printf 'review-readiness: instrument failure: merge-base read failed (exit %s)\n' "${input_status}" >&2
    return 1
  fi
  if diff_digest="$( { git diff "${merge_base}..${head_sha}" | sha256sum | cut -d' ' -f1; } 2>/dev/null)" &&
    [[ "${diff_digest}" =~ ^[a-f0-9]{64}$ ]]; then
    :
  else
    input_status=$?
    printf 'review-readiness: instrument failure: diff digest read failed (exit %s)\n' "${input_status}" >&2
    return 1
  fi

  local live_file
  if ! live_file="$(mktemp 2>/dev/null)"; then
    echo "review-readiness: instrument failure: could not allocate live input" >&2
    return 1
  fi
  if ! jq -n \
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
      scope_claim_boundary: ("pull-request:" + ($pr_number | tostring))}' >"${live_file}" 2>/dev/null; then
    echo "review-readiness: instrument failure: could not create live input" >&2
    rm -f "${live_file}"
    return 1
  fi

  # Disposition discovery: one exact-head record first; then one
  # unique ancestor-bound record (the retained-review-ledger
  # bootstrap). Ambiguity fails closed; no match is the explicit
  # missing-disposition case.
  local exact_matches=() ancestor_matches=() malformed=() candidate bound_head
  if [ -d "${LEDGER_DIR}" ]; then
    for candidate in "${LEDGER_DIR}"/*.json; do
      [ -f "$candidate" ] || continue
      # An unreadable or malformed retained record is review evidence
      # that fails closed; it must never degrade to a missing review.
      if ! jq -e . "$candidate" >/dev/null 2>&1; then
        malformed+=("$candidate")
        continue
      fi
      if ! bound_head="$(jq -r --arg repository "${GITHUB_REPOSITORY}" \
        --argjson pr "${pr_number}" \
        'select(.repository == $repository and .pr_number == $pr) | .head_sha // ""' \
        "$candidate" 2>/dev/null)"; then
        echo "review-readiness: instrument failure: could not read candidate disposition binding" >&2
        rm -f "${live_file}"
        return 1
      fi
      [ -n "${bound_head}" ] || continue
      if [ "${bound_head}" = "${head_sha}" ]; then
        exact_matches+=("$candidate")
      elif git merge-base --is-ancestor "${bound_head}" "${head_sha}" 2>/dev/null; then
        ancestor_matches+=("$candidate")
      else
        input_status=$?
        if [ "${input_status}" -ne 1 ]; then
          printf 'review-readiness: instrument failure: disposition ancestry read failed (exit %s)\n' "${input_status}" >&2
          rm -f "${live_file}"
          return 1
        fi
      fi
    done
  fi

  local disposition_args=() delta_args=() selected=""
  if [ "${#malformed[@]}" -gt 0 ]; then
    rm -f "${live_file}"
    publish "${head_sha}" "failure" \
      "unreadable retained disposition records for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}: ${malformed[*]}; malformed review evidence fails closed" \
      "${base_sha}" "${merge_base}" "${diff_digest}" || return 1
    return 1
  elif [ "${#exact_matches[@]}" -gt 1 ] || [ "${#ancestor_matches[@]}" -gt 1 ]; then
    rm -f "${live_file}"
    publish "${head_sha}" "failure" \
      "ambiguous retained dispositions for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}: exact=${exact_matches[*]:-} ancestor=${ancestor_matches[*]:-}; disposition discovery fails closed" \
      "${base_sha}" "${merge_base}" "${diff_digest}" || return 1
    return 1
  elif [ "${#exact_matches[@]}" -eq 1 ]; then
    selected="${exact_matches[0]}"
  elif [ "${#ancestor_matches[@]}" -eq 1 ]; then
    selected="${ancestor_matches[0]}"
  fi

  if [ -n "${selected}" ]; then
    echo "review-readiness: retained disposition ${selected}"
    disposition_args=(--disposition "${selected}")
    if ! bound_head="$(jq -r '.head_sha // ""' "${selected}" 2>/dev/null)"; then
      echo "review-readiness: instrument failure: could not read selected disposition binding" >&2
      rm -f "${live_file}"
      return 1
    fi
    if [ -n "${bound_head}" ] && [ "${bound_head}" != "${head_sha}" ]; then
      # The retained-review-ledger bootstrap: pass the complete delta
      # so the projection can prove the head movement is disposition
      # records only and reject anything else.
      local delta delta_output
      if delta_output="$(git diff --name-only "${bound_head}..${head_sha}" 2>/dev/null)"; then
        :
      else
        input_status=$?
        printf 'review-readiness: instrument failure: disposition delta read failed (exit %s)\n' "${input_status}" >&2
        rm -f "${live_file}"
        return 1
      fi
      while IFS= read -r delta; do
        [ -n "$delta" ] && delta_args+=(--head-delta-path "$delta")
      done <<<"${delta_output}"
    fi
  else
    echo "review-readiness: no retained disposition for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}"
  fi

  local projection_file exit_code=0
  if ! projection_file="$(mktemp 2>/dev/null)"; then
    echo "review-readiness: instrument failure: could not allocate projection output" >&2
    rm -f "${live_file}"
    return 1
  fi
  cargo run -p cargo-allow --locked -- review-readiness project \
    --live "${live_file}" \
    --draft-state "${draft_state}" \
    --event "${event}" \
    ${disposition_args[@]+"${disposition_args[@]}"} \
    ${delta_args[@]+"${delta_args[@]}"} \
    --format json >"${projection_file}" 2>/dev/null || exit_code=$?
  if ! validate_projection "${projection_file}" "${live_file}" "${event}" "${exit_code}"; then
    printf 'review-readiness: instrument failure: unusable projection (projector exit %s); no check published\n' "${exit_code}" >&2
    rm -f "${live_file}" "${projection_file}"
    return 1
  fi
  rm -f "${live_file}"

  local conclusion summary
  if ! conclusion="$(jq -er '.conclusion' "${projection_file}" 2>/dev/null)" ||
    ! summary="$(jq -er '.conclusion_reasons | join("; ")' "${projection_file}" 2>/dev/null)"; then
    echo "review-readiness: instrument failure: could not read validated projection; no check published" >&2
    rm -f "${projection_file}"
    return 1
  fi
  if ! publish "${head_sha}" "${conclusion}" "${summary}" \
    "${base_sha}" "${merge_base}" "${diff_digest}"; then
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
  if open_prs="$(gh pr list --state open --limit 1000 --json number --jq '.[].number' 2>/dev/null)"; then
    :
  else
    input_status=$?
    printf 'review-readiness: instrument failure: open PR enumeration failed (exit %s)\n' "${input_status}" >&2
    exit 1
  fi
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
