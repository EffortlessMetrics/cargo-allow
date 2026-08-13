#!/usr/bin/env bash
# Offline characterization for ExactCandidateInstallJourneyV1 (#3357).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

validator="${ROOT}/scripts/validate-exact-candidate-install-journey.py"
schema="${ROOT}/docs/dogfood/fixtures/release/exact-candidate-install-journey.v1.schema.json"
fixture="${ROOT}/docs/dogfood/fixtures/release/candidate-crate-set.toml"
example="${ROOT}/docs/dogfood/receipts/exact-candidate-install-journey-pass.example.json"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-exact-candidate-receipt-test.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

python3 "${validator}" final \
  --receipt "${example}" \
  --schema "${schema}" \
  --fixture "${fixture}"

python3 - "${example}" "${work_dir}/forged.json" <<'PY'
import json
import sys
from pathlib import Path

source = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
source["provenance"].pop("candidate_fixture_sha256")
Path(sys.argv[2]).write_text(json.dumps(source), encoding="utf-8")
PY
if python3 "${validator}" final \
  --receipt "${work_dir}/forged.json" \
  --schema "${schema}" \
  --fixture "${fixture}"; then
  printf 'forged receipt unexpectedly validated\n' >&2
  exit 1
fi

printf 'ok exact-candidate-install-journey schema and negative characterization\n'
