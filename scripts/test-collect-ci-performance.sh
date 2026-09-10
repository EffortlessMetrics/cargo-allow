#!/usr/bin/env bash
# Discriminating fixture harness for the #3835 CI performance
# collector's historical source provenance (#4173): a fake `gh` serves
# recorded provider responses — run and jobs JSON, the pregate job log
# that recorded the executed pair at run time, and commit parents — so
# an old pull_request run whose association has since moved to B1/H1
# still yields the executed B0/H0 pair, while contradictory, missing,
# or malformed provider evidence fails instead of silently
# substituting current metadata.
#
# No live GitHub call is made.

set -euo pipefail

work="${PERF_FIXTURE_WORK:-$(mktemp -d)}"
if [ -z "${PERF_FIXTURE_WORK:-}" ]; then trap 'rm -rf "$work"' EXIT; fi
echo "fixture work: $work" >&2
mkdir -p "$work/bin" "$work/fixtures"

B0="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0000"
H0="0000000000000000000000000000000000001111"
H1="1111111111111111111111111111111111112222"
B1="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb3333"
PUSH_HEAD="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4444"
ORPHAN="dddddddddddddddddddddddddddddddddddd5555"

jq -n \
  --arg merge_head "$H0" --arg b1 "$B1" --arg h1 "$H1" \
  '{id: 4242, name: "CI", event: "pull_request", status: "completed",
    conclusion: "success", head_sha: $merge_head, run_attempt: 1,
    pull_requests: [{number: 4168, head: {sha: $h1}, base: {sha: $b1}}]}' \
  >"$work/fixtures/run-pr.json"

jq -n --arg push "$PUSH_HEAD" \
  '{id: 4243, name: "CI", event: "push", status: "completed",
    conclusion: "success", head_sha: $push, run_attempt: 1,
    pull_requests: []}' \
  >"$work/fixtures/run-push.json"

jq -n --arg orphan "$ORPHAN" \
  '{id: 4245, name: "CI", event: "pull_request", status: "completed",
    conclusion: "success", head_sha: $orphan, run_attempt: 1,
    pull_requests: []}' \
  >"$work/fixtures/run-orphan.json"

mk_jobs() {
  jq -n --argjson id "$1" \
    '{count: 1, jobs: [{id: $id, name: "pregate", conclusion: "success",
      runner_name: "runner-1", started_at: "2026-09-08T01:00:00Z",
      completed_at: "2026-09-08T01:02:00Z",
      steps: [{"name": "Set up job", "started_at": "2026-09-08T01:00:00Z",
               "completed_at": "2026-09-08T01:00:10Z"}]}]}'
}
mk_jobs 900 >"$work/fixtures/jobs-pr.json"
mk_jobs 901 >"$work/fixtures/jobs-contradictory.json"
mk_jobs 902 >"$work/fixtures/jobs-missing-log.json"

# The pregate job log: the typed pre-gate step echoed the executed
# base/head with expanded values at run time. This is the immutable
# provenance record; the nested PR association has moved to B1/H1.
cat >"$work/fixtures/pregate-log-pr.txt" <<EOF
2026-09-08T01:00:40.0000000Z ##[group]Run jq -n --arg schema cargo-allow.ci-pregate-result.v1
2026-09-08T01:00:40.1000000Z jq -n --arg schema cargo-allow.ci-pregate-result.v1 --argjson version 1 \\
2026-09-08T01:00:40.2000000Z   --arg head $H0 --arg base $B0 \\
2026-09-08T01:00:41.0000000Z ##[endgroup]
EOF

# A contradictory record: the pregate log names a head that is not the
# run's head. Provenance is unavailable; no substitution occurs.
cat >"$work/fixtures/pregate-log-contradictory.txt" <<EOF
2026-09-08T01:00:40.2000000Z   --arg head $H1 --arg base $B0 \\
EOF

# Missing provenance: the log records no executed pair.
cat >"$work/fixtures/pregate-log-missing.txt" <<EOF
2026-09-08T01:00:40.0000000Z ##[group]Run cargo test --locked
EOF

printf '[%s]\n' "\"$B0\"" >"$work/fixtures/parents-push.json"

