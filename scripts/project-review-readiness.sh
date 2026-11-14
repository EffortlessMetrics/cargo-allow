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
# conclusion (success / failure) is visible at the required check
# boundary. Missing or neutral (unproven) review evidence publishes
# failure; GitHub accepts neutral as passing, so it is not safe here.
#
# On pushes to the base branch (no pull_request event fires), the
# adapter iterates ALL open pull requests and republishes each
# readiness result bound to the new merge base; base changes through a
# PR edit are recomputed through the `edited` event with the base
# change flag. Acquisition and final readback reject observed source
# movement; neither provides an atomic lock against later changes or
# concurrent publishers. Acquisition failure does not invalidate an
# already existing check. Aggregate ordering remains separate (#3839).
#
# Disposition digest recipe (binds the retained record to the exact
# pair). v2 (#4251-lineage: #3844): every diff-affecting knob is pinned
# on the command line so the digest is reproducible across environments:
#   git -c diff.algorithm=histogram -c diff.renames=true \
#       -c diff.noprefix=false -c core.autocrlf=false \
#       diff --no-ext-diff --no-textconv --binary --full-index -U3 \
#       <merge_base>..<head_sha> | sha256sum
# labeled `sha256:v2:`. Historical `sha256:v1:` records hashed ambient
# `git diff` bytes (algorithm/renames/context/autocrlf/textconv varied
# by environment); they are never relabeled — they verify only against
# their own v1 recompute. The review flow that authors dispositions
# must use the same pinned recipe.
#
# Fork PRs receive a read-only token: when the check-run publish is
# refused, the adapter logs the refusal and exits nonzero so the job
# status itself carries the conclusion instead.
#
# Read-only over review semantics: the only GitHub write is the
# review-readiness check run itself. No merge, no PR mutation, no
# branch-rule or release change.

set -euo pipefail

: "${GH_TOKEN:?GH_TOKEN is required}"
API="repos/${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
CHECK_NAME="review-readiness"
LEDGER_DIR=".allow/review-dispositions"

valid_commit() {
  [[ "$1" =~ ^([a-f0-9]{40}|[a-f0-9]{64})$ ]]
}

# Classify a disposition binding against the objects this checkout holds.
# A rebase or force-push drops the reviewed head from the fetch set, so a
# retained record can name a commit that is no longer present. That is a
# stale record, not an instrument failure: it must publish a failure that
# asks for a fresh exact-head review plus rebind.
#   0 = commit present
#   1 = commit absent (stale record)
#   * = the probe itself failed (instrument failure)
commit_presence() {
  local status=0
  # `rev-parse --verify --quiet` uses exit 1 for "no such object" and keeps
  # other failures distinct, unlike `cat-file -e`, whose 128 also covers
  # broken-repository errors.
  git rev-parse --verify --quiet "$1^{commit}" >/dev/null 2>&1 || status=$?
  case "${status}" in
    0) return 0 ;;
    1) return 1 ;;
    *) return "${status}" ;;
  esac
}

read_pr_snapshot() {
  local raw status
  if raw="$(gh pr view "$1" --repo "${GITHUB_REPOSITORY}" \
    --json state,baseRefName,headRefName,headRefOid,isDraft 2>/dev/null)"; then
    :
  else
    status=$?
    printf 'review-readiness: instrument failure: PR snapshot read failed (exit %s); no check published\n' "$status" >&2
    return 1
  fi
  # Exactly one open, typed snapshot. Canonical serialization also makes
  # the final comparison independent of provider JSON property order.
  if ! jq -cse '
    def refname: type == "string" and length > 0 and
      (test("[\u0000-\u0020\u007f]") | not);
    if length == 1 and (.[0] | type == "object" and .state == "OPEN" and
      (.baseRefName | refname) and (.headRefName | refname) and
      (.headRefOid | type == "string" and test("^([a-f0-9]{40}|[a-f0-9]{64})$")) and
      (.isDraft | type == "boolean"))
    then .[0] | {state, baseRefName, headRefName, headRefOid, isDraft}
    else error("invalid snapshot") end' <<<"$raw" 2>/dev/null; then
    echo 'review-readiness: instrument failure: invalid PR snapshot; no check published' >&2
    return 1
  fi
}

