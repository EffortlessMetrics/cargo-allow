#!/usr/bin/env bash
# ExactCandidateInstallJourneyV1: owned lifecycle and validated receipt publication.
# Optional PACKAGE_SET_DIR and WORK_DIR must be separate children of target.
# WORK_DIR and an explicit CONSUMER_DIR must not already exist.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "${ROOT}/scripts/exact-candidate-install-journey.py"
