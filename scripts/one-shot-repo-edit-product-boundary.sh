#!/usr/bin/env bash
set -euo pipefail

branch="refactor/repo-edit-product-edge-2969"

git fetch origin "${branch}" main
git checkout -B one-shot-repo-edit "origin/${branch}"
git merge --no-edit -X theirs origin/main

# Refresh workspace-package dependency metadata before asking rustc to locate
# the product projection sites.
cargo metadata --format-version 1 >/dev/null

python3 - <<'PY'
import json
import subprocess
from collections import defaultdict
from pathlib import Path

adapter = Path("crates/cargo-allow/src/extraction_repo_edit_runtime.rs")
text = adapter.read_text(encoding="utf-8")
function = '''
/// Project a neutral repository-edit failure into cargo-allow's product error surface.
pub(crate) fn map_repo_edit_error(
    error: effortless_repo_edit::RepoEditError,
) -> CargoAllowError {
    let message = error.to_string();
    let kind = if message.contains("outside")
        || message.contains("not inside")
        || message.contains("escape")
    {
        allow_core::CargoAllowErrorKind::InvalidConfig
    } else {
        allow_core::CargoAllowErrorKind::Artifact
    };
    CargoAllowError::with_kind(kind, message)
}

'''
if "fn map_repo_edit_error(" not in text:
    insertion = text.find("#[cfg(test)]")
    if insertion < 0:
        insertion = len(text)
    text = text[:insertion] + function + text[insertion:]
    adapter.write_text(text, encoding="utf-8")

projection = ".map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)"
command = [
    "cargo", "check", "--workspace", "--all-targets", "--locked",
    "--message-format=json",
]

for _attempt in range(20):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode == 0:
        break

    sites = defaultdict(set)
    other_errors = []
    for raw in result.stdout.splitlines():
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if event.get("reason") != "compiler-message":
            continue
        message = event.get("message", {})
        rendered = message.get("rendered") or ""
        code = (message.get("code") or {}).get("code")
        if code == "E0277" and "RepoEditError" in rendered:
            primary = next(
                (span for span in message.get("spans", []) if span.get("is_primary")),
                None,
            )
            if primary is None:
                raise SystemExit("RepoEditError diagnostic had no primary span")
            file_name = primary["file_name"]
            if not file_name.startswith("crates/cargo-allow/src/"):
                raise SystemExit(f"unexpected RepoEditError product site: {file_name}")
            sites[file_name].add(
                (
                    int(primary["line_start"]),
                    int(primary["column_start"]),
                    int(primary["column_end"]),
                )
            )
        elif message.get("level") == "error":
            other_errors.append(rendered)

    if not sites:
        print(result.stderr)
        print("\n".join(other_errors))
        raise SystemExit(
            "workspace check failed without a mappable RepoEditError diagnostic"
        )

    for file_name, file_sites in sites.items():
        path = Path(file_name)
        lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
        for line_no, column_start, column_end in sorted(file_sites, reverse=True):
            index = line_no - 1
            line = lines[index]
            if projection in line:
                continue
            candidates = [position for position, char in enumerate(line) if char == "?"]
            if not candidates:
                raise SystemExit(
                    f"no ? found at {file_name}:{line_no}: {line.rstrip()}"
                )
            target = max(column_start - 1, column_end - 1)
            position = min(candidates, key=lambda value: abs(value - target))
            lines[index] = line[:position] + projection + line[position:]
        path.write_text("".join(lines), encoding="utf-8")

    subprocess.run(["cargo", "fmt", "--all"], check=True)
else:
    raise SystemExit("repo-edit product projection did not converge")

final = subprocess.run(command)
if final.returncode != 0:
    raise SystemExit(final.returncode)
PY

cargo clippy --fix --workspace --all-targets --locked --allow-dirty --allow-staged -- -D warnings
cargo fmt --all
cargo metadata --format-version 1 >/dev/null

# Remove all temporary automation before proving or committing the product diff.
git checkout origin/main -- scripts/check-msrv-consistency.sh
rm -f scripts/one-shot-repo-edit-product-boundary.sh
rm -f .github/workflows/one-shot-repo-edit-product-adapter.yml

cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p effortless-repo-edit --locked
cargo test -p cargo-allow --bins --locked
cargo run -p cargo-allow -- check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
git diff --check

git add -A
git diff --cached --check

git config user.name "EffortlessSteven"
git config user.email "git@effortlesssteven.com"

git commit -m "refactor(repo-edit): project errors at cargo-allow boundary"
git push origin HEAD:"${branch}"