read_base_sha() {
  local encoded raw status sha
  if ! encoded="$(jq -nr --arg ref "$1" '$ref | @uri' 2>/dev/null)"; then
    echo 'review-readiness: instrument failure: could not encode base ref; no check published' >&2
    return 1
  fi
  if raw="$(gh api "${API}/git/ref/heads/${encoded}" 2>/dev/null)"; then
    :
  else
    status=$?
    printf 'review-readiness: instrument failure: base ref read failed (exit %s); no check published\n' "$status" >&2
    return 1
  fi
  if ! sha="$(jq -ers --arg ref "refs/heads/$1" '
    if length == 1 and (.[0] | type == "object" and .ref == $ref and
      .object.type == "commit" and
      (.object.sha | type == "string" and test("^([a-f0-9]{40}|[a-f0-9]{64})$")))
    then .[0].object.sha else error("invalid ref") end' <<<"$raw" 2>/dev/null)"; then
    echo 'review-readiness: instrument failure: invalid base ref; no check published' >&2
    return 1
  fi
  printf '%s\n' "$sha"
}

require_current_pair() {
  local current base_ref current_base
  if ! current="$(read_pr_snapshot "$1")"; then return 1; fi
  if [ "$current" != "$2" ]; then
    echo 'review-readiness: PR changed during projection; no check published' >&2
    return 1
  fi
  if ! base_ref="$(jq -er '.baseRefName' <<<"$current" 2>/dev/null)"; then
    echo 'review-readiness: instrument failure: could not read current base name; no check published' >&2
    return 1
  fi
  if ! current_base="$(read_base_sha "$base_ref")"; then return 1; fi
  if [ "$current_base" != "$3" ]; then
    echo 'review-readiness: base moved during projection; no check published' >&2
    return 1
  fi
}

