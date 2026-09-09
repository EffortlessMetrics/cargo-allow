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

# Routing economy: only manifest, lockfile, topology, feature, or
# dependency-policy inputs can move the graph.
if git diff --quiet "$BASE" HEAD -- \
  'Cargo.lock' \
  'Cargo.toml' \
  '*/Cargo.toml' \
  'crates/*/Cargo.toml' \
  'policy/product-package-topology*.toml' \
  'policy/feature-configuration*.toml' \
  'policy/dependency-*.toml' 2>/dev/null; then
  echo "check-dependency-graph-delta: no manifest/lock/policy inputs changed vs $BASE; not applicable"
  exit 0
fi

echo "check-dependency-graph-delta: compiling delta against base $BASE"
git show "$BASE:Cargo.toml" > "$work/base-root-Cargo.toml" 2>/dev/null \
  || { echo "check-dependency-graph-delta: base manifest unavailable at $BASE" >&2; exit 1; }
git show "$BASE:Cargo.lock" > "$work/base-Cargo.lock" 2>/dev/null \
  || { echo "check-dependency-graph-delta: base lock unavailable at $BASE" >&2; exit 1; }
cp Cargo.toml "$work/head-root-Cargo.toml"
cp Cargo.lock "$work/head-Cargo.lock"

# Member list is the bounded depth-2 workspace layout; the same rule
# enumerates both sides so the denominator cannot drift.
git ls-tree --name-only "$BASE" crates/ > "$work/base-members.txt" 2>/dev/null \
  || { echo "check-dependency-graph-delta: base member list unavailable" >&2; exit 1; }
find crates -maxdepth 2 -name Cargo.toml > "$work/head-member-manifests.txt" 2>/dev/null || : > "$work/head-member-manifests.txt"
sed 's#$#/Cargo.toml#' "$work/base-members.txt" > "$work/base-member-manifests.txt"

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

def show_base(commit, path):
    raw = subprocess.run(["git", "show", f"{commit}:{path}"], capture_output=True)
    if raw.returncode != 0:
        return None
    return tomllib.loads(raw.stdout.decode("utf-8", errors="replace"))

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
manifest_paths = [line.strip() for line in open(sys.argv[2], encoding="utf-8") if line.strip()]
for member_path in sorted(manifest_paths):
    doc = show_base(sys.argv[4], member_path) if sys.argv[4] != "HEAD" else None
    if doc is None and sys.argv[4] == "HEAD":
        # A missing or unparseable head manifest is a broken review
        # tree: fail closed instead of compiling an incomplete set.
        try:
            doc = load(member_path)
        except (OSError, tomllib.TOMLDecodeError) as error:
            raise SystemExit(f"head member manifest {member_path} fails to load: {error}")
    if doc is None:
        # Base side: a member absent from the base tree is a crate
        # added by this PR; nothing to merge for it.
        continue
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

synthesize "$work/base-root-Cargo.toml" "$work/base-member-manifests.txt" "$work/base-Cargo.toml" "$BASE"
synthesize "$work/head-root-Cargo.toml" "$work/head-member-manifests.txt" "$work/head-Cargo.toml" HEAD

BASE_COMMIT="$(git rev-parse "$BASE")"
HEAD_COMMIT="$(git rev-parse HEAD)"

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
echo "  - duplicate lockfile versions are keyed by package name; movement in a shadowed duplicate version needs the follow-up keyed-by-full-identity compiler" >&2
echo "check-dependency-graph-delta: delta and evidence artifacts retained (advisory)"
