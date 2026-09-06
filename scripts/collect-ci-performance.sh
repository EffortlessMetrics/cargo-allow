#!/usr/bin/env bash
# Bounded read-only collector for the #3835 CI performance receipt.
#
# Emits one validated CiPerformanceReceiptV1 JSON observation for an
# explicit list of workflow run ids. Job purpose, routing owner, and
# blocking posture come from the checked inventory
# policy/ci-job-inventory.toml (never inferred from the job name; a
# missing row stays "uncategorized" and fails the typed validator).
# Bucket timing sums only fully completed provider steps — unstarted
# or in-progress steps contribute nothing and their buckets stay
# missing, never zero. Failure-path and critical-path projections run
# in the collector: the first failed job (start order) carries
# first_failure; the longest passed job per run carries
# critical_path; compute_minutes derives from completed job duration.
# base_sha derives per event (the run's PR base or the head's parent).
# Read-only: no workflow, routing, cache, or proof state is changed.
#
# Usage: collect-ci-performance.sh <run_id> [<run_id> ...]
# Environment: GITHUB_REPOSITORY, GH_TOKEN, CI_PERFORMANCE_OUT,
# CI_PERFORMANCE_GENERATION, CI_PERFORMANCE_WINDOW_FROM,
# CI_PERFORMANCE_WINDOW_TO

set -euo pipefail

: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GH_TOKEN:?GH_TOKEN is required}"
API="repos/${GITHUB_REPOSITORY}/actions/runs"
OUT="${CI_PERFORMANCE_OUT:?CI_PERFORMANCE_OUT is required}"
GENERATION="${CI_PERFORMANCE_GENERATION:?CI_PERFORMANCE_GENERATION is required}"
WINDOW_FROM="${CI_PERFORMANCE_WINDOW_FROM:?CI_PERFORMANCE_WINDOW_FROM is required}"
WINDOW_TO="${CI_PERFORMANCE_WINDOW_TO:?CI_PERFORMANCE_WINDOW_TO is required}"
INVENTORY="${CI_PERFORMANCE_INVENTORY:-policy/ci-job-inventory.toml}"
MAX_JOBS_PER_RUN=64

if [ "$#" -eq 0 ] || [ "$#" -gt 16 ]; then
  echo "collect-ci-performance: need 1..16 run ids" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# jq --slurpfile consumes JSON; the checked inventory is TOML, so the
