#!/usr/bin/env bash
set -euo pipefail

branch="fix/intent-protocol-ledger-after-3463"

git config user.name "EffortlessSteven"
git config user.email "git@effortlesssteven.com"
git fetch origin "${branch}" main
git checkout -B one-shot-intent-protocol-ledger-after-3463 "origin/${branch}"

python3 - <<'PY'
from pathlib import Path

path = Path("policy/product-move-ledger.toml")
text = path.read_text(encoding="utf-8")
for deleted in [
    "crates/intent-protocol/src/snapshot_package/repo_protocol/repository_snapshot.rs",
    "crates/intent-protocol/src/snapshot_package/repo_protocol/result_class.rs",
]:
    quoted = f'"{deleted}"'
    if quoted not in text:
        raise SystemExit(f"movement authority does not contain expected deleted path {deleted}")
    text = text.replace(f'{quoted}, ', "", 1)
    text = text.replace(f', {quoted}', "", 1)
path.write_text(text, encoding="utf-8")
PY

cargo test -p allow-policy regenerate_product_move_map_projection -- --ignored

git checkout origin/main -- scripts/check-msrv-consistency.sh
rm -f scripts/one-shot-intent-protocol-ledger-after-3463.sh

cargo fmt --all -- --check
cargo test -p allow-policy repository_move_ledger_is_complete_and_projection_is_current --locked
cargo test -p cargo-allow --bin cargo-allow product_move_ledger --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
git diff --check

git add -A
git diff --cached --check
git commit -m "fix(intent-protocol): retire deleted copied paths from movement authority"
git push origin HEAD:"${branch}"