publish() {
  # publish <head_sha> <conclusion> <summary> <base_sha> <merge_base> <diff_digest> <pr> <snapshot>
  # Update the authoritative run for this head when one exists; create
  # it otherwise. Pair identity is part of the published output so a
  # consumer can identify the observed pair. This is not an atomic CAS.
  local head_sha="$1" conclusion="$2" summary="$3" base_sha="$4" merge_base="$5" diff_digest="$6"
  local pr_number="$7" pr_snapshot="$8" output check_sha
  # Pull-request and base-push checks are attached to the reviewed PR
  # head. A merge queue reads checks on the merge-group commit, so that
  # event publishes on the checkout SHA after the caller has proved it
  # is the two-parent merge of this same PR head.
  check_sha="${head_sha}"
  if [ "${GITHUB_EVENT_NAME}" = "merge_group" ]; then
    if ! valid_commit "${GITHUB_SHA:-}"; then
      echo 'review-readiness: instrument failure: merge group SHA missing; no check published' >&2
      return 1
    fi
    check_sha="${GITHUB_SHA}"
  fi
  # The pair line stays first. Existing consumers and the adapter
  # characterization match output[summary]=pair: base=. The queue
  # subject is extra evidence when the check is attached to a
  # different commit than the reviewed head.
  if [ "${check_sha}" != "${head_sha}" ]; then
    output="$(printf 'pair: base=%s merge-base=%s diff=sha256:v2:%s\nqueue subject: %s\nreviewed head: %s\n\n%s' \
      "${base_sha}" "${merge_base}" "${diff_digest}" "${check_sha}" "${head_sha}" "${summary}")"
  else
    output="$(printf 'pair: base=%s merge-base=%s diff=sha256:v2:%s\n\n%s' \
      "${base_sha}" "${merge_base}" "${diff_digest}" "${summary}")"
  fi
  # These functions run in conditional contexts, where Bash disables
  # errexit. Check every API status explicitly, and keep raw tool
  # diagnostics (which can contain credentials or response bodies) out
  # of the publication log.
  local existing_id api_status
  if existing_id="$(gh api "${API}/commits/${check_sha}/check-runs?check_name=${CHECK_NAME}" \
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
  # Fence every write, including discovery failures, after lookup. Do
  # not relabel an already computed projection if any observed field moved.
  if ! require_current_pair "$pr_number" "$pr_snapshot" "$base_sha"; then return 1; fi
  if [ -n "${existing_id}" ]; then
    if gh api "${API}/check-runs/${existing_id}" -X PATCH \
      -f "status=completed" \
      -f "conclusion=${conclusion}" \
      -f "output[title]=review-readiness: ${conclusion}" \
      -f "output[summary]=${output}" >/dev/null 2>&1; then
      printf 'review-readiness: updated run %s -> %s at %s\n' \
        "${existing_id}" "${conclusion}" "${check_sha:0:12}"
      published_check=1
    else
      api_status=$?
      printf 'review-readiness: check-run PATCH failed (exit %s); no update confirmed\n' "${api_status}" >&2
      return 1
    fi
  else
    if gh api "${API}/check-runs" -X POST \
      -f "name=${CHECK_NAME}" \
      -f "head_sha=${check_sha}" \
      -f "status=completed" \
      -f "conclusion=${conclusion}" \
      -f "output[title]=review-readiness: ${conclusion}" \
      -f "output[summary]=${output}" >/dev/null 2>&1; then
      printf 'review-readiness: published %s at %s\n' "${conclusion}" "${check_sha:0:12}"
      published_check=1
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
  if ! pr_json="$(read_pr_snapshot "$pr_number")"; then return 1; fi
  if ! base_ref="$(jq -er '.baseRefName' <<<"$pr_json" 2>/dev/null)" ||
    ! head_ref="$(jq -er '.headRefName' <<<"$pr_json" 2>/dev/null)" ||
    ! head_sha="$(jq -er '.headRefOid' <<<"$pr_json" 2>/dev/null)" ||
    ! draft_state="$(jq -er 'if .isDraft then "draft" else "ready" end' <<<"$pr_json" 2>/dev/null)"; then
    echo 'review-readiness: instrument failure: invalid PR snapshot' >&2
    return 1
  fi
  if [ "${GITHUB_EVENT_NAME}" = "merge_group" ]; then
    # The reviewed head stays the pull request head. The queue commit is
    # accepted only when it is exactly that head merged onto the base,
    # which the parent check below enforces. Do not treat the queue SHA
    # as the reviewed head.
    if ! valid_commit "${MERGE_GROUP_HEAD_SHA:-}" || [ "${MERGE_GROUP_HEAD_SHA}" != "${GITHUB_SHA:-}" ]; then
      echo 'review-readiness: merge group subject does not match the checkout; fails closed' >&2
      return 1
    fi
  elif [ "${GITHUB_EVENT_NAME}" != push ] && [ "$head_sha" != "${PR_HEAD_SHA:-}" ]; then
    echo 'review-readiness: stale or missing PR event head; no check published' >&2
    return 1
  fi
  if ! base_sha="$(read_base_sha "$base_ref")"; then return 1; fi
  if [ "${GITHUB_EVENT_NAME}" = "merge_group" ]; then
    # The queue commit was built on the event base. A live base read that has
    # moved past it cannot prove this subject, and tree equality alone can
    # survive such a move (for example an empty commit on main), so require
    # the two to agree before any content proof runs.
    if [ -z "${MERGE_GROUP_BASE_SHA:-}" ] || [ "$base_sha" != "${MERGE_GROUP_BASE_SHA}" ]; then
      echo 'review-readiness: instrument failure: base moved since the queue event; fails closed' >&2
      return 1
    fi
  fi
  # Acquire immutable objects, not tracking refs or a new checkout. Fork
  # heads must be reachable through the base repository's PR objects;
  # an unavailable object is a failure even if cached history looks usable.
  if git fetch --no-tags --no-write-fetch-head --refmap= origin "$base_sha" "$head_sha" >/dev/null 2>&1; then
    :
  else
    input_status=$?
    printf 'review-readiness: instrument failure: object fetch failed (exit %s)\n' "$input_status" >&2
    return 1
  fi
  local object observed checkout parents subject_tree merge_tree queue_parent
  for object in "$base_sha" "$head_sha"; do
    if ! observed="$(git rev-parse --verify "${object}^{commit}" 2>/dev/null)" || [ "$observed" != "$object" ]; then
      echo 'review-readiness: instrument failure: fetched commit object did not verify' >&2
      return 1
    fi
  done
  if [ "${GITHUB_EVENT_NAME}" != push ]; then
    if ! valid_commit "${GITHUB_SHA:-}" ||
      ! checkout="$(git rev-parse --verify 'HEAD^{commit}' 2>/dev/null)" || [ "$checkout" != "$GITHUB_SHA" ]; then
      echo 'review-readiness: instrument failure: event checkout subject mismatch' >&2
      return 1
    fi
    if ! parents="$(git rev-list --parents -n 1 "$GITHUB_SHA" 2>/dev/null)"; then
      echo 'review-readiness: instrument failure: event synthetic parents unreadable' >&2
      return 1
    fi
    if [ "$parents" = "$GITHUB_SHA $base_sha $head_sha" ]; then
      : # two-parent merge subject
    elif [ "${GITHUB_EVENT_NAME}" = "merge_group" ]; then
      # A SQUASH queue subject has exactly one parent. Prove the content
      # instead of the parent identity: G's tree must equal the three-way
      # merge of the queue base and the reviewed head. The ref/base binding
      # above already pinned the queue base. A stale, foreign or conflicting
      # queue commit fails closed.
      parent_count="$(printf '%s\n' "$parents" | wc -w | tr -d '[:space:]')"
      if [ "$parent_count" -ne 2 ]; then
        echo 'review-readiness: instrument failure: squash queue subject must have exactly one parent' >&2
        return 1
      fi
      # The sole parent must be the event base itself. Without this, a commit
      # built on some other parent whose tree happens to match the merge of
      # base and head would pass the content proof below. A batched squash
      # entry is built on the preceding queue commit rather than the event
      # base, so it lands here too: the payload names only the group, not its
      # members, so a batch is unprovable and fails closed. The message names
      # both cases so a batch is not read as a single-PR candidate defect.
      queue_parent="${parents#* }"
      if [ "$queue_parent" != "$base_sha" ]; then
        echo 'review-readiness: instrument failure: squash queue subject parent is not the event base (batched or foreign group); fails closed' >&2
        return 1
      fi
      if ! subject_tree="$(git rev-parse --verify "${GITHUB_SHA}^{tree}" 2>/dev/null)" ||
        ! valid_commit "$subject_tree"; then
        echo 'review-readiness: instrument failure: queue subject tree unreadable' >&2
        return 1
      fi
      if ! merge_tree="$(git merge-tree --write-tree "$base_sha" "$head_sha" 2>/dev/null)"; then
        echo 'review-readiness: instrument failure: three-way merge tree unreadable' >&2
        return 1
      fi
      merge_tree="${merge_tree%%$'\n'*}"
      if ! valid_commit "$merge_tree" || [ "$subject_tree" != "$merge_tree" ]; then
        # The merge_group payload names only the group, not its members. A
        # single-parent G whose tree is not the one-PR three-way merge is
        # treated as a batched group (or another multi-entry composition)
        # and fails closed with a distinct reason, so a batch is not
        # misdiagnosed as a single-PR candidate defect.
        queue_failure_reason='batched merge group; single-PR proof not applicable'
        echo "review-readiness: ${queue_failure_reason}" >&2
        return 1
      fi
    else
      echo 'review-readiness: instrument failure: event synthetic parents mismatch' >&2
      return 1
    fi
  fi
  # A main-push checkout is not a synthetic subject for each enumerated PR.
  if merge_base="$(git merge-base --all "$base_sha" "$head_sha" 2>/dev/null)"; then
    if ! valid_commit "$merge_base"; then
      echo 'review-readiness: instrument failure: merge-base is missing or nonunique' >&2
      return 1
    fi
  else
    input_status=$?
    printf 'review-readiness: instrument failure: merge-base read failed (exit %s)\n' "${input_status}" >&2
    return 1
  fi
  if diff_digest="$( { git -c diff.algorithm=histogram -c diff.renames=true -c diff.noprefix=false -c core.autocrlf=false diff --no-ext-diff --no-textconv --binary --full-index -U3 "${merge_base}..${head_sha}" | sha256sum | cut -d' ' -f1; } 2>/dev/null)" &&
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
    --arg diff_digest "sha256:v2:${diff_digest}" \
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
  local exact_matches=() ancestor_matches=() unreachable=() malformed=() candidate bound_head
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
        continue
      fi
      # Probe the binding before asking git about ancestry: an absent
      # object makes `merge-base --is-ancestor` exit 128, which the old
      # code read as an instrument failure and published nothing at all.
      local presence=0
      commit_presence "${bound_head}" || presence=$?
      case "${presence}" in
        0) ;;
        1)
          unreachable+=("${candidate}@${bound_head}")
          continue
          ;;
        *)
          printf 'review-readiness: instrument failure: disposition object probe failed (exit %s)\n' "${presence}" >&2
          rm -f "${live_file}"
          return 1
          ;;
      esac
      if git merge-base --is-ancestor "${bound_head}" "${head_sha}" 2>/dev/null; then
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
      "${base_sha}" "${merge_base}" "${diff_digest}" "$pr_number" "$pr_json" || return 1
    return 1
  elif [ "${#exact_matches[@]}" -gt 1 ] || [ "${#ancestor_matches[@]}" -gt 1 ]; then
    rm -f "${live_file}"
    publish "${head_sha}" "failure" \
      "ambiguous retained dispositions for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}: exact=${exact_matches[*]:-} ancestor=${ancestor_matches[*]:-}; disposition discovery fails closed" \
      "${base_sha}" "${merge_base}" "${diff_digest}" "$pr_number" "$pr_json" || return 1
    return 1
  elif [ "${#exact_matches[@]}" -eq 1 ]; then
    selected="${exact_matches[0]}"
  elif [ "${#ancestor_matches[@]}" -eq 1 ]; then
    selected="${ancestor_matches[0]}"
  elif [ "${#unreachable[@]}" -gt 0 ]; then
    # A retained record that binds a commit this checkout does not hold is
    # stale review evidence. Publish the failure so the required context is
    # visible and names the rebind it needs, instead of leaving the PR
    # BLOCKED with no explanation. A valid exact/ancestor record wins over
    # a stale one, so this only fires when no current record exists.
    rm -f "${live_file}"
    publish "${head_sha}" "failure" \
      "unreachable retained disposition for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}: ${unreachable[*]} binds a commit this checkout does not contain; a rebase or force-push dropped it, so a fresh exact-head review and rebind are required; stale review evidence fails closed" \
      "${base_sha}" "${merge_base}" "${diff_digest}" "$pr_number" "$pr_json" || return 1
    return 1
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
      # --no-renames is load-bearing: with rename detection on, a head can
      # rename a file that already carries a valid record for this PR into
      # the ledger directory and delete the original invisibly, because
      # --name-only then prints only the destination. Decomposing the
      # rename emits both paths, so the source path reaches the projection
      # and is rejected as outside the one-record rule. Same knobs as the
      # digest recipe (scripts/ci-proof-route.py:1340).
      if delta_output="$(git diff --name-only --no-renames "${bound_head}..${head_sha}" 2>/dev/null)"; then
        :
      else
        input_status=$?
        if [ "${input_status}" -eq 128 ]; then
          # 128 is ambiguous: the binding may have vanished, or the
          # commit may be present with an unreadable tree. Probe the
          # commit before classifying. Absent is stale evidence and
          # still has to publish; present is a broken instrument.
          local presence=0
          commit_presence "${bound_head}" || presence=$?
          if [ "${presence}" -eq 1 ]; then
            rm -f "${live_file}"
            publish "${head_sha}" "failure" \
              "unreachable retained disposition for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}: ${selected} binds ${bound_head}, which this checkout no longer contains; a fresh exact-head review and rebind are required; stale review evidence fails closed" \
              "${base_sha}" "${merge_base}" "${diff_digest}" "$pr_number" "$pr_json" || return 1
            return 1
          fi
        fi
        printf 'review-readiness: instrument failure: disposition delta read failed (exit %s)\n' "${input_status}" >&2
        rm -f "${live_file}"
        return 1
      fi
      while IFS= read -r delta; do
        [ -n "$delta" ] && delta_args+=(--head-delta-path "$delta")
      done <<<"${delta_output}"
    fi
  else
    # Absence is known evidence absence, not a projector/instrument error.
    # Do not ask the product CLI to turn Missing into neutral: neutral passes
    # a required GitHub context and can overwrite a previous failure on push.
    rm -f "${live_file}"
    publish "${head_sha}" "failure" \
      "no retained disposition for ${GITHUB_REPOSITORY}#${pr_number}@${head_sha}; exact-head review readiness is unproven and fails closed" \
      "${base_sha}" "${merge_base}" "${diff_digest}" "$pr_number" "$pr_json" || return 1
    return 1
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
  # The product projection retains its three-state vocabulary. This adapter
  # enforces the stricter required-check boundary without changing that API.
  # A retained record alone is not affirmative readiness evidence.
  if [ "${conclusion}" = neutral ]; then
    conclusion=failure
    summary="${summary}; neutral projection does not prove exact-head review readiness; required context fails closed"
  fi
  if ! publish "${head_sha}" "${conclusion}" "${summary}" \
    "${base_sha}" "${merge_base}" "${diff_digest}" "$pr_number" "$pr_json"; then
    rm -f "${projection_file}"
    return 1
  fi
  rm -f "${projection_file}"
  [ "${exit_code}" -eq 0 ] || return 1
  [ "${conclusion}" != "failure" ] || return 1
}

