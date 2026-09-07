#!/usr/bin/env bash
# Bounded direct-floor proof for the cargo-allow release set (#3903 PR B).
#
# Constructs the exact direct-floor candidate: a detached worktree of the
# merged HEAD whose Cargo.lock is regenerated and then pinned so every
# external direct dependency sits at its declared requirement's minimum
# version. Runs the bounded proof classes (check, test, package) under the
# claimed MSRV toolchain and emits the typed receipt.
#
# Read-only over the live tree: all mutation happens inside a temporary
# worktree that is removed on exit. No dependency requirement is changed,
# nothing is published, and no live lock is touched.
#
# Environment:
#   CI_PROOF_MSRV      claimed MSRV (default 1.95)
#   CI_PROOF_OUT       receipt JSON output path (required; resolved to an
#                      absolute path before the worktree is entered)
#   CI_PROOF_CLASSES   comma-separated bounded classes (required; must
#                      contain at least one known class)

set -euo pipefail

MSRV="${CI_PROOF_MSRV:-1.95}"
PRODUCT="${CI_PROOF_PRODUCT:-cargo-allow}"
# Report-only products stay advisory: check is the bounded proof class;
# test/package enforcement requires the owning package-family authority.
# An explicit CI_PROOF_CLASSES still wins.
ADVISORY="${CI_PROOF_ADVISORY:-false}"
CLASSES="${CI_PROOF_CLASSES:-}"
if [ -z "$CLASSES" ]; then
  if [[ "$ADVISORY" == "true" ]]; then
    CLASSES="check"
  else
    CLASSES="check,test,package"
  fi
fi
ROOT="$(git rev-parse --show-toplevel)"

# Validate the class selection before anything runs: an empty or unknown
# selection would skip every proof and still certify the rows (negative
# control 10).
IFS=',' read -r -a CLASS_LIST <<< "$CLASSES"
known_class() {
  case "$1" in
    check | test | package) return 0 ;;
    *) return 1 ;;
  esac
}
selected_any=false
for class in "${CLASS_LIST[@]}"; do
  known_class "$class" || {
    echo "proof-direct-floors: unknown proof class '$class'" >&2
    exit 1
  }
  selected_any=true
done
"$selected_any" || {
  echo "proof-direct-floors: no proof class selected" >&2
  exit 1
}

# Resolve the output path before the worktree changes the working directory.
mkdir -p "$(dirname "$CI_PROOF_OUT")"
OUT="$(cd "$(dirname "$CI_PROOF_OUT")" && pwd)/$(basename "$CI_PROOF_OUT")"

WORKTREE="$(mktemp -d)"
cleanup() {
  git worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git worktree add --detach "$WORKTREE" HEAD >/dev/null
cd "$WORKTREE"

# The MSRV toolchain compiles the proof: a newer toolchain would break
# negative control 7 (a newer Rust must not satisfy the rows silently).
export RUSTUP_TOOLCHAIN="$MSRV"
rustc --version
cargo --version

# Identity digests computed from the proof inputs: the manifest set is the
# root manifest plus every member manifest; the lock identity is the HEAD
# Cargo.lock the floor candidate starts from.
manifest_set_digest="$(
  {
    cat "$ROOT/Cargo.toml"
    find crates -maxdepth 2 -name Cargo.toml | sort | xargs cat
  } | sha256sum | cut -d' ' -f1
)"
lock_digest="$(sha256sum "$ROOT/Cargo.lock" | cut -d' ' -f1)"

# 1. Derive the external direct dependency floors from the root
#    manifest's [workspace.dependencies] (registry deps only; workspace
#    path deps with exact =x.y.z pins are proven by the workspace build
#    itself).
python3 - "$ROOT/Cargo.toml" > floors.json <<'PY'
import json
import sys
import tomllib

data = tomllib.load(open(sys.argv[1], "rb"))
deps = data["workspace"]["dependencies"]
rows = []
for name, spec in sorted(deps.items()):
    if isinstance(spec, str):
        requirement = spec
    elif isinstance(spec, dict) and "version" in spec and "path" not in spec and "git" not in spec:
        requirement = spec["version"]
    else:
        continue
    parts = requirement.split(".")
    while len(parts) < 3:
        parts.append("0")
    floor = ".".join(parts[:3])
    rows.append({"package": name, "requirement": requirement, "floor": floor})
print(json.dumps(rows, indent=1))
PY
floor_count="$(jq length floors.json)"
if [ "$floor_count" -eq 0 ]; then
  echo "proof-direct-floors: empty floor inventory; nothing to certify" >&2
  exit 1
fi

# 2. Build the direct-floor candidate lock: regenerate from scratch, then
#    pin each external direct dependency to its declared minimum. Pin
#    failures (version does not exist or is yanked) are recorded as
#    resolver failures for those rows and fail the overall proof.
rm -f Cargo.lock
python3 - <<'PY'
import json
import subprocess

