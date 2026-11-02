#!/usr/bin/env bash
# Dependency graph delta review surface (#3920 PR D): compiles one
# exact base/head manifest/lockfile pair, enriches it with the linked
# evidence authorities, and retains both artifacts for review.
#
# Exit contract (calibration law): ordinary graph movement stays
# review-required/advisory and exits zero until the owning policy
# selects a blocking consequence; unreadable, empty, or malformed
# required inputs fail immediately and exit nonzero. The job runs on
# every pull request but exits early when no manifest or lockfile
# input changed, so ordinary dependency-free PRs pay almost nothing.

set -euo pipefail

work="${TARGET_DIR:-target}/dependency-graph-delta"
mkdir -p "$work"

BASE="${GITHUB_BASE_SHA:-}"
if [ -z "$BASE" ]; then
  BASE="$(git merge-base origin/main HEAD 2>/dev/null || git rev-parse origin/main)"
fi

# Resolve immutable subjects before routing or acquiring any input. A supplied
# base must never silently fall back to a different comparison.
BASE_COMMIT="$(git rev-parse --verify --end-of-options "${BASE}^{commit}")" \
  || { echo "check-dependency-graph-delta: base commit unavailable: $BASE" >&2; exit 1; }
HEAD_COMMIT="$(git rev-parse --verify HEAD^{commit})"

# Routing economy: only manifest, lockfile, topology, feature, or
# dependency-policy inputs can move the graph.
if git diff --quiet "$BASE_COMMIT" "$HEAD_COMMIT" -- \
  'Cargo.lock' \
  'Cargo.toml' \
  '*/Cargo.toml' \
  'crates/*/Cargo.toml' \
  'policy/product-package-topology*.toml' \
  'policy/feature-configuration*.toml' \
  'policy/dependency-*.toml' 2>/dev/null; then
  echo "check-dependency-graph-delta: no manifest/lock/policy inputs changed vs $BASE; not applicable"
  exit 0
else
  diff_status=$?
  [ "$diff_status" -eq 1 ] || {
    echo "check-dependency-graph-delta: input routing failed (git diff exit $diff_status)" >&2
    exit 1
  }
fi

echo "check-dependency-graph-delta: compiling delta against base $BASE_COMMIT"
# The declared commit identities govern BOTH sides. Never mix working-tree
# root/member files, the index, or untracked crates into a committed receipt.
git show "$BASE_COMMIT:Cargo.toml" > "$work/base-root-Cargo.toml"
git show "$BASE_COMMIT:Cargo.lock" > "$work/base-Cargo.lock"
git show "$HEAD_COMMIT:Cargo.toml" > "$work/head-root-Cargo.toml"
git show "$HEAD_COMMIT:Cargo.lock" > "$work/head-Cargo.lock"

# Enumerate only committed crates/<member>/Cargo.toml blobs at the existing
# bounded depth. NUL framing preserves spaces, tabs and newlines in Git paths.
member_manifests() {
  python3 - "$1" "$2" <<'PY_MEMBERS'
import subprocess
import sys
from pathlib import Path

raw = subprocess.check_output(["git", "ls-tree", "-rz", "--name-only", sys.argv[1], "--", "crates/"])
paths = []
for path in raw.split(b"\0"):
    parts = path.split(b"/")
    if len(parts) == 3 and parts[0] == b"crates" and parts[2] == b"Cargo.toml":
        # Source paths and TOML are UTF-8 inputs to this bounded compiler.
        # Refuse unrepresentable paths rather than rewriting their identity.
        path.decode("utf-8")
        paths.append(path)
Path(sys.argv[2]).write_bytes(b"".join(path + b"\0" for path in sorted(paths)))
PY_MEMBERS
}
member_manifests "$BASE_COMMIT" "$work/base-member-manifests.txt"
member_manifests "$HEAD_COMMIT" "$work/head-member-manifests.txt"

