#!/usr/bin/env bash
set -euo pipefail

branch="refactor/intent-protocol-canonical-repo-3387"

git config user.name "EffortlessSteven"
git config user.email "git@effortlesssteven.com"

git fetch origin "${branch}" main
git checkout -B one-shot-intent-protocol-ledger "origin/${branch}"

python3 - <<'PY'
from pathlib import Path

path = Path("policy/product-move-ledger.toml")
text = path.read_text(encoding="utf-8")
old = 'current_paths = ["crates/intent-protocol/src/closure.rs", "crates/intent-protocol/src/diff.rs", "crates/intent-protocol/src/identity.rs", "crates/intent-protocol/src/lib.rs", "crates/intent-protocol/src/obligation.rs", "crates/intent-protocol/src/parity/mod.rs", "crates/intent-protocol/src/query.rs", "crates/intent-protocol/src/snapshot_package/mod.rs", "crates/intent-protocol/src/snapshot_package/repo_protocol/mod.rs", "crates/intent-protocol/src/snapshot_package/repo_protocol/repository_snapshot.rs", "crates/intent-protocol/src/snapshot_package/repo_protocol/result_class.rs", "crates/intent-protocol/src/tests.rs", "crates/intent-protocol/src/view.rs"]'
new = 'current_paths = ["crates/intent-protocol/src/closure.rs", "crates/intent-protocol/src/diff.rs", "crates/intent-protocol/src/identity.rs", "crates/intent-protocol/src/lib.rs", "crates/intent-protocol/src/obligation.rs", "crates/intent-protocol/src/parity/mod.rs", "crates/intent-protocol/src/query.rs", "crates/intent-protocol/src/tests.rs", "crates/intent-protocol/src/view.rs"]'
if old not in text:
    raise SystemExit("intent-protocol inventory row did not match expected authority")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
PY

cargo test -p allow-policy regenerate_product_move_map_projection -- --ignored

# Remove branch-local automation before proving and committing the authority diff.
git checkout origin/main -- scripts/check-msrv-consistency.sh
rm -f scripts/one-shot-intent-protocol-ledger-finish.sh

cargo fmt --all -- --check
cargo test -p allow-policy repository_move_ledger_is_complete_and_projection_is_current --locked
cargo test -p cargo-allow --bin cargo-allow product_move_ledger --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
git diff --check

git add -A
git diff --cached --check
git commit -m "refactor(intent-protocol): retire copied repository authority"
git push origin HEAD:"${branch}"