floors = json.load(open("floors.json", encoding="utf-8"))
failures = {}
for row in floors:
    proc = subprocess.run(
        ["cargo", "update", "-p", row["package"], "--precise", row["floor"]],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        failures[row["package"]] = proc.stderr.strip()[:300]
json.dump(failures, open("pin-failures.json", "w"), indent=1)
PY
pin_failures="$(cat pin-failures.json)"
# Verify every pin that succeeded actually landed at the floor. Packages
# whose pin failed are excluded here — they surface as resolver_failure
# rows in the receipt instead of aborting before any receipt exists.
floor_move_failures="$(python3 - <<'PY'
import json
import tomllib

lock = tomllib.load(open("Cargo.lock", "rb"))
floors = json.load(open("floors.json", encoding="utf-8"))
pin_failed = set(json.load(open("pin-failures.json", encoding="utf-8")).keys())
resolved = {}
for package in lock.get("package", []):
    resolved.setdefault(package["name"], package["version"])
moved = []
for row in floors:
    if row["package"] in pin_failed:
        continue
    # Cargo can record a registry version with build metadata (e.g.
    # 1.1.4+spec-1.1.0); SemVer precedence ignores it, so compare the
    # version without the suffix.
    locked_base = (resolved.get(row["package"]) or "").split("+")[0]
    locked = locked_base
    if locked != row["floor"]:
        moved.append(row["package"] + ": locked at " + str(locked) + ", floor requires " + str(row["floor"]))
print("; ".join(moved))
PY
)"
if [ -n "$floor_move_failures" ]; then
  echo "proof-direct-floors: pinned floors silently moved:" >&2
  echo "$floor_move_failures" >&2
  exit 6
fi

# 3. Bounded proof classes.
check_status=0
test_status=0
package_status=0
if [[ "$CLASSES" == *check* ]]; then
  cargo check --locked --workspace || check_status=$?
fi
if [[ "$CLASSES" == *test* ]]; then
  cargo test -p cargo-allow --locked || test_status=$?
fi
if [[ "$CLASSES" == *package* ]]; then
  cargo package -p allow-core --locked --no-verify --allow-dirty \
    --target-dir target/package-proof || package_status=$?
fi

# 4. Emit the typed receipt: resolved versions from the floor lock; pin
#    failures become resolver failures; a failed proof class becomes an
#    instrument failure for every row (the failing crate is in the
#    captured output).
PRODUCT="$PRODUCT" python3 - "$WORKTREE/Cargo.lock" "$WORKTREE/floors.json" "$WORKTREE/pin-failures.json" \
  "$check_status" "$test_status" "$package_status" "$MSRV" \
  "$manifest_set_digest" "$lock_digest" > "$OUT" <<'PY'
import hashlib
import json
import os
import sys
import tomllib

lock_path, floors_path, pins_path = sys.argv[1], sys.argv[2], sys.argv[3]
check_status, test_status, package_status = (int(v) for v in sys.argv[4:7])
msrv = sys.argv[7]
manifest_set_digest, lock_digest = sys.argv[8], sys.argv[9]
product = os.environ.get("PRODUCT", "cargo-allow")

lock = tomllib.load(open(lock_path, "rb"))
floors = json.load(open(floors_path, encoding="utf-8"))
pin_failures = json.load(open(pins_path, encoding="utf-8"))
resolved = {}
for package in lock.get("package", []):
    resolved.setdefault(package["name"], package["version"])

class_failed = bool(check_status or test_status or package_status)
rows = []
for row in floors:
    name = row["package"]
    in_workspace = row["requirement"].startswith("=")
    if name in pin_failures:
        result = "resolver_failure"
        limitation = f"pin to the declared floor failed: {pin_failures[name]}"
    elif class_failed:
        result = "instrument_failure"
        limitation = "a bounded proof class failed at the floors; see the proof output"
    else:
        result = "proven"
        limitation = None
    rows.append({
        "package": name,
        "declared_requirement": row["requirement"],
        "tested_floor": row["floor"],
        "resolved_version": resolved.get(name, row["floor"]),
        "source_identity": (
            "workspace:path" if in_workspace else "registry:crates.io"
        ),
        "result": result,
        "limitation": limitation,
    })

lock_bytes = open(lock_path, "rb").read()
floor_lock_digest = "sha256:v1:" + hashlib.sha256(lock_bytes).hexdigest()

receipt = {
    "schema_id": "cargo-allow.minimum-direct-version.v1",
    "schema_version": 1,
    "product": product,
    "msrv": msrv,
    "toolchain": msrv + ".0",
    "target": "host (release-set default target)",
    "manifest_set_digest": manifest_set_digest,
    "lock_digest": lock_digest,
    "rows": rows,
    "commands": [
        "cargo update -p <dep> --precise <floor> (per external direct dep)",
        "cargo check --locked --workspace",
        "cargo test -p cargo-allow --locked",
        "cargo package -p allow-core --locked --no-verify --allow-dirty",
    ],
    "floor_lock_digest": floor_lock_digest,
    "limitations": [
        "bounded proof: the release-set test subset, not every target or feature combination",
        "internal =0.2.0 workspace pins are proven by the same workspace build",
    ],
    "claim_boundary": "Exact selected proof of declared direct dependency floors for the cargo-allow release set at the claimed MSRV. Transitive minimal combinations are not certified.",
}
print(json.dumps(receipt, indent=1))
PY

overall=0
if [[ "$check_status" -ne 0 || "$test_status" -ne 0 || "$package_status" -ne 0 ]]; then
  overall=1
fi
if [[ -s "$WORKTREE/pin-failures.json" ]] &&
  [[ "$(cat "$WORKTREE/pin-failures.json")" != "{}" ]]; then
  overall=1
fi
echo "proof-direct-floors: proof classes completed (overall=$overall)"
exit "$overall"
