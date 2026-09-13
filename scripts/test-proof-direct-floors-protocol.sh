#!/usr/bin/env bash
# Exercises producer decisions and real-Git subject admission, never builds.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 scripts/test_floor_selection.py
python3 scripts/test_floor_execution_identity.py
python3 scripts/test_floor_source_identity.py --rehearsal-script scripts/release-rehearsal.py
python3 scripts/test-release-rehearsal.py TestRehearsalSubjectBinding
python3 scripts/test_floor_protocol.py --bash "$BASH"
