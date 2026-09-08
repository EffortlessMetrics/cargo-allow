#!/usr/bin/env bash
# Full-adapter characterization for #4165. Cargo, git, and gh are stubs;
# jq is real. No compilation, network, or live check publication occurs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${ROOT}/target"
work="$(mktemp -d "${ROOT}/target/review-readiness-fixture.XXXXXX")"
trap 'rm -rf "${work}"' EXIT
mkdir -p "${work}/bin" "${work}/repo" "${work}/tmp"
export REAL_JQ
REAL_JQ="$(command -v jq)"

# Native jq on Git Bash otherwise emits CRLF into shell variables.
cat >"${work}/bin/jq" <<'SH'
#!/usr/bin/env bash
case "${FIXTURE_JQ_FAILURE:-none}:$1:${2:-}" in
  validate:-se:* | conclusion:-er:.conclusion | summary:-er:*)
    printf 'private-jq-canary\n' >&2
    exit 27
    ;;
esac
if [[ "${OSTYPE}" == msys* ]]; then
  exec "${REAL_JQ}" -b "$@"
fi
exec "${REAL_JQ}" "$@"
SH

cat >"${work}/bin/git" <<'SH'
#!/usr/bin/env bash
case "$1" in
  merge-base) printf '%040d\n' 1 ;;
  diff) : ;;
  *) exit 90 ;;
esac
SH

cat >"${work}/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
live="" event=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --live) live="$2"; shift ;;
    --event) event="$2"; shift ;;
  esac
  shift
done
printf 'private-projector-canary\n' >&2
case "${FIXTURE_OUTPUT}" in
  empty) exit "${FIXTURE_CARGO_STATUS}" ;;
  invalid) printf '{private-invalid-json-canary\n'; exit "${FIXTURE_CARGO_STATUS}" ;;
esac
jq -n --slurpfile live "${live}" --arg event "${event}" --arg conclusion "${FIXTURE_CONCLUSION}" '
  {schema_id: "cargo-allow.review-readiness-check.v1", schema_version: 1,
   check_context: "review-readiness", repository: $live[0].repository,
   pr_number: $live[0].pr_number, event: $event, conclusion: $conclusion,
   conclusion_reasons: ["fixture typed result"],
   required_posture: (if $conclusion == "success" then "ready" else "draft" end),
   stale_green_invalidated: false, head_ledger_bootstrap: false,
   binding: ($live[0] | del(.review_protocol, .scope_claim_boundary) + {disposition_identity: ""}),
   claim_boundary: "mock transport fixture, no review performed"}' |
  jq "${FIXTURE_FILTER}"
if [ "${FIXTURE_OUTPUT}" = multiple ]; then
  printf '{}\n'
fi
exit "${FIXTURE_CARGO_STATUS}"
SH