# A merge-group ref names one pull request:
# gh-readonly-queue/<base>/pr-<number>-<sha>
# The trailing sha is not trusted. The number is accepted only when the
# ref base matches the event base and the checkout commit's parents are
# exactly that pull request's base and head. Anything else fails closed
# and still publishes a failure check on the queue commit.
resolve_merge_group_pr() {
  local ref base_ref resolved
  ref="${MERGE_GROUP_HEAD_REF:-}"
  ref="${ref#refs/heads/}"
  base_ref="${MERGE_GROUP_BASE_REF:-}"
  base_ref="${base_ref#refs/heads/}"
  if ! valid_commit "${GITHUB_SHA:-}" || ! valid_commit "${MERGE_GROUP_HEAD_SHA:-}" ||
    [ "${MERGE_GROUP_HEAD_SHA}" != "${GITHUB_SHA}" ] || [ -z "${base_ref}" ] ||
    [[ ! "${ref}" =~ ^gh-readonly-queue/([^/]+)/pr-([0-9]+)-([0-9a-f]{40}|[0-9a-f]{64})$ ]] ||
    [ "${BASH_REMATCH[1]}" != "${base_ref}" ]; then
    echo 'review-readiness: missing or ambiguous merge group identity; fails closed' >&2
    return 1
  fi
  resolved="${BASH_REMATCH[2]}"
  if [ "${BASH_REMATCH[3]}" != "${MERGE_GROUP_BASE_SHA:-}" ]; then
    echo 'review-readiness: merge group ref base does not equal the event base; fails closed' >&2
    return 1
  fi
  if [ -n "${PR_NUMBER:-}" ] && [ "${PR_NUMBER}" != "${resolved}" ]; then
    echo 'review-readiness: conflicting pull request numbers; fails closed' >&2
    return 1
  fi
  printf '%s\n' "${resolved}"
}

