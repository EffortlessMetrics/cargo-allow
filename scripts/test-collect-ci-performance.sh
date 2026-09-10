#!/usr/bin/env bash
# Discriminating fixture harness for the #3835 CI performance
# collector's historical source provenance (#4173): a fake `gh` serves
# recorded provider responses, so an old pull_request run whose
# association has since moved to B1/H1 still yields the executed
# B0/H0 pair — and contradictory, parentless, or malformed provider
# evidence fails instead of silently substituting current metadata.
#
# No live GitHub call is made.

set -euo pipefail

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/fixtures"

B0="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0000"
H0="0000000000000000000000000000000000000000000000000000000000000000head"
H1="1111111111111111111111111111111111111111111111111111111111111111"
B1="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb1111"
MERGE="eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
PUSH_HEAD="aaaa00000000000000000000000000000000000000000000000000000000000a"
ORPHAN="dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"

jq -n \
  --arg merge "$MERGE" --arg b1 "$B1" --arg h1 "$H1" \
  '{id: 4242, name: "CI", event: "pull_request", status: "completed",
    conclusion: "success", head_sha: $merge, run_attempt: 1,
    pull_requests: [{number: 4168, head: {sha: $h1}, base: {sha: $b1}}]}' \
  >"$work/fixtures/run-pr.json"

jq -n --arg push "$PUSH_HEAD" \
  '{id: 4243, name: "CI", event: "push", status: "completed",
    conclusion: "success", head_sha: $push, run_attempt: 1,
    pull_requests: []}' \
  >"$work/fixtures/run-push.json"

jq -n --arg orphan "$ORPHAN" \
  '{id: 4244, name: "CI", event: "pull_request", status: "completed",
    conclusion: "success", head_sha: $orphan, run_attempt: 1,
    pull_requests: []}' \
  >"$work/fixtures/run-orphan.json"

cat >"$work/fixtures/jobs.json" <<'JSON'
{"count": 1, "jobs": [{"id": 900, "name": "pregate", "conclusion": "success",
  "runner_name": "runner-1", "started_at": "2026-09-08T01:00:00Z",
  "completed_at": "2026-09-08T01:02:00Z",
  "steps": [{"name": "Set up job", "started_at": "2026-09-08T01:00:00Z",
             "completed_at": "2026-09-08T01:00:10Z"}]}]}
JSON

# The synthetic merge commit: immutable at execution time, parents
# are the executed base and head. The parent arrays are the
# pre-applied `--jq '[.parents[].sha]'` projection the collector
# requests through gh.
cat >"$work/fixtures/commit-merge.json" <<EOF
{"sha": "$MERGE", "parents": [{"sha": "$B0"}, {"sha": "$H0"}]}
EOF
printf '[%s, %s]\n' "\"$B0\"" "\"$H0\"" >"$work/fixtures/parents-merge.json"
cat >"$work/fixtures/commit-push.json" <<EOF
{"sha": "$PUSH_HEAD", "parents": [{"sha": "$B0"}]}
EOF
printf '[%s]\n' "\"$B0\"" >"$work/fixtures/parents-push.json"
cat >"$work/fixtures/commit-orphan.json" <<EOF
{"sha": "$ORPHAN", "parents": [{"sha": "$B0"}]}
EOF
printf '[%s]\n' "\"$B0\"" >"$work/fixtures/parents-orphan.json"

# Fake gh: serve the recorded response for the exact requested URL.
# The collector's only --jq call is the commit-parents projection, so
# commit URLs serve the pre-projected arrays when --jq is present.
export PERF_FIXTURES="$work/fixtures"
export FIX_MERGE="$MERGE" FIX_PUSH="$PUSH_HEAD" FIX_ORPHAN="$ORPHAN"
cat >"$work/bin/gh" <<'SHIM'
#!/usr/bin/env bash
url="$2"
fixtures="${PERF_FIXTURES:?}"
case "$url" in
  */actions/runs/4242) cat "$fixtures/run-pr.json" ;;
  */actions/runs/4243) cat "$fixtures/run-push.json" ;;
  */actions/runs/4244) cat "$fixtures/run-orphan.json" ;;
  */actions/runs/*/jobs?per_page=100) cat "$fixtures/jobs.json" ;;
  */commits/$FIX_MERGE)
    if [ "${3:-}" = "--jq" ]; then cat "$fixtures/parents-merge.json"; else cat "$fixtures/commit-merge.json"; fi ;;
  */commits/$FIX_PUSH)
    if [ "${3:-}" = "--jq" ]; then cat "$fixtures/parents-push.json"; else cat "$fixtures/commit-push.json"; fi ;;
  */commits/$FIX_ORPHAN)
    if [ "${3:-}" = "--jq" ]; then cat "$fixtures/parents-orphan.json"; else cat "$fixtures/commit-orphan.json"; fi ;;
  *) echo "fake gh: unexpected url $url" >&2; exit 1 ;;
esac
SHIM
chmod +x "$work/bin/gh"

repo="effortless-metrics/cargo-allow"
inventory="$PWD/policy/ci-job-inventory.toml"

