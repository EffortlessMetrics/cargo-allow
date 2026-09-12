#!/usr/bin/env bash
# Product-scoped direct-floor proof for the #3903 release set (PR B) and
# its advisory product rows (PR C).
#
# For the selected product (CI_PROOF_PRODUCT) the script derives that
# product's package closure from the checked-in manifests, inventories
# the closure's declared external direct dependencies, pins each to its
# requirement minimum inside a detached worktree, and runs the bounded
# proof classes against the product's own packages under the claimed
# MSRV toolchain. A receipt names only the selected product's floors,
# roots, and actually executed commands.
#
# Read-only over the live tree: all mutation happens inside a temporary
# worktree that is removed on exit. No dependency requirement is changed,
# nothing is published, and no live lock is touched.
#
# Environment:
#   CI_PROOF_PRODUCT   product whose closure is proved (default
#                      cargo-allow; one of the registered products)
#   CI_PROOF_MSRV      claimed MSRV (default 1.95)
#   CI_PROOF_OUT       receipt JSON output path (required; resolved to an
#                      absolute path before the worktree is entered)
#   CI_PROOF_CLASSES   comma-separated bounded classes (required; must
#                      contain at least one known class)
#   CI_PROOF_ADVISORY  when true and no class selection is given, the
#                      bounded classes default to check only (report-only
#                      products: test/package enforcement requires the
#                      owning package-family authority)

set -euo pipefail

# Windows jq builds emit CRLF; native cargo rejects a trailing CR in a
# package name. Strip the CR from every jq read so the proof inputs and
# the -p argument lists stay byte-clean on every platform.
jq() {
  command jq "$@" | tr -d '\r'
}

MSRV="${CI_PROOF_MSRV:-1.95}"
PRODUCT="${CI_PROOF_PRODUCT:-cargo-allow}"
ADVISORY="${CI_PROOF_ADVISORY:-false}"
CLASSES="${CI_PROOF_CLASSES:-}"
if [ -z "$CLASSES" ]; then
  if [[ "$ADVISORY" == "true" ]]; then
    CLASSES="check"
  else
    CLASSES="check,test,package"
  fi
fi

# The product registry: each product maps to the package roots whose
# dependency closure the receipt certifies. An unregistered product must
# fail closed — a label-only override would mint a receipt that names a
# product its inputs never covered.
product_roots() {
  case "$1" in
    cargo-allow)
      printf '%s\n' "cargo-allow"
      ;;
    shared)
      printf '%s\n' \
        "effortless-repo-protocol" \
        "effortless-repo-snapshot" \
        "effortless-repo-edit" \
        "effortless-rust-source-index"
      ;;
    cargo-intent)
      printf '%s\n' "cargo-intent"
      ;;
    cargo-proof)
      printf '%s\n' "cargo-proof"
      ;;
    *)
      return 1
      ;;
  esac
}
if ! product_roots "$PRODUCT" >/dev/null; then
  echo "proof-direct-floors: unknown product '$PRODUCT' (registered: cargo-allow, shared, cargo-intent, cargo-proof)" >&2
  exit 1
fi
mapfile -t ROOTS < <(product_roots "$PRODUCT")

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

SOURCE_COMMIT="$(git rev-parse HEAD)"
git worktree add --detach "$WORKTREE" "$SOURCE_COMMIT" >/dev/null
cd "$WORKTREE"
mkdir -p target/floor-proof

