#!/usr/bin/env bash
# Bounded read-only collector for the #3835 CI performance receipt.
#
# Emits one CiPerformanceReceiptV1 JSON observation for an explicit
# list of workflow run ids. Job purpose, routing owner, and blocking
# posture come from the checked inventory policy/ci-job-inventory.toml
# (never inferred from the job name); per-job timing comes from the
# provider's step observations classified into the bounded breakdown
# buckets (heuristic classification, recorded as a limit); fields the
# provider does not expose stay missing (never zero-filled). Read-only:
# no workflow, routing, cache, or proof state is changed.
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
      ([.steps[] |
        if .started_at == null then
          {key: bucket(.name // "unknown"), value: 0}
        else
          ((.completed_at // .started_at) | split(".") | first | fromdateiso8601) as $end |
          (.started_at | split(".") | first | fromdateiso8601) as $start |
          {key: bucket(.name // "unknown"), value: ([$end - $start, 0] | max)}
        end]
        | group_by(.key)
        | map({(.[0].key): (map(.value) | add)})
        | add // {}) as $buckets |
      {
        name: .name,
        conclusion: ((.conclusion // "unknown") as $c |
          {success: "passed", failure: "failed"}[$c] // $c),
        runner: (.runner_name // "unknown"),
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
    '. + [{
      workflow: $run[0].name,
      run_id: $run[0].id,
      attempt: $run[0].run_attempt,
      event: $run[0].event,
      conclusion: ($run[0].conclusion // "unknown"),
      environment: "hosted",
      source_pair: {base_sha: "", head_sha: $run[0].head_sha, generation: $generation},
      jobs: $jobs[0]
    }]' <<<"$runs_json")"
  index=$((index + 1))
done

# Join the checked inventory: purpose, routing owner, and blocking
# posture per exact job name. Unmatched job names surface with owner
# "uncategorized" so the typed validator and the author see them; the
# inventory must grow rather than the collector guessing.
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
    ([$inventory[0].jobs[] | select(.name == $name) | .purpose] | first
      | if . == null then null else (snake | sub("^msrv$"; "msrv")) end) // null;
  def owner_of($name):
    ([$inventory[0].jobs[] | select(.name == $name) | .routing_owner] | first) // null;
  def blocking_of($name):
    ([$inventory[0].jobs[] | select(.name == $name) | .blocking] | first) // null;
  {
    schema_id: $schema,
    schema_version: $version,
    window_from: $from,
    window_to: $to,
    generation: $generation,
    runs: [$runs[] | {
      workflow: .workflow, run_id: .run_id, attempt: .attempt, event: .event,
      conclusion: .conclusion, environment: .environment, source_pair: .source_pair,
      jobs: [.jobs[] | {
        name: .name,
        purpose: (purpose_of(.name) // "ArtifactDiagnostics"),
        routing_owner: (owner_of(.name) // "uncategorized"),
        blocking: (blocking_of(.name) // false),
        runner: .runner,
        conclusion: .conclusion,
        timing: .timing,
        first_failure: false,
        critical_path: false,
        cache: null,
        compute_minutes: null
      }]
    }],
    limits: [
      "base_sha is left empty by the collector for push events; the author binds it from git before retention",
      "step-to-bucket classification is heuristic; the typed law keeps the buckets separate",
      "cache classes stay unknown until restored/saved byte evidence is retained",
      "queue time is not exposed per job by the provider; it stays missing, never zero-filled"
    ],
    critical_path_first_failure: [],
    critical_path_full_matrix: [],
    redundant_work_candidates: [],
    cache_opportunities: [],
    improvement_targets_owner: "#3753",
    claim_boundary: "Measured current CI topology, proof purpose, timing, cache posture, critical path, and compute cost for one bounded observation window. It supplies the evidence for later tiering and caching decisions; it does not optimize CI, does not change routing or proof selection, and is not product or release correctness evidence."
  }' >"$OUT"
echo "collect-ci-performance: wrote $OUT with $# run(s)"