# collector converts it read-only at run time.
python3 -c "import tomllib, json, sys; print(json.dumps(tomllib.load(open(sys.argv[1], 'rb'))))" "$INVENTORY" >"${work}/inventory.json"
runs_json="[]"
index=0
for run_id in "$@"; do
  gh api "${API}/${run_id}" >"${work}/run-${index}.json"
  gh api "${API}/${run_id}/jobs?per_page=100" >"${work}/jobs-${index}.raw.json"
  if [ "$(jq '.jobs | length' "${work}/jobs-${index}.raw.json")" -gt "$MAX_JOBS_PER_RUN" ]; then
    echo "collect-ci-performance: run ${run_id} exceeds the ${MAX_JOBS_PER_RUN}-job bound" >&2
    exit 1
  fi
  # base_sha: the run's PR base when associated with a pull request,
  # otherwise the head commit's first parent (push/other events).
  base_sha="$(jq -r '.pull_requests[0].base.sha // ""' "${work}/run-${index}.json")"
  if [ -z "$base_sha" ]; then
    head_sha="$(jq -r '.head_sha' "${work}/run-${index}.json")"
    base_sha="$(gh api "repos/${GITHUB_REPOSITORY}/commits/${head_sha}" --jq '.parents[0].sha' 2>/dev/null || echo '')"
  fi
  jq --slurpfile inventory "${work}/inventory.json" '
    def bucket($name):
      ($name | ascii_downcase) as $n |
      if ($n | test("checkout|set up job|toolchain")) then "setup"
      elif ($n | test("cache")) then "cache"
      elif ($n | test("test")) then "test"
      elif ($n | test("cargo |build|clippy|check|package|install|fetch|compile")) then "compile"
      elif ($n | test("^post ")) then "artifact"
      else "provider" end;
    [.jobs[] |
      # Only fully completed provider observations contribute to a
      # bucket; unstarted and in-progress steps stay missing, never
      # zero.
      ([.steps[] | select(.started_at != null and .completed_at != null) |
        ((.completed_at | .[:19] + "Z" | fromdateiso8601) -
         (.started_at | .[:19] + "Z" | fromdateiso8601)) as $seconds |
        {key: bucket(.name // "unknown"), value: $seconds}]
        | group_by(.key)
        | map({(.[0].key): (map(.value) | add)})
        | add // {}) as $buckets |
      {
        name: .name,
        conclusion: ((.conclusion // "unknown") as $c |
          {success: "passed", failure: "failed"}[$c] // $c),
        runner: (.runner_name // "unknown"),
        started_at: .started_at,
        completed_at: .completed_at,
        timing: {
          queue_seconds: null,
          setup_seconds: ($buckets.setup // null),
          cache_seconds: ($buckets.cache // null),
          compile_seconds: ($buckets.compile // null),
          test_seconds: ($buckets.test // null),
          provider_seconds: ($buckets.provider // null),
          artifact_seconds: ($buckets.artifact // null)
        }
      }]' "${work}/jobs-${index}.raw.json" >"${work}/jobs-${index}.json"
  runs_json="$(jq -c \
    --slurpfile run "${work}/run-${index}.json" \
    --slurpfile jobs "${work}/jobs-${index}.json" \
    --argjson generation "$GENERATION" \
    --arg base_sha "$base_sha" \
    '. + [{
      workflow: $run[0].name,
      run_id: $run[0].id,
      attempt: $run[0].run_attempt,
      event: $run[0].event,
      conclusion: ($run[0].conclusion // "unknown"),
      environment: "hosted",
      source_pair: {base_sha: $base_sha, head_sha: $run[0].head_sha, generation: $generation},
      jobs: $jobs[0]
    }]' <<<"$runs_json")"
  index=$((index + 1))
done

# Join the checked inventory (purpose/owner/blocking per exact job
# name), project the failure and critical paths, and derive compute
# minutes from completed job durations.
jq -n \
  --arg schema "cargo-allow.ci-performance-receipt.v1" \
  --argjson version 1 \
  --arg from "$WINDOW_FROM" \
  --arg to "$WINDOW_TO" \
  --argjson generation "$GENERATION" \
  --argjson runs "$runs_json" \
  --slurpfile inventory "${work}/inventory.json" '
  def snake: gsub("(?<a>[a-z])(?<b>[A-Z])"; "\(.a)_\(.b)") | ascii_downcase;
  def purpose_of($name):
    ([$inventory[0].jobs[] | select(.name == $name) | .purpose] | first | if . == null then null else snake end) // null;
  def owner_of($name):
    ([$inventory[0].jobs[] | select(.name == $name) | .routing_owner] | first) // "uncategorized";
  def blocking_of($name):
    ([$inventory[0].jobs[] | select(.name == $name) | .blocking] | first) // false;
  def total_seconds:
    ((.timing.setup_seconds // 0) + (.timing.cache_seconds // 0) +
     (.timing.compile_seconds // 0) + (.timing.test_seconds // 0) +
     (.timing.provider_seconds // 0) + (.timing.artifact_seconds // 0));
  def compute_minutes_of:
    if .started_at != null and .completed_at != null then
      (((.completed_at | .[:19] + "Z" | fromdateiso8601) -
        (.started_at | .[:19] + "Z" | fromdateiso8601)) / 60 | floor)
    else null end;
  ([ $runs[] | . as $run |
    ([ $run.jobs[] | {
      name: .name,
      purpose: (purpose_of(.name) // "artifact_diagnostics"),
      routing_owner: owner_of(.name),
      blocking: blocking_of(.name),
      runner: .runner,
      conclusion: .conclusion,
      timing: .timing,
      started_at: .started_at,
      completed_at: .completed_at,
      first_failure: false,
      critical_path: false,
      cache: null,
      compute_minutes: compute_minutes_of
    } ]) as $jobs |
    ([$jobs[] | select(.conclusion == "failed")][0].name // "") as $first_failed_name |
    ($jobs | [.[] | select(.conclusion == "passed") | total_seconds] | max // 0) as $max_passed |
    {
      workflow: $run.workflow, run_id: $run.run_id, attempt: $run.attempt,
      event: $run.event, conclusion: $run.conclusion,
      environment: $run.environment, source_pair: $run.source_pair,
      jobs: [$jobs[] |
        (.conclusion == "failed" and .name == $first_failed_name) as $is_first_failure |
        (.conclusion == "passed" and (total_seconds) == $max_passed and $max_passed > 0) as $on_path |
        .first_failure = $is_first_failure | .critical_path = $on_path |
        {name, purpose, routing_owner, blocking, runner, conclusion, timing,
         first_failure, critical_path, cache, compute_minutes}
      ]
    }
  ]) as $transformed_runs |
  ([$transformed_runs[] | .jobs[] | select(.conclusion == "failed" and .first_failure) | .name]
    | (first // [])) as $first_failed_candidate |
  (if ($first_failed_candidate | type) == "array" then $first_failed_candidate else [$first_failed_candidate] end) as $first_failure_list |
  ([$transformed_runs[] | .jobs[] | select(.critical_path) | .name] | unique) as $full_matrix_list |
  {
    schema_id: $schema,
    schema_version: $version,
    window_from: $from,
    window_to: $to,
    generation: $generation,
    runs: $transformed_runs,
    limits: [
      "step-to-bucket classification is heuristic; the typed law keeps the buckets separate",
      "cache classes stay unknown until restored/saved byte evidence is retained",
      "queue time is not exposed per job by the provider; it stays missing, never zero-filled",
      "compute minutes floor the hosted job duration to whole minutes"
    ],
    critical_path_first_failure: $first_failure_list,
    critical_path_full_matrix: $full_matrix_list,
    redundant_work_candidates: [],
    cache_opportunities: [],
    improvement_targets_owner: "#3753",
    claim_boundary: "Measured current CI topology, proof purpose, timing, cache posture, critical path, and compute cost for one bounded observation window. It supplies the evidence for later tiering and caching decisions; it does not optimize CI, does not change routing or proof selection, and is not product or release correctness evidence."
  }' >"$OUT"
echo "collect-ci-performance: wrote $OUT with $# run(s)"
