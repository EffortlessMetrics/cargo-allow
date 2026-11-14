#!/usr/bin/env bash
# Exercises producer decisions and real-Git subject admission, never builds.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 scripts/test_floor_selection.py
python3 scripts/test_floor_pin_settlement.py
python3 scripts/test_floor_product_workspace.py
python3 scripts/test_floor_execution_identity.py
python3 -B scripts/test_floor_workspace_contract.py
python3 scripts/test_floor_source_identity.py --rehearsal-script scripts/release-rehearsal.py
python3 scripts/test-release-rehearsal.py TestRehearsalSubjectBinding TestReceiptOutput
python3 scripts/test-release-rehearsal.py \
  TestZeroMutationDrills.test_tag_worker_observes_the_denial_but_never_claims_prevention \
  TestZeroMutationDrills.test_tag_flag_follows_the_measurement_not_a_literal \
  TestZeroMutationDrills.test_direct_routes_keep_unconfined_families_false
python3 scripts/test_floor_protocol.py --bash "$BASH"