# The MSRV toolchain compiles the proof: a newer toolchain would break
# negative control 7 (a newer Rust must not satisfy the rows silently).
export RUSTUP_TOOLCHAIN="$MSRV"
# This collector owns tool selection. Explicit tool paths and empty wrappers
# override both inherited variables and Cargo's build.* configuration.
native_tool_path() {
  python3 -c 'import os, shutil, sys; path = os.path.abspath(sys.argv[1]); path = path if os.path.isfile(path) else shutil.which(path); path or sys.exit("selected tool is not a file"); print(os.path.abspath(path))' "$(command -v "$1")"
}
RUSTC="$(native_tool_path rustc)"
RUSTDOC="$(native_tool_path rustdoc)"
export RUSTC RUSTDOC
export RUSTC_WRAPPER=""
export RUSTC_WORKSPACE_WRAPPER=""
python3 scripts/floor_execution_identity.py "$MSRV" > target/floor-proof/execution-identity.json
host_target="$(jq -r '.host' target/floor-proof/execution-identity.json)"
cat target/floor-proof/execution-identity.json

# Identity digests are bound to the detached worktree's own inputs — the
# exact manifests and the HEAD Cargo.lock the floor candidate starts
# from — so uncommitted live-tree state can never certify a receipt.
# The digests are line-ending independent: CR bytes are stripped from
# the hashed stream so a CRLF checkout (Windows autocrlf) hashes the
# same identity as an LF checkout.
manifest_set_digest="$(
  python3 - <<'PY'
import hashlib
from pathlib import Path
import tomllib

root_manifest = Path("Cargo.toml")
workspace = tomllib.loads(root_manifest.read_text(encoding="utf-8"))["workspace"]
paths = {path.as_posix() for path in Path("crates").glob("*/Cargo.toml")}
paths.update(f"{member}/Cargo.toml" for member in workspace["members"])
digest = hashlib.sha256()
for relative in ["Cargo.toml", *sorted(paths)]:
    digest.update(relative.encode("utf-8") + b"\0")
    digest.update(Path(relative).read_bytes().replace(b"\r", b""))
    digest.update(b"\0")
print(digest.hexdigest())
PY
)"
lock_digest="$(tr -d '\r' < Cargo.lock | sha256sum | cut -d' ' -f1)"

# 1. Derive the product's package closure and its external direct
#    dependency floors from the checked-in manifests: starting at the
#    product's package roots, follow workspace path dependencies to grow
#    the closure, and collect every non-path requirement (normal and
#    build dependencies; dev-dependencies are exercised by the test
#    class but are not certified floors) those members declare from the
#    root manifest's [workspace.dependencies] table or inline.
python3 - Cargo.toml "${ROOTS[@]}" > target/floor-proof/floors-selection.json <<'PY'
import json
import posixpath
import sys
import tomllib

def fail(message):
    print("proof-direct-floors: " + message, file=sys.stderr)
    raise SystemExit(3)

data = tomllib.load(open(sys.argv[1], "rb"))
workspace = data["workspace"]
ws_deps = workspace["dependencies"]
roots = list(sys.argv[2:])

# Package name <-> manifest directory, from the checked-in members.
name_dir = {}
dir_name = {}
for member in workspace["members"]:
    manifest = tomllib.load(open(f"{member}/Cargo.toml", "rb"))
    name = manifest["package"]["name"]
    name_dir[name] = member
    dir_name[member] = name

def declared_requirement(spec):
    if isinstance(spec, str):
        return spec
    return spec["version"]

def expanded_features(manifest, requested):
    features = manifest.get("features", {})
    seen = set()
    paths = {feature: [feature] for feature in requested}
    stack = sorted(requested, reverse=True)
    while stack:
        feature = stack.pop()
        if feature in seen:
            continue
        seen.add(feature)
        for child in reversed(features.get(feature, [])):
            paths.setdefault(child, paths[feature] + [child])
            stack.append(child)
    return seen, paths


def enabling_features(features, manifest, dep):
    namespaced = "dep:" + dep
    implicit = not any(
        namespaced in edges for edges in manifest.get("features", {}).values()
    )
    return sorted(feature for feature in features if
                  (implicit and feature == dep) or feature == namespaced
                  or feature.startswith(dep + "/"))

