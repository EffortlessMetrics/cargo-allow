#!/usr/bin/env bash
# Aggregate lane over the workflow construction denominator (#3907 PR
# D): runs both lane scripts in fetch-only mode, grades them through
# the hidden workflow-construction CLI, and emits the stable semantic
# aggregate.
#
# Advisory: completed runs always exit zero — findings are the
# evidence and the report is retained even when the aggregate carries
# findings. Only an aggregate instrument failure exits nonzero.
# Enforcement selection is the #2283/#2284 authority.
#
# Environment:
#   ZIZMOR_BIN / ACTIONLINT_BIN  pre-fetched analyzer binaries (local
#                                parity without network).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

export SKIP_EVALUATE=1

ACTIONLINT_BIN="${ACTIONLINT_BIN:-}" \
bash scripts/check-workflow-syntax.sh
cp tool-run.json "${work}/syntax-run.json"

ZIZMOR_BIN="${ZIZMOR_BIN:-}" \
bash scripts/check-workflow-security.sh
cp tool-run-security.json "${work}/security-run.json"

cargo run -q -p cargo-allow -- workflow-construction aggregate \
  --syntax-run "${work}/syntax-run.json" \
  --security-run "${work}/security-run.json" \
  --exceptions policy/workflow-security-exceptions.toml \
  --root . \
  --format json > "${work}/aggregate.json" || aggregate_status=$?

# Retain the aggregate artifact even when the aggregate result is an
# instrument failure: the evidence is never masked by the exit.
if [[ -s "${work}/aggregate.json" ]]; then
  cp "${work}/aggregate.json" workflow-construction-aggregate.json
fi

echo "check-workflow-construction: aggregate emitted at workflow-construction-aggregate.json"
cat workflow-construction-aggregate.json 2>/dev/null || true

if [[ "${aggregate_status:-0}" -ne 0 ]]; then
  echo "check-workflow-construction: aggregate instrument failure" >&2
  exit 1
fi