# Synthesize one merged manifest per side: the root's
# [workspace.dependencies] (normalized inline) plus every member's
# [dependencies] entries, deduplicated with deterministic conflict
# resolution (smallest normalized spec wins). This is the
# direct-requirement denominator; dev/build classes stay outside the
# bounded compiler and are named in the receipt limitations.
synthesize() {
  python3 - "$1" "$2" "$3" "$4" <<'PY'
import subprocess
import sys
import tomllib

def load(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)

def show_member(commit, path):
    # Enumeration already proved this path exists at the selected commit.
    # Any failed read/decode/parse is an instrumentation failure, not an
    # absent member or a replacement-character approximation of its bytes.
    raw = subprocess.check_output(["git", "show", f"{commit}:{path}"])
    return tomllib.loads(raw.decode("utf-8"))

def inline(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        escaped = value.replace("\\", "\\\\").replace('"', '\\"')
        return f'"{escaped}"'
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(inline(item) for item in value) + "]"
    if isinstance(value, dict):
        body = ", ".join(f"{key} = {inline(item)}" for key, item in sorted(value.items()))
        return "{ " + body + " }"
    raise SystemExit(f"unsupported spec type: {type(value)!r}")

root = load(sys.argv[1])
workspace_deps = root.get("workspace", {}).get("dependencies", {})

merged = {}
with open(sys.argv[2], "rb") as handle:
    manifest_paths = [path.decode("utf-8") for path in handle.read().split(b"\0") if path]
for member_path in sorted(manifest_paths):
    doc = show_member(sys.argv[4], member_path)
    for name, spec in doc.get("dependencies", {}).items():
        normalized = inline(spec)
        prior = merged.get(name)
        if prior is None or normalized < prior:
            merged[name] = normalized

with open(sys.argv[3], "w", encoding="utf-8", newline="\n") as handle:
    handle.write("[workspace.dependencies]\n")
    for name, spec in sorted(workspace_deps.items()):
        handle.write(f"{name} = {inline(spec)}\n")
    handle.write("\n[dependencies]\n")
    for name, spec in sorted(merged.items()):
        handle.write(f"{name} = {spec}\n")
PY
}

synthesize "$work/base-root-Cargo.toml" "$work/base-member-manifests.txt" "$work/base-Cargo.toml" "$BASE_COMMIT"
synthesize "$work/head-root-Cargo.toml" "$work/head-member-manifests.txt" "$work/head-Cargo.toml" "$HEAD_COMMIT"

cargo run -q -p cargo-allow -- dependency-graph-delta compile \
  --base-manifest "$work/base-Cargo.toml" \
  --head-manifest "$work/head-Cargo.toml" \
  --base-lock "$work/base-Cargo.lock" \
  --head-lock "$work/head-Cargo.lock" \
  --base-commit "$BASE_COMMIT" \
  --head-commit "$HEAD_COMMIT" \
  --output "$work/delta.json"

# Calibration bundle: no authority records are claimed yet. Rows land
# on observed_movement and the receipt reports decision_required,
# which is the advisory posture PR D publishes until the owning
# policy selects blocking consequences. The bundle's identity fields
# are derived from the compiled receipt so the exact-input binding
# holds; producers can hand a real bundle via DEPENDENCY_GRAPH_BUNDLE.
if [ -n "${DEPENDENCY_GRAPH_BUNDLE:-}" ]; then
  cp "$DEPENDENCY_GRAPH_BUNDLE" "$work/bundle.json"
else
  python3 - "$work/delta.json" "$work/bundle.json" <<'PY'
import json
import sys

delta = json.load(open(sys.argv[1], encoding="utf-8"))
identity = delta["identity"]
bundle = {
    "authorities_in_scope": [],
    "base_commit": identity["base_commit"],
    "head_commit": identity["head_commit"],
    "base_manifest_set_digest": identity["base_manifest_set_digest"],
    "head_manifest_set_digest": identity["head_manifest_set_digest"],
    "base_lock_digest": identity["base_lock_digest"],
    "head_lock_digest": identity["head_lock_digest"],
    "product": identity["product"],
    "target": identity["target"],
    "records": [],
}
with open(sys.argv[2], "w", encoding="utf-8", newline="\n") as handle:
    json.dump(bundle, handle, indent=1)
    handle.write("\n")
PY
fi

cargo run -q -p cargo-allow -- dependency-graph-evidence evaluate \
  --delta "$work/delta.json" \
  --bundle "$work/bundle.json" \
  --format human \
  --output "$work/evidence.md"

cp "$work/delta.json" dependency-graph-delta.json
cp "$work/evidence.md" dependency-graph-evidence-summary.md

echo "----- compact review surface -----"
cat dependency-graph-evidence-summary.md
echo "----------------------------------"

# Calibration law: every well-formed outcome — including
# decision_required — exits zero. Only the instrument-failure exits
# above are blocking, and PR D publishes the retained artifact for
# the owning policy to calibrate consequences against.
echo "check-dependency-graph-delta: bounded-denominator limitations:" >&2
echo "  - member requirement specs collapse by dependency name; per-member manifest edits that do not move the workspace-wide union or the lockfile are outside this lane" >&2
echo "  - duplicate lockfile versions are distinguished by (version, source) identity (#4243); what remains unexpressed is only the dependency edge that pulled a specific resolved row (edge provenance, #4244)" >&2
echo "check-dependency-graph-delta: delta and evidence artifacts retained (advisory)"