closure = []
processed = {}
requested = {name: {"default"} for name in roots}
stack = sorted(roots, reverse=True)
requirements = {}
floors = []
optional_decisions = {}
while stack:
    name = stack.pop()
    if name not in name_dir:
        fail(f"product root {name} is not a workspace member")
    if processed.get(name) == requested[name]:
        continue
    processed[name] = set(requested[name])
    if name not in closure:
        closure.append(name)
    manifest = tomllib.load(open(f"{name_dir[name]}/Cargo.toml", "rb"))
    features, feature_paths = expanded_features(manifest, requested[name])
    for table in ("dependencies", "build-dependencies"):
        for dep, spec in manifest.get(table, {}).items():
            member_spec = spec
            member_optional = isinstance(spec, dict) and spec.get("optional")
            inherited = isinstance(spec, dict) and spec.get("workspace")
            if inherited:
                if dep not in ws_deps:
                    fail(f"{name} inherits {dep} from the workspace, but the workspace does not declare it")
                spec = ws_deps[dep]
            if isinstance(spec, dict) and "git" in spec:
                fail(f"{name} declares a git dependency {dep}; out of proof scope")
            resolved_optional = isinstance(spec, dict) and spec.get("optional")
            if member_optional or resolved_optional:
                witnesses = enabling_features(features, manifest, dep)
                namespaced = any("dep:" + dep in edges
                                 for edges in manifest.get("features", {}).values())
                reason = "enabled by a selected feature path" if witnesses else (
                    "no selected feature enables this optional dependency"
                    + ("; dep: namespace suppresses implicit activation" if namespaced else "")
                )
                optional_decisions[(name, table, dep)] = {
                    "owner": name, "table": table, "dependency": dep,
                    "disposition": "included" if witnesses else "excluded",
                    "reason": reason, "requested_features": sorted(requested[name]),
                    "activation_paths": [
                        [name + "/" + step for step in feature_paths[feature]]
                        for feature in witnesses
                    ],
                }
                if not witnesses:
                    continue
            if isinstance(spec, dict) and "path" in spec:
                # An inherited spec's path is workspace-root relative; a
                # direct spec's path is relative to the member manifest.
                if inherited:
                    target = spec["path"]
                else:
                    target = posixpath.normpath(posixpath.join(name_dir[name], spec["path"]))
                if target not in dir_name:
                    fail(f"{name} path dependency {dep} leaves the workspace ({target})")
                target_name = dir_name[target]
                # Every closure member is selected with -p by the proof
                # classes, so it receives its own defaults as well.
                wanted = requested.setdefault(target_name, {"default"})
                for source in (member_spec, spec):
                    if isinstance(source, dict):
                        wanted.update(source.get("features", []))
                for feature in features:
                    for prefix in (dep + "/", dep + "?/"):
                        if feature.startswith(prefix):
                            wanted.add(feature[len(prefix):])
                stack.append(target_name)
                continue
            requirement = declared_requirement(spec)
            if dep in requirements:
                if requirements[dep] != requirement:
                    fail(f"{dep} is declared with conflicting requirements ({requirements[dep]} vs {requirement})")
                continue
            requirements[dep] = requirement
            parts = requirement.split(".")
            while len(parts) < 3:
                parts.append("0")
            floors.append({
                "package": dep,
                "requirement": requirement,
                "floor": ".".join(parts[:3]),
            })

floors.sort(key=lambda row: row["package"])
closure.sort()
print(json.dumps({
    "roots": roots, "closure": closure, "floors": floors,
    "optional_dependencies": [optional_decisions[key] for key in sorted(optional_decisions)],
}, indent=1))
PY
floor_count="$(jq '.floors | length' target/floor-proof/floors-selection.json)"
closure_count="$(jq '.closure | length' target/floor-proof/floors-selection.json)"
if [ "$floor_count" -eq 0 ]; then
  echo "proof-direct-floors: empty floor inventory for $PRODUCT; nothing to certify" >&2
  exit 1
fi
if [ "$closure_count" -eq 0 ]; then
  echo "proof-direct-floors: empty package closure for $PRODUCT; nothing to prove" >&2
  exit 1