publish_queue_failure() {
  local reason="$1" existing_id api_status
  echo "review-readiness: ${reason}; fails closed" >&2
  if ! valid_commit "${GITHUB_SHA:-}"; then
    echo 'review-readiness: merge group SHA missing; no check published' >&2
    return 1
  fi
  if existing_id="$(gh api "${API}/commits/${GITHUB_SHA}/check-runs?check_name=${CHECK_NAME}" \
    --jq '.check_runs | map(select(.name == "review-readiness")) | first | .id // empty' 2>/dev/null)"; then
    if [ -n "${existing_id}" ] && ! [[ "${existing_id}" =~ ^[0-9]+$ ]]; then
      echo 'review-readiness: instrument failure: check-run lookup returned an invalid id' >&2
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
      -f "conclusion=failure" \
      -f "output[title]=review-readiness: failure" \
      -f "output[summary]=${reason}; fails closed" >/dev/null 2>&1; then
      printf 'review-readiness: updated run %s -> failure at %s\n' \
        "${existing_id}" "${GITHUB_SHA:0:12}"
    else
      api_status=$?
      printf 'review-readiness: check-run PATCH failed (exit %s); no update confirmed\n' "${api_status}" >&2
      return 1
    fi
  elif gh api "${API}/check-runs" -X POST \
    -f "name=${CHECK_NAME}" \
    -f "head_sha=${GITHUB_SHA}" \
    -f "status=completed" \
    -f "conclusion=failure" \
    -f "output[title]=review-readiness: failure" \
    -f "output[summary]=${reason}; fails closed" >/dev/null 2>&1; then
    printf 'review-readiness: published failure at %s\n' "${GITHUB_SHA:0:12}"
  else
    api_status=$?
    printf 'review-readiness: check-run POST failed (exit %s); no publication confirmed\n' "${api_status}" >&2
    return 1
  fi
  return 1
}