cat >"${work}/bin/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
case "$1 $2" in
  'pr view')
    printf '{"baseRefName":"main","baseRefOid":"%040d","headRefName":"fixture","headRefOid":"%040d","isDraft":false}\n' 1 2
    ;;
  'pr list') printf '4165\n' ;;
  api\ *)
    printf '%s\n' "$2" >>"${FIXTURE_STATE}/api-calls"
    if [[ "$2" == */commits/* ]]; then
      if [ "${FIXTURE_LOOKUP_STATUS}" -ne 0 ]; then
        printf 'private-lookup-canary\n' >&2
        exit "${FIXTURE_LOOKUP_STATUS}"
      fi
      shift 2
      [ "$1" = --jq ] || exit 91
      # Apply the actual adapter query, so a broken name filter cannot
      # pass the update fixture by receiving an unconditional fake id.
      jq -r "$2" "${FIXTURE_STATE}/check-runs.json"
    else
      printf '%s\n' "$@" >"${FIXTURE_STATE}/mutation-args"
      printf 'private-publication-canary\n' >&2
      exit "${FIXTURE_WRITE_STATUS}"
    fi
    ;;
  *) exit 92 ;;
esac
SH
chmod +x "${work}/bin/"*
export PATH="${work}/bin:${PATH}" TMPDIR="${work}/tmp"
export GH_TOKEN=fixture-token GITHUB_REPOSITORY=EffortlessMetrics/cargo-allow
export PR_NUMBER=4165 PR_EVENT_ACTION=synchronize PR_HEAD_SHA
PR_HEAD_SHA="$(printf '%040d' 2)"
unset CHECK_NAME

fail() { printf 'FAIL %s: %s\n' "${case_name}" "$1" >&2; exit 1; }

run_case() {
  local case_name="$1" expected_status="$2" expected_write="$3" expected_message="$4"
  local result=0
  export FIXTURE_STATE="${work}/${case_name}"
  mkdir -p "${FIXTURE_STATE}"
  printf '%s\n' "${check_runs}" >"${FIXTURE_STATE}/check-runs.json"
  (
    cd "${work}/repo"
    bash "${ROOT}/scripts/project-review-readiness.sh"
  ) >"${FIXTURE_STATE}/log" 2>&1 || result=$?
  [ "${result}" -eq "${expected_status}" ] || fail "exit ${result}, expected ${expected_status}"
  grep -Fq "${expected_message}" "${FIXTURE_STATE}/log" || fail "missing expected diagnostic"
  if grep -Eq 'private-.*-canary' "${FIXTURE_STATE}/log"; then
    fail "raw tool payload leaked"
  fi
  if [ "${expected_write}" = none ]; then
    [ ! -e "${FIXTURE_STATE}/mutation-args" ] || fail "unexpected API mutation"
  else
    grep -Fxq "${expected_write}" "${FIXTURE_STATE}/mutation-args" || fail "wrong API method"
    grep -Fxq "conclusion=${FIXTURE_CONCLUSION}" "${FIXTURE_STATE}/mutation-args" || fail "wrong conclusion"
    grep -Fq 'output[summary]=pair: base=' "${FIXTURE_STATE}/mutation-args" || fail "missing bound summary"
    grep -Fxq 'fixture typed result' "${FIXTURE_STATE}/mutation-args" || fail "missing reason"
    if [ "${expected_write}" = PATCH ]; then
      grep -Fxq 'repos/EffortlessMetrics/cargo-allow/check-runs/42' "${FIXTURE_STATE}/mutation-args" || fail "wrong update id"
    else
      grep -Fxq 'name=review-readiness' "${FIXTURE_STATE}/mutation-args" || fail "wrong check identity"
    fi
  fi
  if [ "${expected_write}" = none ] || [ "${FIXTURE_WRITE_STATUS}" -ne 0 ]; then
    if grep -Eq 'review-readiness: (published|updated)' "${FIXTURE_STATE}/log"; then
      fail "failure printed a success message"
    fi
  fi
  if [[ "${expected_message}" == *'unusable projection'* ]]; then
    [ ! -e "${FIXTURE_STATE}/api-calls" ] || fail "invalid projection reached the API"
  fi
  [ -z "$(find "${work}/tmp" -type f -print -quit)" ] || fail "temporary projection leaked"
  printf 'ok %s\n' "${case_name}"
}

export FIXTURE_OUTPUT=json FIXTURE_CARGO_STATUS=0 FIXTURE_CONCLUSION=success FIXTURE_FILTER=.
export FIXTURE_LOOKUP_STATUS=0 FIXTURE_WRITE_STATUS=0 GITHUB_EVENT_NAME=pull_request
check_runs='{"check_runs":[]}'
run_case create_success 0 POST 'published success'
FIXTURE_CONCLUSION=neutral
run_case create_neutral 0 POST 'published neutral'
FIXTURE_CONCLUSION=failure FIXTURE_CARGO_STATUS=1
run_case typed_failure 1 POST 'published failure'
FIXTURE_CONCLUSION=success FIXTURE_CARGO_STATUS=0
# An unrelated check must never receive the readiness update.
check_runs='{"check_runs":[{"name":"unrelated","id":17},{"name":"review-readiness","id":42}]}'
run_case update_success 0 PATCH 'updated run 42 -> success'
FIXTURE_WRITE_STATUS=22
run_case failed_patch 1 PATCH 'PATCH failed (exit 22)'
check_runs='{"check_runs":[]}'
run_case failed_post 1 POST 'POST failed (exit 22)'
FIXTURE_WRITE_STATUS=0 FIXTURE_LOOKUP_STATUS=23
run_case failed_lookup 1 none 'lookup failed (exit 23)'
FIXTURE_LOOKUP_STATUS=0
check_runs='{private-lookup-json-canary'
run_case malformed_lookup_json 1 none 'lookup failed'
check_runs='{"check_runs":[{"name":"review-readiness","id":"invalid"}]}'
run_case invalid_lookup_id 1 none 'lookup returned an invalid id'
check_runs='{"check_runs":[]}'
export FIXTURE_JQ_FAILURE=validate
run_case failed_jq_validation 1 none 'unusable projection'
FIXTURE_JQ_FAILURE=conclusion
run_case failed_jq_conclusion 1 none 'could not read validated projection'
FIXTURE_JQ_FAILURE=summary
run_case failed_jq_summary 1 none 'could not read validated projection'
FIXTURE_JQ_FAILURE=none

FIXTURE_OUTPUT=empty FIXTURE_CARGO_STATUS=101
run_case failed_cargo_empty 1 none 'unusable projection (projector exit 101)'
FIXTURE_CARGO_STATUS=0
run_case empty_output 1 none 'unusable projection (projector exit 0)'
FIXTURE_OUTPUT=invalid
run_case invalid_json 1 none 'unusable projection'
FIXTURE_OUTPUT=multiple
run_case multiple_documents 1 none 'unusable projection'
FIXTURE_OUTPUT=json FIXTURE_CARGO_STATUS=1
run_case failed_cargo_success_json 1 none 'unusable projection (projector exit 1)'
FIXTURE_CARGO_STATUS=101 FIXTURE_CONCLUSION=failure
run_case crashed_cargo_failure_json 1 none 'unusable projection (projector exit 101)'
FIXTURE_CARGO_STATUS=0
run_case zero_exit_failure_json 1 none 'unusable projection'
FIXTURE_CONCLUSION=success

for mutation in \
  '.conclusion = ""' '.conclusion = "cancelled"' 'del(.conclusion)' \
  '.conclusion_reasons = []' '.conclusion_reasons = [null]' \
  '.conclusion_reasons = [" "]' 'del(.conclusion_reasons)' \
  '.schema_id = "unknown"' '.schema_version = 2' \
  '.check_context = "other"' 'del(.binding)' '.binding.head_sha = "stale"' \
  'del(.binding.disposition_identity)' '.event = "opened"' '.repository = "other"' \
  '.pr_number = 1' 'del(.required_posture)' 'del(.stale_green_invalidated)' \
  'del(.head_ledger_bootstrap)' 'del(.claim_boundary)' '[]' 'null'; do
  FIXTURE_FILTER="${mutation}"
  run_case "invalid_envelope_${mutation//[^a-zA-Z0-9]/_}" 1 none 'unusable projection'
done
FIXTURE_FILTER=.
GITHUB_EVENT_NAME=push FIXTURE_WRITE_STATUS=22
run_case push_publication_failure 1 POST 'POST failed (exit 22)'
FIXTURE_WRITE_STATUS=0
run_case push_success 0 POST 'published success'

printf 'all review-readiness adapter characterization checks passed\n'