fi
jq '.floors' target/floor-proof/floors-selection.json > target/floor-proof/floors.json
mapfile -t CLOSURE < <(jq -r '.closure[]' target/floor-proof/floors-selection.json)

# 2. Build the direct-floor candidate lock: regenerate from scratch, then
#    pin each external direct dependency to its declared minimum. Pin
#    failures (version does not exist or is yanked) are recorded as
#    resolver failures for those rows and fail the overall proof.
rm -f Cargo.lock
python3 - <<'PY'
import json
import subprocess

floors = json.load(open("target/floor-proof/floors.json", encoding="utf-8"))
failures = {}
for row in floors:
    proc = subprocess.run(
        ["cargo", "update", "-p", row["package"], "--precise", row["floor"]],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        failures[row["package"]] = proc.stderr.strip()[:300]
json.dump(failures, open("target/floor-proof/pin-failures.json", "w"), indent=1)
PY
pin_failures="$(cat target/floor-proof/pin-failures.json)"
# Verify every pin that succeeded actually landed at the floor. Packages
# whose pin failed are excluded here — they surface as resolver_failure
# rows in the receipt instead of aborting before any receipt exists.
floor_move_failures="$(python3 - <<'PY'
import json
import tomllib

lock = tomllib.load(open("Cargo.lock", "rb"))
floors = json.load(open("target/floor-proof/floors.json", encoding="utf-8"))
pin_failed = set(json.load(open("target/floor-proof/pin-failures.json", encoding="utf-8")).keys())
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

# Establish the actual committed floor subject before strict source admission.
python3 scripts/floor_source_identity.py "$SOURCE_COMMIT" > target/floor-proof/source-identity.json

# 3. Bounded proof classes, run against the product's own package
#    closure — never the whole workspace — so the receipt's commands
#    cover exactly the packages the product names.
check_args=()
for member in "${CLOSURE[@]}"; do
  check_args+=(-p "$member")
done
# The floored test class excludes only retained-output meta-tests:
# they grade the proof's own retained receipts against the tree, so
# they cannot pass inside the very run that refreshes those receipts.
# The skip is recorded verbatim in the receipt's command list, and CI
# still runs the meta-tests against every committed tree.
DRIFT_TEST_SKIPS=(
  --skip minimum_direct_version_drift_retained_receipts_are_current_with_the_live_tree
  --skip check_exits_zero_when_every_release_set_receipt_is_current
  --skip minimum_direct_version_fixtures_retained_proof_receipt_is_law_clean
  --skip minimum_direct_version_products_retained_advisory_receipts_are_clean
  --skip minimum_direct_version_products_cargo_allow_receipt_certifies_its_closure
)
check_cmd=""
test_cmd=""
package_cmd=""
if [[ " ${CLASS_LIST[*]} " == *" check "* ]]; then
  check_cmd="cargo check --locked --target $host_target ${check_args[*]}"
fi
if [[ " ${CLASS_LIST[*]} " == *" test "* ]]; then
  test_cmd="cargo test --locked --target $host_target ${check_args[*]} -- ${DRIFT_TEST_SKIPS[*]}"
fi
if [[ " ${CLASS_LIST[*]} " == *" package "* ]]; then
  package_cmd="cargo package -p ${CLOSURE[0]} --locked --target $host_target --no-verify --allow-dirty --target-dir target/package-proof"
fi

check_status=0
test_status=0
package_status=0
if [[ -n "$check_cmd" ]]; then
  cargo check --locked --target "$host_target" "${check_args[@]}" || check_status=$?
fi
if [[ -n "$test_cmd" ]]; then
  cargo test --locked --target "$host_target" "${check_args[@]}" \
    -- "${DRIFT_TEST_SKIPS[@]}" || test_status=$?
fi
if [[ -n "$package_cmd" ]]; then
  cargo package -p "${CLOSURE[0]}" --locked --target "$host_target" --no-verify --allow-dirty \
    --target-dir target/package-proof || package_status=$?
fi

# 4. Emit the typed receipt: resolved versions from the floor lock; pin
#    failures become resolver failures; a failed proof class becomes an
#    instrument failure for every row (the failing crate is in the
#    captured output). The command list records only the classes that
#    actually ran, and the claim boundary follows the selected product's
#    posture: non-cargo-allow products are report-only and advisory.
PRODUCT="$PRODUCT" \
CHECK_CMD="$check_cmd" TEST_CMD="$test_cmd" PACKAGE_CMD="$package_cmd" \
ROOTS_JSON="$(jq -c '.roots' target/floor-proof/floors-selection.json)" \
python3 - "$WORKTREE/Cargo.lock" "$WORKTREE/target/floor-proof/floors.json" "$WORKTREE/target/floor-proof/pin-failures.json" \
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
product = os.environ["PRODUCT"]
package_roots = json.loads(os.environ["ROOTS_JSON"])
execution = json.load(open("target/floor-proof/execution-identity.json", encoding="utf-8"))
subject = json.load(open("target/floor-proof/source-identity.json", encoding="utf-8"))

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
        "source_identity": "registry:crates.io",
        "result": result,
        "limitation": limitation,
    })