# Fake gh: serve the recorded response for the exact requested URL.
# The collector's only --jq call is the commit-parents projection for
# push events. Log requests return the recorded job log text.
export PERF_FIXTURES="$work/fixtures"
cat >"$work/bin/gh" <<'SHIM'
#!/usr/bin/env bash
url="$2"
fixtures="${PERF_FIXTURES:?}"
case "$url" in
  */actions/runs/4242) cat "$fixtures/run-pr.json" ;;
  */actions/runs/4243) cat "$fixtures/run-push.json" ;;
  */actions/runs/4244) cat "$fixtures/run-orphan.json" ;;
  */actions/runs/4245) cat "$fixtures/run-orphan.json" ;;
  */actions/runs/4242/jobs?per_page=100) cat "$fixtures/jobs-pr.json" ;;
  */actions/runs/4243/jobs?per_page=100) cat "$fixtures/jobs-pr.json" ;;
  */actions/runs/4245/jobs?per_page=100) cat "$fixtures/jobs-missing-log.json" ;;
  */actions/runs/*/jobs?per_page=100) cat "$fixtures/jobs-contradictory.json" ;;
  */actions/jobs/900/logs) cat "$fixtures/pregate-log-pr.txt" ;;
  */actions/jobs/901/logs) cat "$fixtures/pregate-log-contradictory.txt" ;;
  */actions/jobs/902/logs) cat "$fixtures/pregate-log-missing.txt" ;;
  */commits/*) cat "$fixtures/parents-push.json" ;;
  *) echo "fake gh: unexpected url $url" >&2; exit 1 ;;
esac
SHIM
chmod +x "$work/bin/gh"

repo="effortless-metrics/cargo-allow"
inventory="$PWD/policy/ci-job-inventory.toml"

collect() {
  local out="$1" run_id="$2"
  GITHUB_REPOSITORY="$repo" GH_TOKEN=x PATH="$work/bin:$PATH" \
    CI_PERFORMANCE_INVENTORY="$inventory" CI_PERFORMANCE_OUT="$out" \
    CI_PERFORMANCE_GENERATION='"g1"' \
    CI_PERFORMANCE_WINDOW_FROM="2026-09-01" CI_PERFORMANCE_WINDOW_TO="2026-09-30" \
    bash ${PERF_FIXTURE_TRACE:+-x} scripts/collect-ci-performance.sh "$run_id" >/dev/null
}

# 1. The moved association is ignored: the executed B0/H0 pair from
#    the pregate record is retained.
receipt="$work/receipt-pr.json"
collect "$receipt" 4242
actual_base="$(jq -r '.runs[0].source_pair.base_sha' "$receipt")"
if [ "$actual_base" != "$B0" ]; then
  echo "test-collect-ci-performance: moved association expected base $B0, got $actual_base" >&2
  exit 1
fi
if [ "$(jq -r '.runs[0].source_pair.head_sha' "$receipt")" != "$H0" ]; then
  echo "test-collect-ci-performance: the executed head must be retained" >&2
  exit 1
fi

# 2. Repeated collection keeps the same historical pair.
receipt2="$work/receipt-pr-2.json"
collect "$receipt2" 4242
if [ "$(jq -c '.runs[0].source_pair | {base_sha, head_sha}' "$receipt2")" != \
     "$(jq -c '.runs[0].source_pair | {base_sha, head_sha}' "$receipt")" ]; then
  echo "test-collect-ci-performance: repeated collection moved the historical pair" >&2
  exit 1
fi

# 3. Push events keep the first-parent base and the executed head.
receipt3="$work/receipt-push.json"
collect "$receipt3" 4243
push_base="$(jq -r '.runs[0].source_pair.base_sha' "$receipt3")"
if [ "$push_base" != "$B0" ]; then
  echo "test-collect-ci-performance: push first parent expected $B0, got $push_base" >&2
  exit 1
fi
if [ "$(jq -r '.runs[0].source_pair.head_sha' "$receipt3")" != "$PUSH_HEAD" ]; then
  echo "test-collect-ci-performance: push head must be the executed subject" >&2
  exit 1
fi

# 4. A contradictory pregate record fails instead of substituting.
if collect "$work/receipt-contradictory.json" 4244 2>"$work/contradictory.log"; then
  echo "test-collect-ci-performance: a contradictory pregate record must fail" >&2
  exit 1
fi
grep -q "unavailable" "$work/contradictory.log" \
  || { echo "test-collect-ci-performance: the failure must name the unavailability" >&2; exit 1; }
[ ! -f "$work/receipt-contradictory.json" ] \
  || { echo "test-collect-ci-performance: no receipt may be written on failure" >&2; exit 1; }

# 5. Malformed provider output fails closed (no receipt written).
cat >"$work/bin/gh" <<'SHIM'
#!/usr/bin/env bash
case "$2" in
  */actions/runs/4242) echo "{ not json" ;;
  *) echo "fake gh: unexpected url $2" >&2; exit 1 ;;
esac
SHIM
chmod +x "$work/bin/gh"
if collect "$work/receipt-bad.json" 4242 2>"$work/bad.log"; then
  echo "test-collect-ci-performance: malformed provider output must fail" >&2
  exit 1
fi
[ ! -f "$work/receipt-bad.json" ] \
  || { echo "test-collect-ci-performance: no receipt may be written on failure" >&2; exit 1; }

# 6. A pregate log that records no executed pair is unavailable.
cat >"$work/bin/gh" <<'SHIM'
#!/usr/bin/env bash
fixtures="${PERF_FIXTURES:?}"
url="$2"
case "$url" in
  */actions/runs/4245) cat "$fixtures/run-orphan.json" ;;
  */actions/runs/4245/jobs?per_page=100) cat "$fixtures/jobs-missing-log.json" ;;
  */actions/jobs/902/logs) cat "$fixtures/pregate-log-missing.txt" ;;
  *) echo "fake gh: unexpected url $2" >&2; exit 1 ;;
esac
SHIM
chmod +x "$work/bin/gh"
if collect "$work/receipt-missing.json" 4245 2>"$work/missing.log"; then
  echo "test-collect-ci-performance: a provenance-less pregate log must fail" >&2
  exit 1
fi
grep -q "unavailable" "$work/missing.log" \
  || { echo "test-collect-ci-performance: the missing-provenance failure must name the unavailability" >&2; exit 1; }

echo "test-collect-ci-performance: provenance fixtures green (moved association, determinism, push, contradictory, malformed, missing)"
