#!/usr/bin/env bash
# Exercises producer decisions with synthetic process observations, never builds.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 scripts/test_floor_selection.py
python3 scripts/test_floor_execution_identity.py
python3 scripts/test_floor_protocol.py --bash "$BASH"