lock_bytes = open(lock_path, "rb").read().replace(b"\r", b"")
floor_lock_digest = "sha256:v1:" + hashlib.sha256(lock_bytes).hexdigest()

commands = ["cargo update -p <dep> --precise <floor> (per external direct dep of the product closure)"]
commands += [os.environ[name] for name in ("CHECK_CMD", "TEST_CMD", "PACKAGE_CMD") if os.environ[name]]
class_text = ", ".join(name for name in ("check", "test", "package") if os.environ.get(name.upper() + "_CMD"))

if product == "cargo-allow":
    claim_boundary = (
        "Exact selected proof of the cargo-allow release set's declared direct "
        "dependency floors at the claimed MSRV, over the release set's package "
        f"closure under the bounded classes: {class_text} (package is a "
        "single-package archive sample, not closure packaging). Transitive minimal "
        "combinations are not certified."
    )
    limitations = [
        f"bounded proof: selected check/test classes cover the release-set closure; "
        "package covers only the package named by its command, "
        "not every target or feature combination",
        "internal =0.2.0 workspace pins are proven by the same closure build",
        "dev-dependencies are exercised by the test class but are not certified floors",
        "the certified set is the closure's default-feature compile set: optional "
        "dependencies enabled by a default feature are certified; optional "
        "dependencies no default feature enables stay outside it",
        "the drift meta-tests are excluded from the floored test class by name "
        "(they grade this proof's own retained receipts); CI runs them against "
        "every committed tree",
    ]
else:
    claim_boundary = (
        f"Advisory, report-only proof of the {product} package closure's declared "
        f"direct dependency floors at the claimed MSRV under the bounded classes: "
        f"{class_text}. Test and package enforcement requires the owning "
        "package-family authority; these rows never gate the cargo-allow release "
        "set. Transitive minimal combinations are not certified."
    )
    limitations = [
        f"bounded proof: the {class_text} classes over the {product} closure, "
        "not every target or feature combination",
        "advisory: report-only product rows never gate the cargo-allow release set",
        "dev-dependencies are exercised by the test class but are not certified floors",
        "the certified set is the closure's default-feature compile set: optional "
        "dependencies enabled by a default feature are certified; optional "
        "dependencies no default feature enables stay outside it",
    ]

limitations.append(
    f"observed rustc {execution['toolchain']}; cargo {execution['cargo']}; "
    f"rustdoc {execution['rustdoc']}; host {execution['host']}; "
    "every selected class explicitly targets that host using the observed tool paths; "
    "collector-owned RUSTC/RUSTDOC override inherited and configured tool selection, "
    "and compiler wrappers are disabled"
)
if os.environ["PACKAGE_CMD"]:
    limitations.append("package is a single-package --no-verify archive sample; see its exact command")