assert_base() {
  local receipt="$1" expected="$2" label="$3"
  local actual
  actual="$(jq -r '.runs[0].source_pair.base_sha' "$receipt")"
  if [ "$actual" != "$expected" ]; then
    echo "test-collect-ci-performance: $label expected base $expected, got $actual" >&2
    exit 1
  fi
}

# 1. The moved association is ignored: the executed B0 is retained.
receipt="$work/receipt-pr.json"
GITHUB_REPOSITORY="$repo" GH_TOKEN=x PATH="$work/bin:$PATH" \
  CI_PERFORMANCE_INVENTORY="$inventory" CI_PERFORMANCE_OUT="$receipt" \
  CI_PERFORMANCE_GENERATION='"g1"' \
  CI_PERFORMANCE_WINDOW_FROM="2026-09-01" CI_PERFORMANCE_WINDOW_TO="2026-09-30" \
  bash scripts/collect-ci-performance.sh 4242 >/dev/null
assert_base "$receipt" "$B0" "moved pull_request association"
actual_pair="$(jq -c '.runs[0].source_pair | {base_sha, head_sha}' "$receipt")"
expected_pair="$(jq -cn --arg b "$B0" --arg m "$MERGE" '{base_sha: $b, head_sha: $m}')"
if [ "$actual_pair" != "$expected_pair" ]; then
  echo "test-collect-ci-performance: executed pair mismatch: $actual_pair" >&2
  exit 1
fi

# 2. Repeated collection keeps the same historical pair.
receipt2="$work/receipt-pr-2.json"
GITHUB_REPOSITORY="$repo" GH_TOKEN=x PATH="$work/bin:$PATH" \
  CI_PERFORMANCE_INVENTORY="$inventory" CI_PERFORMANCE_OUT="$receipt2" \
  CI_PERFORMANCE_GENERATION='"g1"' \
  CI_PERFORMANCE_WINDOW_FROM="2026-09-01" CI_PERFORMANCE_WINDOW_TO="2026-09-30" \
  bash scripts/collect-ci-performance.sh 4242 >/dev/null
if [ "$(jq -c '.runs[0].source_pair | {base_sha, head_sha}' "$receipt2")" != "$expected_pair" ]; then
  echo "test-collect-ci-performance: repeated collection moved the historical pair" >&2
  exit 1
fi

# 3. Push events keep the first-parent base and the executed head.
receipt3="$work/receipt-push.json"
GITHUB_REPOSITORY="$repo" GH_TOKEN=x PATH="$work/bin:$PATH" \
  CI_PERFORMANCE_INVENTORY="$inventory" CI_PERFORMANCE_OUT="$receipt3" \
  CI_PERFORMANCE_GENERATION='"g1"' \
  CI_PERFORMANCE_WINDOW_FROM="2026-09-01" CI_PERFORMANCE_WINDOW_TO="2026-09-30" \
  bash scripts/collect-ci-performance.sh 4243 >/dev/null
assert_base "$receipt3" "$B0" "push first parent"
if [ "$(jq -r '.runs[0].source_pair.head_sha' "$receipt3")" != "$PUSH_HEAD" ]; then
  echo "test-collect-ci-performance: push head must be the executed subject" >&2
  exit 1
fi

# 4. A contradictory pull_request pair fails instead of substituting.
if GITHUB_REPOSITORY="$repo" GH_TOKEN=x PATH="$work/bin:$PATH" \
  CI_PERFORMANCE_INVENTORY="$inventory" CI_PERFORMANCE_OUT="$work/receipt-orphan.json" \
  CI_PERFORMANCE_GENERATION='"g1"' \
  CI_PERFORMANCE_WINDOW_FROM="2026-09-01" CI_PERFORMANCE_WINDOW_TO="2026-09-30" \
  bash scripts/collect-ci-performance.sh 4244 >"$work/orphan.log" 2>&1; then
  echo "test-collect-ci-performance: a single-parent pull_request head must fail" >&2
  exit 1
fi
grep -q "unavailable" "$work/orphan.log" \
  || { echo "test-collect-ci-performance: the failure must name the unavailability" >&2; exit 1; }

# 5. Malformed provider output fails closed (no receipt written).
cat >"$work/bin/gh" <<'SHIM'
#!/usr/bin/env bash
case "$2" in
  */actions/runs/4242) echo "{ not json" ;;
  *) echo "fake gh: unexpected url $1" >&2; exit 1 ;;
esac
SHIM
chmod +x "$work/bin/gh"
if GITHUB_REPOSITORY="$repo" GH_TOKEN=x PATH="$work/bin:$PATH" \
  CI_PERFORMANCE_INVENTORY="$inventory" CI_PERFORMANCE_OUT="$work/receipt-bad.json" \
  CI_PERFORMANCE_GENERATION='"g1"' \
  CI_PERFORMANCE_WINDOW_FROM="2026-09-01" CI_PERFORMANCE_WINDOW_TO="2026-09-30" \
  bash scripts/collect-ci-performance.sh 4242 >"$work/bad.log" 2>&1; then
  echo "test-collect-ci-performance: malformed provider output must fail" >&2
  exit 1
fi
[ ! -f "$work/receipt-bad.json" ] \
  || { echo "test-collect-ci-performance: no receipt may be written on failure" >&2; exit 1; }

echo "test-collect-ci-performance: provenance fixtures green (moved association, determinism, push, contradictory, malformed)"
