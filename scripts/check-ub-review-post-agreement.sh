#!/usr/bin/env bash
# Agreement check between one ub-review run's retained publication records
# (#4255).
#
# The pinned action's gate serializer can record
# `publication_result="posted"` from intent alone while the retained
# `review/post-error.json` records the actual tolerated posting failure
# (missing-token preflight in every observed reproduction). The upstream
# truthful-`posted` serializer fix is owned by EffortlessMetrics/ub-review#959;
# this caller-side check only reads the retained record pair and fails the
# advisory wrapper step when a posted claim has no acknowledgement:
#
#   review/gate_outcome.json   publication_result: posted | not_needed | failed | not_proven
#   review/post-result.json    status: "ok" on an API-acknowledged successful post
#   review/post-error.json     status: "failed" with error_kind/failure_stage/...
#
# The check never mutates the run directory, never reads or prints token
# values, and never decides the review verdict itself; advisory enforcement
# stays with `fail-on-gate: 'false'`.
#
# Exit codes: 0 agreed or nothing to check, 1 contradiction or unverifiable
# posted claim, 2 instrument failure (malformed retained gate record).
#
# Usage:
#   check-ub-review-post-agreement.sh <run-dir>    live check (default target/ub-review)
#   check-ub-review-post-agreement.sh --self-test  fixture self-test
set -euo pipefail

usage() {
  printf 'usage: %s [<run-dir>]|--self-test\n' "$(basename "${BASH_SOURCE[0]}")" >&2
  exit 2
}

run_check() {
  local run_dir="$1"
  command -v python3 >/dev/null 2>&1 ||
    { echo 'check-ub-review-post-agreement: python3 is required' >&2; exit 2; }
  python3 - "${run_dir}" <<'PYEOF'
import json
import sys
from pathlib import Path

run_dir = Path(sys.argv[1])
review_dir = run_dir / "review"
gate_path = review_dir / "gate_outcome.json"
result_path = review_dir / "post-result.json"
error_path = review_dir / "post-error.json"


def load(path):
    """Return None when absent, False when present but unreadable/malformed."""
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None
    except (OSError, ValueError):
        return False


gate = load(gate_path)
if gate is None:
    print(
        f"::notice::{run_dir}: no review/gate_outcome.json retained; "
        "nothing to agreement-check"
    )
    sys.exit(0)
if gate is False or not isinstance(gate, dict):
    print(
        f"::error::{gate_path}: retained gate_outcome.json is not a readable "
        "JSON object; the publication claim cannot be verified"
    )
    sys.exit(2)

publication = gate.get("publication_result")
if publication != "posted":
    print(
        "AGREEMENT ok: publication_result="
        f"{publication!r} is not a posted claim; no contradiction is possible"
    )
    sys.exit(0)

result = load(result_path)
if isinstance(result, dict) and result.get("status") == "ok":
    print(
        "AGREEMENT ok: publication_result=posted with the acknowledged "
        f"{result_path.name} receipt"
    )
    sys.exit(0)

error = load(error_path)
if isinstance(error, dict) and error.get("status") == "failed":
    fields = {
        key: error.get(key)
        for key in (
            "error_kind",
            "failure_stage",
            "token_present",
            "payload_written",
            "would_post",
            "failure_tolerated",
        )
    }
    print(
        "::error::publication_result=posted contradicts the retained "
        f"{error_path.name} record {json.dumps(fields, sort_keys=True)}; "
        "the posted claim is not acknowledged by any post-result receipt"
    )
    sys.exit(1)

print(
    f"::error::publication_result=posted with neither an acknowledged "
    f"{result_path.name} receipt nor a recorded {error_path.name}; the posted "
    "claim is unverifiable from the retained records"
)
sys.exit(1)
PYEOF
}

build_fixture() {
  local dir="$1" gate="$2" result="$3" error="$4"
  mkdir -p "${dir}/review"
  [[ -z "${gate}" ]] || printf '%s' "${gate}" > "${dir}/review/gate_outcome.json"
  [[ -z "${result}" ]] || printf '%s' "${result}" > "${dir}/review/post-result.json"
  [[ -z "${error}" ]] || printf '%s' "${error}" > "${dir}/review/post-error.json"
}

expect_exit() {
  local expected="$1" label="$2" dir="$3" status
  set +e
  run_check "${dir}" >/dev/null 2>&1
  status=$?
  set -e
  if [[ "${status}" -ne "${expected}" ]]; then
    printf 'self-test %s: expected exit %s, got %s\n' \
      "${label}" "${expected}" "${status}" >&2
    return 1
  fi
  printf 'self-test %s: exit %s as expected\n' "${label}" "${status}"
}

run_self_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "${SELF_TEST_TMP}"' EXIT
  SELF_TEST_TMP="${tmp}"

  local gate_posted gate_not_needed gate_failed
  local post_error_missing_token post_error_rejected post_result_ok
  gate_posted='{"schema":"ub-review.gate_outcome.v1","conclusion":"pass","publication_result":"posted","gate_result":"pass","not_proven_reasons":[]}'
  gate_not_needed='{"schema":"ub-review.gate_outcome.v1","conclusion":"pass","publication_result":"not_needed","gate_result":"pass"}'
  gate_failed='{"schema":"ub-review.gate_outcome.v1","conclusion":"fail","publication_result":"failed","gate_result":"fail"}'
  post_error_missing_token='{"schema_version":1,"status":"failed","error_kind":"missing_token","failure_stage":"preflight","token_present":false,"payload_written":false,"would_post":false,"failure_tolerated":true,"fail_on_post_error":false}'
  post_error_rejected='{"schema_version":1,"status":"failed","error_kind":"http_422_unprocessable","failure_stage":"pending_review_creation","token_present":true,"payload_written":true,"would_post":true,"failure_tolerated":true,"http_status":422}'
  post_result_ok='{"schema_version":1,"status":"ok","repo":"EffortlessMetrics/cargo-allow","pull_number":1}'

  build_fixture "${tmp}/missing-token" "${gate_posted}" '' "${post_error_missing_token}"
  build_fixture "${tmp}/rejected-post" "${gate_posted}" '' "${post_error_rejected}"
  build_fixture "${tmp}/success" "${gate_posted}" "${post_result_ok}" ''
  build_fixture "${tmp}/artifact-only" "${gate_not_needed}" '' ''
  build_fixture "${tmp}/truthful-failure" "${gate_failed}" '' "${post_error_missing_token}"
  build_fixture "${tmp}/posted-unverified" "${gate_posted}" '' ''
  build_fixture "${tmp}/missing-gate" '' '' ''
  mkdir -p "${tmp}/malformed-gate/review"
  printf 'not json at all' > "${tmp}/malformed-gate/review/gate_outcome.json"

  expect_exit 1 missing-token "${tmp}/missing-token"
  expect_exit 1 rejected-post "${tmp}/rejected-post"
  expect_exit 0 success "${tmp}/success"
  expect_exit 0 artifact-only "${tmp}/artifact-only"
  expect_exit 0 truthful-failure "${tmp}/truthful-failure"
  expect_exit 1 posted-unverified "${tmp}/posted-unverified"
  expect_exit 0 missing-gate "${tmp}/missing-gate"
  expect_exit 2 malformed-gate "${tmp}/malformed-gate"

  printf 'check-ub-review-post-agreement self-test: passed\n'
}

case "${1:-}" in
  --self-test) run_self_test ;;
  '') run_check 'target/ub-review' ;;
  -h|--help) usage ;;
  *) run_check "$1" ;;
esac