limitations.append(
    f"local floor subject: source {subject['source_commit']}; "
    f"derived commit {subject['derived_commit']}; tree {subject['derived_tree']}; "
    "only Cargo.lock may differ from source; this local candidate is not an upstream commit"
)

receipt = {
    "schema_id": "cargo-allow.minimum-direct-version.v1",
    "schema_version": 1,
    "product": product,
    "package_roots": package_roots,
    "msrv": msrv,
    "toolchain": execution["toolchain"],
    "target": execution["target"],
    "manifest_set_digest": manifest_set_digest,
    "lock_digest": lock_digest,
    "rows": rows,
    "commands": commands,
    "floor_lock_digest": floor_lock_digest,
    "limitations": limitations,
    "claim_boundary": claim_boundary,
}
sys.stdout.reconfigure(newline="\n")
print(json.dumps(receipt, indent=1))
PY

# Keep selection explanations separate from the public v1 proof vocabulary.
# The companion binds the exact emitted JSON bytes; it adds no proof class.
python3 - "$OUT" target/floor-proof/floors-selection.json "$SOURCE_COMMIT" <<'PY'
import hashlib
import json
from pathlib import Path
import sys

receipt_path = Path(sys.argv[1])
receipt_bytes = receipt_path.read_bytes()
receipt = json.loads(receipt_bytes)
selection = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
subject = json.loads(Path("target/floor-proof/source-identity.json").read_text(encoding="utf-8"))

def cell(value):
    return (str(value).replace("\\", "\\\\").replace("|", "\\|")
            .replace("`", "\\`").replace("\r", "\\r").replace("\n", "\\n"))

lines = [
    "# Direct-floor selection evidence", "",
    "Selection explanations only; the companion JSON owns proof dispositions.",
    "These paths start at each member's selected default/requested features;",
    "they are local activation witnesses, not a complete cross-crate feature graph.", "",
    f"- Product: {receipt['product']}",
    f"- Package roots: {cell(', '.join(selection['roots']))}",
    f"- Selected closure: {cell(', '.join(selection['closure']))}",
    f"- Starting source commit: {cell(sys.argv[3])}",
    f"- Executed derived commit: {cell(subject['derived_commit'])}",
    f"- Executed derived tree: {cell(subject['derived_tree'])}",
    f"- Receipt: {cell(receipt_path.name)}",
    f"- Receipt SHA-256: sha256:v1:{hashlib.sha256(receipt_bytes).hexdigest()}",
    f"- Manifest-set digest: {receipt['manifest_set_digest']}",
    f"- Starting lock digest: {receipt['lock_digest']}",
    f"- Executed floor-lock digest: {receipt['floor_lock_digest']}", "",
    "| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |",
    "| --- | --- | --- | --- | --- | --- |",
]
for decision in selection["optional_dependencies"]:
    witnesses = "; ".join(" -> ".join(path) for path in decision["activation_paths"])
    lines.append("| " + " | ".join(cell(value) for value in (
        decision["owner"], decision["table"], decision["dependency"], decision["disposition"],
        decision["reason"], witnesses or "none",
    )) + " |")
if not selection["optional_dependencies"]:
    lines += ["", "No optional dependency declarations were encountered in this selected closure."]
receipt_path.with_suffix(".selection.md").write_text(
    "\n".join(lines) + "\n", encoding="utf-8", newline="\n",
)
PY

overall=0
if [[ "$check_status" -ne 0 || "$test_status" -ne 0 || "$package_status" -ne 0 ]]; then
  overall=1
fi
if [[ -s "$WORKTREE/target/floor-proof/pin-failures.json" ]] &&
  [[ "$(cat "$WORKTREE/target/floor-proof/pin-failures.json")" != "{}" ]]; then
  overall=1
fi
echo "proof-direct-floors: proof classes completed for $PRODUCT (overall=$overall)"
exit "$overall"