overall=0
if [ "${GITHUB_EVENT_NAME}" = "push" ]; then
  # Base-branch movement: no pull_request event fires, so recompute
  # every open pull request against the new base and republish. The
  # enumeration is captured (not process-substituted) so a failed
  # gh pr list fails the run instead of looking like an empty list,
  # and the limit is raised past the 30-PR default page.
  if open_prs="$(gh pr list --repo "${GITHUB_REPOSITORY}" --state open --limit 1000 --json number --jq '.[].number' 2>/dev/null)"; then
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
elif [ "${GITHUB_EVENT_NAME}" = "merge_group" ]; then
  resolved_pr=""
  published_check=0
  queue_failure_reason=""
  if resolved_pr="$(resolve_merge_group_pr)"; then
    PR_NUMBER="${resolved_pr}"
    project_pr "${PR_NUMBER}" "checks_requested" || overall=1
  else
    overall=1
  fi
  # A typed failure already published on the queue commit must keep its
  # disposition reason. The generic failure is only for the paths that
  # returned before any check existed; a batched composition carries its
  # own distinct reason from project_pr.
  if [ "${overall}" -ne 0 ] && [ "${published_check}" -eq 0 ]; then
    publish_queue_failure "${queue_failure_reason:-merge group review-readiness did not prove one exact pull request}" || true
  fi
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
