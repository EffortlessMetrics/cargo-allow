#!/usr/bin/env bash
# Exact installed 0.1.11 -> candidate 0.2.0 -> 0.1.11 stateful journey (#2485/#3853).
set -euo pipefail

fail() { printf 'exact-upgrade-rollback: error: %s\n' "$*" >&2; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_SET_DIR="${PACKAGE_SET_DIR:-${ROOT}/target/exact-candidate-package-set}"
WORK_DIR="${WORK_DIR:-${ROOT}/target/exact-upgrade-rollback-journey}"
PACKAGE_RECEIPT="${PACKAGE_SET_DIR}/exact-candidate-package-set.receipt.json"
CANDIDATE_BIN="${CANDIDATE_BIN:-}"
if [[ -z "${CANDIDATE_BIN}" ]]; then
  candidate_default="${PACKAGE_SET_DIR}/install/bin/cargo-allow"
  # MSYS stat aliases cargo-allow to cargo-allow.exe, so probe the .exe
  # first and let Windows Python see the real file name.
  if [[ -f "${candidate_default}.exe" ]]; then
    CANDIDATE_BIN="${candidate_default}.exe"
  elif [[ -f "${candidate_default}" ]]; then
    CANDIDATE_BIN="${candidate_default}"
  else
    fail "missing exact candidate package install binary under ${PACKAGE_SET_DIR}/install/bin"
  fi
fi
RECEIPT="${WORK_DIR}/exact-upgrade-rollback-journey.receipt.json"
SCHEMA="${ROOT}/docs/dogfood/fixtures/release/exact-upgrade-rollback-journey.v1.schema.json"
FIXTURE="${ROOT}/docs/dogfood/fixtures/release/upgrade-rollback-repository.toml"

[[ -f "${PACKAGE_RECEIPT}" ]] || fail "missing exact candidate package receipt"
[[ -f "${SCHEMA}" && -f "${FIXTURE}" ]] || fail "missing journey contract"
[[ -x "${CANDIDATE_BIN}" || -f "${CANDIDATE_BIN}" ]] || fail "missing candidate binary"
command -v cargo >/dev/null || fail "cargo is required"
command -v python3 >/dev/null || fail "python3 is required"
command -v git >/dev/null || fail "git is required"

python3 - "${CANDIDATE_BIN}" "${PACKAGE_SET_DIR}/install" "${PACKAGE_RECEIPT}" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

binary = Path(sys.argv[1]).resolve(strict=True)
install = Path(sys.argv[2]).resolve(strict=True)
package_receipt = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
try:
    binary.relative_to(install)
except ValueError as error:
    raise SystemExit(f"candidate binary must be below exact install root {install}") from error
if binary.is_symlink() or not binary.is_file():
    raise SystemExit("candidate binary must be a regular non-symlink file")
expected_sha = (package_receipt.get("install") or {}).get("binary_sha256")
if expected_sha:
    actual_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    if actual_sha != expected_sha:
        raise SystemExit(
            f"candidate binary digest {actual_sha} does not match package receipt {expected_sha}"
        )
PY

if [[ -z "${OLD_BIN:-}" ]]; then
  rm -rf "${WORK_DIR}"
fi
mkdir -p "${WORK_DIR}/old-install" "${WORK_DIR}/old-cargo-home" "${WORK_DIR}/clean-repository" "${WORK_DIR}/repository" "${WORK_DIR}/artifacts"
old_root="${WORK_DIR}/old-install"
old_bin="${old_root}/bin/cargo-allow"
if [[ ! -f "${old_bin}" && -f "${old_bin}.exe" ]]; then
  old_bin="${old_bin}.exe"
fi
if [[ -n "${OLD_BIN:-}" ]]; then
  old_bin="$(python3 - "${OLD_BIN}" "${WORK_DIR}" <<'PY'
import sys
from pathlib import Path

p = Path(sys.argv[1]).resolve(strict=True)
root = Path(sys.argv[2]).resolve()
try:
    p.relative_to(root)
except ValueError as error:
    raise SystemExit(f"OLD_BIN must be under {root}") from error
if p.is_symlink() or not p.is_file():
    raise SystemExit("OLD_BIN must be a regular file")
print(p)
PY
)"
else
  CARGO_HOME="${WORK_DIR}/old-cargo-home" cargo install cargo-allow --version 0.1.11 --locked --root "${old_root}" --quiet
fi
# MSYS stat aliases the .exe name, so probe the .exe first after install.
if [[ -f "${old_bin}.exe" ]]; then
  old_bin="${old_bin}.exe"
fi
[[ -f "${old_bin}" ]] || fail "exact 0.1.11 binary was not installed"

version() { "$1" --version | tr -d '\r'; }
old_version="$(version "${old_bin}")"
candidate_version="$(version "${CANDIDATE_BIN}")"
[[ "${old_version}" == "cargo-allow 0.1.11" ]] || fail "old binary is not exact 0.1.11: ${old_version}"
[[ "${candidate_version}" == "cargo-allow 0.2.0" ]] || fail "candidate binary is not exact 0.2.0: ${candidate_version}"

PACKAGE_RECEIPT="${PACKAGE_RECEIPT}" \
OLD_BIN="${old_bin}" \
CANDIDATE_BIN="${CANDIDATE_BIN}" \
WORK_DIR="${WORK_DIR}" \
RECEIPT="${RECEIPT}" \
FIXTURE="${FIXTURE}" \
OLD_VERSION="${old_version}" \
CANDIDATE_VERSION="${candidate_version}" \
CHECKOUT_ROOT="${ROOT}" \
python3 - <<'PY'
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"exact-upgrade-rollback: error: {message}")


def rmtree_force(path: Path) -> None:
    if not path.exists():
        return

    def handle_ro(func, target, _exc):
        os.chmod(target, stat.S_IWRITE)
        func(target)

    shutil.rmtree(path, onexc=handle_ro) if sys.version_info >= (3, 12) else shutil.rmtree(path, onerror=handle_ro)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_lf(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.replace("\r\n", "\n").encode("utf-8"))


def isolated_env(checkout_root: Path) -> dict[str, str]:
    env = {}
    for key, value in os.environ.items():
        if key in {
            "CARGO_ALLOW_ROOT",
            "CARGO_ALLOW_CONFIG",
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        }:
            continue
        env[key] = value
    # Strip any ambient PATH entry that contains a cargo-allow executable or
    # points inside the source checkout so decisive commands rely only on the
    # explicit absolute binary path.
    checkout_resolved = checkout_root.resolve()
    filtered_path = []
    for entry in env.get("PATH", "").split(os.pathsep):
        if not entry:
            continue
        entry_path = Path(entry)
        try:
            resolved = entry_path.resolve()
            if checkout_resolved == resolved or checkout_resolved in resolved.parents:
                continue
        except OSError:
            pass
        if (entry_path / "cargo-allow").exists() or (entry_path / "cargo-allow.exe").exists():
            continue
        filtered_path.append(entry)
    env["PATH"] = os.pathsep.join(filtered_path)
    return env


def run_cmd(argv: list[str], cwd: Path, env: dict[str, str], expect_ok: bool = True) -> subprocess.CompletedProcess:
    proc = subprocess.run(
        argv,
        cwd=str(cwd),
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if expect_ok and proc.returncode != 0:
        fail(
            f"command failed ({proc.returncode}): {' '.join(argv)}\n"
            f"stdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    if not expect_ok and proc.returncode == 0:
        fail(f"expected non-zero exit but command succeeded: {' '.join(argv)}")
    return proc


def parse_json_output(proc: subprocess.CompletedProcess, label: str) -> dict:
    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError as error:
        fail(f"{label} did not emit valid JSON: {error}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}")
    if not isinstance(data, dict):
        fail(f"{label} JSON output must be an object")
    return data


def init_git_repo(repo: Path, env: dict[str, str]) -> str:
    run_cmd(["git", "init", "-q"], repo, env)
    run_cmd(["git", "config", "core.autocrlf", "false"], repo, env)
    run_cmd(["git", "config", "user.name", "cargo-allow-proof"], repo, env)
    run_cmd(["git", "config", "user.email", "cargo-allow-proof@localhost"], repo, env)
    run_cmd(["git", "add", "src", "policy", "unrelated.txt"], repo, env)
    run_cmd(["git", "commit", "-qm", "initial"], repo, env)
    return run_cmd(["git", "rev-parse", "HEAD"], repo, env).stdout.strip()


def extract_list_snapshot(list_json: dict) -> dict:
    entries = list_json.get("allow_entries") or list_json.get("entries") or []
    by_id = {}
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("id"), str):
            continue
        by_id[entry["id"]] = {
            "status": entry.get("status"),
            "classification": entry.get("classification"),
        }
    return {
        "allow_ids": sorted(by_id.keys()),
        "baseline_debt_ids": sorted(
            aid for aid, info in by_id.items() if info.get("classification") == "baseline_debt"
        ),
        "location_drift_ids": sorted(
            aid for aid, info in by_id.items() if info.get("status") == "location_drift"
        ),
        "stale_ids": sorted(
            aid for aid, info in by_id.items() if info.get("status") == "stale"
        ),
        "by_id": by_id,
    }


def extract_changed_ids(report: dict) -> set[str]:
    ids: set[str] = set()
    for value in (report.get("mutation_receipt") or {}).get("changed_allow_ids") or []:
        if isinstance(value, str):
            ids.add(value)
    for entry in report.get("stale_entries") or []:
        if isinstance(entry, dict) and isinstance(entry.get("id"), str):
            ids.add(entry["id"])
    return ids


work_dir = Path(os.environ["WORK_DIR"]).resolve()
package_receipt_path = Path(os.environ["PACKAGE_RECEIPT"]).resolve()
fixture_path = Path(os.environ["FIXTURE"]).resolve()
receipt_path = Path(os.environ["RECEIPT"]).resolve()
old_bin = Path(os.environ["OLD_BIN"]).resolve()
candidate_bin = Path(os.environ["CANDIDATE_BIN"]).resolve()
checkout_root = Path(os.environ["CHECKOUT_ROOT"]).resolve()
old_version = os.environ["OLD_VERSION"]
candidate_version = os.environ["CANDIDATE_VERSION"]

old_sha = sha256_file(old_bin)
candidate_sha = sha256_file(candidate_bin)
if old_sha == candidate_sha:
    fail("old 0.1.11 binary and candidate 0.2.0 binary must not share the same SHA-256 digest")

package_receipt = json.loads(package_receipt_path.read_text(encoding="utf-8"))
env = isolated_env(checkout_root)

fixture_bytes = fixture_path.read_bytes()
fixture_text = fixture_bytes.decode("utf-8")
header_cutoff = fixture_text.find("\n[[allow]]")
if header_cutoff < 0:
    fail("fixture missing [[allow]] section")
clean_policy_text = fixture_text[:header_cutoff].rstrip() + "\n"

# 1. Materialize clean_strict repository fixture (optional profiles absent).
clean_repo = work_dir / "clean-repository"
rmtree_force(clean_repo)
clean_repo.mkdir(parents=True)
write_lf(clean_repo / "policy/allow.toml", clean_policy_text)
write_lf(clean_repo / "src/main.rs", "fn main() {}\n")
write_lf(clean_repo / "unrelated.txt", "clean-unrelated-preserved\n")
init_git_repo(clean_repo, env)
clean_policy_sha = sha256_file(clean_repo / "policy/allow.toml")
clean_unrelated_pre = sha256_file(clean_repo / "unrelated.txt")

# 2. Materialize stateful_brownfield_lifecycle repository fixture.
repo = work_dir / "repository"
rmtree_force(repo)
repo.mkdir(parents=True)
(repo / "policy").mkdir(parents=True, exist_ok=True)
(repo / "policy/allow.toml").write_bytes(fixture_bytes)
write_lf(
    repo / "src/main.rs",
    'fn reviewed_boot(v: Option<u8>) -> u8 {\n    v.expect("reviewed boot invariant")\n}\n\nfn main() {\n    let _ = reviewed_boot(Some(1));\n}\n',
)
write_lf(
    repo / "src/legacy.rs",
    "pub fn legacy_load(v: Option<u8>) -> u8 {\n    v.unwrap()\n}\n",
)
write_lf(
    repo / "src/drifted.rs",
    "pub fn drifted_helper(v: Option<u8>) -> u8 {\n    v.unwrap()\n}\n",
)
write_lf(
    repo / "src/removed_finding.rs",
    "pub fn cleaned_helper(v: Option<u8>) -> u8 {\n    v.unwrap_or(0)\n}\n",
)
write_lf(repo / "unrelated.txt", "unrelated-preserved\n")
write_lf(repo / "target/cargo-allow/unrelated-user.txt", "unrelated-output-root-preserved\n")
base_commit = init_git_repo(repo, env)

preimage_dir = work_dir / "preimages"
preimage_dir.mkdir(parents=True, exist_ok=True)
preimage_policy = preimage_dir / "policy.allow.toml"
preimage_policy.write_bytes((repo / "policy/allow.toml").read_bytes())

pre_policy_sha = sha256_file(repo / "policy/allow.toml")
unrelated_root_pre = sha256_file(repo / "unrelated.txt")
unrelated_output_pre = sha256_file(repo / "target/cargo-allow/unrelated-user.txt")

steps: list[dict] = []


def run_readonly_suite(binary: Path, leg: str, label: str) -> dict:
    clean_policy = clean_repo / "policy/allow.toml"
    stateful_policy = repo / "policy/allow.toml"
    before_clean_sha = sha256_file(clean_policy)
    before_stateful_sha = sha256_file(stateful_policy)

    # Clean repository checks (optional profiles absent).
    p_c_doc = run_cmd([str(binary), "doctor", "--root", str(clean_repo), "--config", str(clean_policy), "--format", "json"], clean_repo, env)
    parse_json_output(p_c_doc, f"{label}:clean:doctor")
    steps.append({"leg": leg, "id": f"{label}:clean:doctor", "exit_code": p_c_doc.returncode})

    p_c_chk = run_cmd([str(binary), "check", "--root", str(clean_repo), "--config", str(clean_policy), "--mode", "no-new", "--format", "json"], clean_repo, env)
    c_chk = parse_json_output(p_c_chk, f"{label}:clean:check")
    if c_chk.get("status") != "passed":
        fail(f"{label}:clean:check expected passed, got {c_chk.get('status')!r}")
    steps.append({"leg": leg, "id": f"{label}:clean:check", "exit_code": p_c_chk.returncode})

    # Stateful brownfield/lifecycle checks.
    p_doc = run_cmd([str(binary), "doctor", "--root", str(repo), "--config", str(stateful_policy), "--format", "json"], repo, env)
    parse_json_output(p_doc, f"{label}:doctor")
    steps.append({"leg": leg, "id": f"{label}:doctor", "exit_code": p_doc.returncode})

    p_aud = run_cmd([str(binary), "audit", "--root", str(repo), "--config", str(stateful_policy), "--format", "json"], repo, env)
    parse_json_output(p_aud, f"{label}:audit")
    steps.append({"leg": leg, "id": f"{label}:audit", "exit_code": p_aud.returncode})

    p_chk = run_cmd([str(binary), "check", "--root", str(repo), "--config", str(stateful_policy), "--mode", "no-new", "--format", "json"], repo, env)
    chk = parse_json_output(p_chk, f"{label}:check")
    if chk.get("status") != "passed":
        fail(f"{label}:check expected passed, got {chk.get('status')!r}")
    steps.append({"leg": leg, "id": f"{label}:check", "exit_code": p_chk.returncode})

    p_lst = run_cmd([str(binary), "list", "--root", str(repo), "--config", str(stateful_policy), "--format", "json"], repo, env)
    lst = parse_json_output(p_lst, f"{label}:list")
    snap = extract_list_snapshot(lst)
    steps.append({"leg": leg, "id": f"{label}:list", "exit_code": p_lst.returncode})

    p_exp = run_cmd([str(binary), "explain", "allow-0001", "--root", str(repo), "--config", str(stateful_policy), "--format", "json"], repo, env)
    parse_json_output(p_exp, f"{label}:explain")
    steps.append({"leg": leg, "id": f"{label}:explain", "exit_code": p_exp.returncode})

    p_wrk = run_cmd([str(binary), "worklist", "--root", str(repo), "--config", str(stateful_policy), "--format", "json"], repo, env)
    parse_json_output(p_wrk, f"{label}:worklist")
    steps.append({"leg": leg, "id": f"{label}:worklist", "exit_code": p_wrk.returncode})

    p_dif = run_cmd([str(binary), "diff", "--root", str(repo), "--config", str(stateful_policy), "--base", base_commit, "--format", "json"], repo, env)
    dif = parse_json_output(p_dif, f"{label}:diff")
    if dif.get("status") != "passed":
        fail(f"{label}:diff expected passed, got {dif.get('status')!r}")
    steps.append({"leg": leg, "id": f"{label}:diff", "exit_code": p_dif.returncode})

    if sha256_file(clean_policy) != before_clean_sha or sha256_file(stateful_policy) != before_stateful_sha:
        fail(f"{label}: read-only command unexpectedly mutated policy/allow.toml")

    return snap


# Leg 1: Exact 0.1.11 pre-upgrade baseline capture.
from_snap = run_readonly_suite(old_bin, "from", "old-0.1.11")
if from_snap["allow_ids"] != ["allow-0001", "allow-0002", "allow-0003", "allow-0004"]:
    fail(f"unexpected 0.1.11 allow_ids: {from_snap['allow_ids']}")
if from_snap["baseline_debt_ids"] != ["allow-0002"]:
    fail(f"expected allow-0002 as baseline_debt in 0.1.11, got {from_snap['baseline_debt_ids']}")
if from_snap["location_drift_ids"] != ["allow-0003"]:
    fail(f"expected allow-0003 as location_drift in 0.1.11, got {from_snap['location_drift_ids']}")
if from_snap["stale_ids"] != ["allow-0004"]:
    fail(f"expected allow-0004 as stale in 0.1.11, got {from_snap['stale_ids']}")

# Leg 2: Exact 0.2.0 candidate read-only compatibility + previewed write/migrate journey.
candidate_snap = run_readonly_suite(candidate_bin, "candidate", "candidate-0.2.0")
if candidate_snap["allow_ids"] != from_snap["allow_ids"]:
    fail(f"0.2.0 read-only allow_ids drifted: {candidate_snap['allow_ids']} vs {from_snap['allow_ids']}")
if candidate_snap["baseline_debt_ids"] != from_snap["baseline_debt_ids"]:
    fail("0.2.0 silently altered baseline_debt classification")
if candidate_snap["location_drift_ids"] != from_snap["location_drift_ids"]:
    fail(f"0.2.0 location_drift drifted: {candidate_snap['location_drift_ids']}")
if candidate_snap["stale_ids"] != from_snap["stale_ids"]:
    fail(f"0.2.0 stale detection drifted: {candidate_snap['stale_ids']}")
post_readonly_policy_sha = sha256_file(repo / "policy/allow.toml")

# Negative control: shallow / missing-base diff must fail closed without mutating policy.
p_missing_base = run_cmd(
    [
        str(candidate_bin),
        "diff",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--base",
        "0" * 40,
        "--format",
        "json",
    ],
    repo,
    env,
    expect_ok=False,
)
steps.append({"leg": "candidate", "id": "candidate-0.2.0:diff-missing-base", "exit_code": p_missing_base.returncode})
if sha256_file(repo / "policy/allow.toml") != post_readonly_policy_sha:
    fail("missing-base diff mutated policy/allow.toml")

# Track exact owned outputs written inside the repository under candidate leg.
created_owned_files: list[str] = []
modified_files = ["policy/allow.toml"]

# Previewed mutation 1: refresh --allow-id allow-0003 (--dry-run -> --write).
refresh_preview_rel = "target/cargo-allow/refresh-preview.json"
refresh_write_rel = "target/cargo-allow/refresh-write.json"
refresh_preview_path = repo / refresh_preview_rel
refresh_write_path = repo / refresh_write_rel
created_owned_files.extend([refresh_preview_rel, refresh_write_rel])

p_ref_prev = run_cmd(
    [
        str(candidate_bin),
        "refresh",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--allow-id",
        "allow-0003",
        "--dry-run",
        "--format",
        "json",
        "--output",
        str(refresh_preview_path),
    ],
    repo,
    env,
)
steps.append({"leg": "candidate", "id": "candidate-0.2.0:refresh-preview", "exit_code": p_ref_prev.returncode})
refresh_preview_unchanged = sha256_file(repo / "policy/allow.toml") == post_readonly_policy_sha
if not refresh_preview_unchanged:
    fail("refresh --dry-run mutated policy/allow.toml")
ref_prev_json = json.loads(refresh_preview_path.read_text(encoding="utf-8"))
if ref_prev_json.get("schema_id") != "cargo-allow.refresh.v1":
    fail(f"unexpected refresh preview schema_id: {ref_prev_json.get('schema_id')!r}")
if (ref_prev_json.get("mode") or {}).get("write_requested") is True:
    fail("refresh --dry-run reported write_requested=true")
ref_prev_ids = extract_changed_ids(ref_prev_json)
if "allow-0003" not in ref_prev_ids:
    fail(f"refresh preview missing allow-0003: {sorted(ref_prev_ids)}")

p_ref_write = run_cmd(
    [
        str(candidate_bin),
        "refresh",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--allow-id",
        "allow-0003",
        "--write",
        "--format",
        "json",
        "--output",
        str(refresh_write_path),
    ],
    repo,
    env,
)
steps.append({"leg": "candidate", "id": "candidate-0.2.0:refresh-write", "exit_code": p_ref_write.returncode})
ref_write_json = json.loads(refresh_write_path.read_text(encoding="utf-8"))
ref_write_ids = extract_changed_ids(ref_write_json)
if (ref_write_json.get("mutation_receipt") or {}).get("result") != "written":
    fail(f"refresh --write did not report written: {ref_write_json.get('mutation_receipt')!r}")
refresh_agree = ref_prev_ids == ref_write_ids
if not refresh_agree:
    fail(f"refresh preview/apply disagree: {sorted(ref_prev_ids)} != {sorted(ref_write_ids)}")

# Previewed mutation 2: prune --stale (--dry-run -> --write).
post_refresh_policy_sha = sha256_file(repo / "policy/allow.toml")
prune_preview_rel = "target/cargo-allow/prune-preview.json"
prune_write_rel = "target/cargo-allow/prune-write.json"
prune_preview_path = repo / prune_preview_rel
prune_write_path = repo / prune_write_rel
created_owned_files.extend([prune_preview_rel, prune_write_rel])

p_prn_prev = run_cmd(
    [
        str(candidate_bin),
        "prune",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--stale",
        "--dry-run",
        "--format",
        "json",
        "--output",
        str(prune_preview_path),
    ],
    repo,
    env,
)
steps.append({"leg": "candidate", "id": "candidate-0.2.0:prune-preview", "exit_code": p_prn_prev.returncode})
prune_preview_unchanged = sha256_file(repo / "policy/allow.toml") == post_refresh_policy_sha
if not prune_preview_unchanged:
    fail("prune --dry-run mutated policy/allow.toml")
prn_prev_json = json.loads(prune_preview_path.read_text(encoding="utf-8"))
if prn_prev_json.get("schema_id") != "cargo-allow.prune.v1":
    fail(f"unexpected prune preview schema_id: {prn_prev_json.get('schema_id')!r}")
prn_prev_ids = extract_changed_ids(prn_prev_json)
if "allow-0004" not in prn_prev_ids:
    fail(f"prune preview missing allow-0004: {sorted(prn_prev_ids)}")

p_prn_write = run_cmd(
    [
        str(candidate_bin),
        "prune",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--stale",
        "--write",
        "--format",
        "json",
        "--output",
        str(prune_write_path),
    ],
    repo,
    env,
)
steps.append({"leg": "candidate", "id": "candidate-0.2.0:prune-write", "exit_code": p_prn_write.returncode})
prn_write_json = json.loads(prune_write_path.read_text(encoding="utf-8"))
prn_write_ids = extract_changed_ids(prn_write_json)
if (prn_write_json.get("mutation_receipt") or {}).get("result") != "written":
    fail(f"prune --write did not report written: {prn_write_json.get('mutation_receipt')!r}")
prune_agree = prn_prev_ids == prn_write_ids
if not prune_agree:
    fail(f"prune preview/apply disagree: {sorted(prn_prev_ids)} != {sorted(prn_write_ids)}")

# Explicit receipted migrate path to an owned output file.
migrated_policy_rel = "target/cargo-allow/migrated-allow.toml"
migrate_summary_rel = "target/cargo-allow/migrate-summary.json"
migrated_policy_path = repo / migrated_policy_rel
migrate_summary_path = repo / migrate_summary_rel
created_owned_files.extend([migrated_policy_rel, migrate_summary_rel])

p_mig = run_cmd(
    [
        str(candidate_bin),
        "migrate",
        "--root",
        str(repo),
        "--from",
        str(repo / "policy/allow.toml"),
        "--out",
        str(migrated_policy_path),
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        str(migrate_summary_path),
    ],
    repo,
    env,
)
steps.append({"leg": "candidate", "id": "candidate-0.2.0:migrate", "exit_code": p_mig.returncode})
migrated_policy_sha = sha256_file(migrated_policy_path)
mig_summary = json.loads(migrate_summary_path.read_text(encoding="utf-8"))
mig_schema_id = mig_summary.get("schema_id") or "cargo-allow.migrate.v1"
p_mig_check = run_cmd(
    [
        str(candidate_bin),
        "check",
        "--root",
        str(repo),
        "--config",
        str(migrated_policy_path),
        "--mode",
        "no-new",
        "--format",
        "json",
    ],
    repo,
    env,
)
mig_check_json = parse_json_output(p_mig_check, "candidate-0.2.0:migrate-check")
if mig_check_json.get("status") != "passed":
    fail(f"migrated policy failed check: {mig_check_json.get('status')!r}")
p_mig_list = run_cmd(
    [
        str(candidate_bin),
        "list",
        "--root",
        str(repo),
        "--config",
        str(migrated_policy_path),
        "--format",
        "json",
    ],
    repo,
    env,
)
mig_list_snap = extract_list_snapshot(parse_json_output(p_mig_list, "candidate-0.2.0:migrate-list"))
if mig_list_snap["allow_ids"] != ["allow-0001", "allow-0002", "allow-0003"]:
    fail(f"migrate altered allow_ids: {mig_list_snap['allow_ids']}")
if mig_list_snap["baseline_debt_ids"] != ["allow-0002"]:
    fail(f"migrate altered baseline_debt classification: {mig_list_snap['baseline_debt_ids']}")

# Negative control: stale why --plan artifact rejected as write authority by add --from-plan.
plan_rel = "target/cargo-allow/stale-probe-plan.json"
plan_path = repo / plan_rel
probe_src_rel = "src/unreceipted_probe.rs"
probe_src_path = repo / probe_src_rel
created_owned_files.append(plan_rel)

policy_before_stale_plan = sha256_file(repo / "policy/allow.toml")
write_lf(probe_src_path, "pub fn probe(v: Option<u8>) -> u8 {\n    v.unwrap()\n}\n")
run_cmd(
    [
        str(candidate_bin),
        "why",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--kind",
        "panic",
        "--path",
        probe_src_rel,
        "--line",
        "2",
        "--include-untracked",
        "--plan",
        str(plan_path),
    ],
    repo,
    env,
)
# Mutate source so the plan becomes stale.
write_lf(probe_src_path, "// drifted line\npub fn probe(v: Option<u8>) -> u8 {\n    v.unwrap_or(0)\n}\n")
p_stale_add = run_cmd(
    [
        str(candidate_bin),
        "add",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--from-plan",
        str(plan_path),
        "--update",
        "--include-untracked",
        "--owner",
        "cargo-allow-release-proof",
        "--reason",
        "stale plan negative control must fail closed",
    ],
    repo,
    env,
    expect_ok=False,
)
steps.append({"leg": "candidate", "id": "candidate-0.2.0:why-plan-stale-reject", "exit_code": p_stale_add.returncode})
stale_plan_policy_unchanged = sha256_file(repo / "policy/allow.toml") == policy_before_stale_plan
if not stale_plan_policy_unchanged:
    fail("stale add --from-plan mutated policy/allow.toml")
probe_src_path.unlink()

# Post-mutation confirmation check.
p_post_chk = run_cmd(
    [
        str(candidate_bin),
        "check",
        "--root",
        str(repo),
        "--config",
        str(repo / "policy/allow.toml"),
        "--mode",
        "no-new",
        "--format",
        "json",
    ],
    repo,
    env,
)
post_chk_json = parse_json_output(p_post_chk, "candidate-0.2.0:post-mutation-check")
if post_chk_json.get("status") != "passed":
    fail(f"post-mutation check failed: {post_chk_json.get('status')!r}")
steps.append({"leg": "candidate", "id": "candidate-0.2.0:post-mutation-check", "exit_code": p_post_chk.returncode})

post_mutation_policy_sha = sha256_file(repo / "policy/allow.toml")
if post_mutation_policy_sha == pre_policy_sha:
    fail("stateful mutation journey did not modify policy/allow.toml")

clean_unrelated_post_cand = sha256_file(clean_repo / "unrelated.txt")
unrelated_root_post_cand = sha256_file(repo / "unrelated.txt")
unrelated_output_post_cand = sha256_file(repo / "target/cargo-allow/unrelated-user.txt")
if unrelated_root_post_cand != unrelated_root_pre or unrelated_output_post_cand != unrelated_output_pre:
    fail("candidate leg modified an unrelated control file")

# Leg 3: Owned-state rollback (restore ONLY owned modified files and delete ONLY
# declared owned created files — never rm -rf the repository or target/cargo-allow).
restored_modified_files: list[str] = []
for rel in modified_files:
    target = repo / rel
    if rel == "policy/allow.toml":
        target.write_bytes(preimage_policy.read_bytes())
    else:
        fail(f"no preimage recorded for modified file {rel}")
    restored_modified_files.append(rel)

removed_owned_files: list[str] = []
for rel in created_owned_files:
    target = repo / rel
    if target.exists():
        target.unlink()
    if target.exists():
        fail(f"failed to remove owned file {rel}")
    removed_owned_files.append(rel)

restored_policy_sha = sha256_file(repo / "policy/allow.toml")
if restored_policy_sha != pre_policy_sha:
    fail("restored policy/allow.toml digest does not match preimage")

clean_unrelated_post_rb = sha256_file(clean_repo / "unrelated.txt")
unrelated_root_post_rb = sha256_file(repo / "unrelated.txt")
unrelated_output_post_rb = sha256_file(repo / "target/cargo-allow/unrelated-user.txt")
if (
    clean_unrelated_post_rb != clean_unrelated_pre
    or unrelated_root_post_rb != unrelated_root_pre
    or unrelated_output_post_rb != unrelated_output_pre
):
    fail("owned-state rollback modified or removed an unrelated control file")

# Re-run exact 0.1.11 across both fixtures and verify semantic parity.
rollback_snap = run_readonly_suite(old_bin, "rollback", "rollback-0.1.11")
semantic_parity_restored = (
    rollback_snap["allow_ids"] == from_snap["allow_ids"]
    and rollback_snap["baseline_debt_ids"] == from_snap["baseline_debt_ids"]
    and rollback_snap["location_drift_ids"] == from_snap["location_drift_ids"]
    and rollback_snap["stale_ids"] == from_snap["stale_ids"]
)
if not semantic_parity_restored:
    fail(f"post-rollback 0.1.11 semantic state drifted: {rollback_snap} vs {from_snap}")

receipt = {
    "schema_version": 1,
    "schema_id": "cargo-allow.exact-upgrade-rollback-journey.v1",
    "tool": "cargo-allow",
    "result": "Passed",
    "upgrade_verdict": "SupportedUpgrade",
    "rollback_verdict": "RollbackRequiresRestore",
    "claim_boundary": [
        "exact_public_0.1.11",
        "exact_candidate_0.2.0",
        "isolated_three_leg_journey",
        "stateful_previewed_mutation_and_migrate",
        "owned_state_preimage_restore_preserving_unrelated_files",
    ],
    "from": {
        "version": old_version,
        "binary_sha256": old_sha,
        "install_root_id": "old-install",
    },
    "candidate": {
        "version": candidate_version,
        "binary_sha256": candidate_sha,
        "package_set_receipt_sha256": sha256_file(package_receipt_path),
        "git_head": package_receipt["candidate"]["git_head"],
    },
    "rollback": {
        "version": old_version,
        "binary_sha256": old_sha,
        "install_root_id": "old-install",
        "restored_preimage": True,
        "owned_restore_only": True,
        "preimage_policy_sha256": pre_policy_sha,
        "restored_policy_sha256": restored_policy_sha,
        "restored_modified_files": restored_modified_files,
        "removed_owned_files": removed_owned_files,
        "preserved_unrelated_files": [
            "unrelated.txt",
            "target/cargo-allow/unrelated-user.txt",
        ],
        "semantic_parity_restored": semantic_parity_restored,
    },
    "repository": {
        "fixture_sha256": sha256_file(fixture_path),
        "unrelated_file_preserved": True,
        "unrelated_output_root_file_preserved": True,
        "source_checkout_not_used_as_binary": True,
        "optional_profiles_absent": True,
    },
    "fixtures": [
        {
            "id": "clean_strict",
            "policy_sha256": clean_policy_sha,
            "pre_upgrade_allow_ids": [],
            "post_rollback_allow_ids": [],
            "baseline_debt_ids": [],
            "location_drift_ids": [],
            "stale_ids": [],
            "optional_profiles_absent": True,
            "unrelated_controls": [
                {
                    "path": "unrelated.txt",
                    "pre_sha256": clean_unrelated_pre,
                    "post_candidate_sha256": clean_unrelated_post_cand,
                    "post_rollback_sha256": clean_unrelated_post_rb,
                    "preserved": True,
                }
            ],
        },
        {
            "id": "stateful_brownfield_lifecycle",
            "policy_sha256": pre_policy_sha,
            "pre_upgrade_allow_ids": from_snap["allow_ids"],
            "post_rollback_allow_ids": rollback_snap["allow_ids"],
            "baseline_debt_ids": from_snap["baseline_debt_ids"],
            "location_drift_ids": from_snap["location_drift_ids"],
            "stale_ids": from_snap["stale_ids"],
            "optional_profiles_absent": True,
            "unrelated_controls": [
                {
                    "path": "unrelated.txt",
                    "pre_sha256": unrelated_root_pre,
                    "post_candidate_sha256": unrelated_root_post_cand,
                    "post_rollback_sha256": unrelated_root_post_rb,
                    "preserved": True,
                },
                {
                    "path": "target/cargo-allow/unrelated-user.txt",
                    "pre_sha256": unrelated_output_pre,
                    "post_candidate_sha256": unrelated_output_post_cand,
                    "post_rollback_sha256": unrelated_output_post_rb,
                    "preserved": True,
                },
            ],
        },
    ],
    "surface_dispositions": [
        {
            "surface": "policy/allow.toml",
            "category": "policy",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["old-0.1.11:check", "candidate-0.2.0:check", "rollback-0.1.11:check"],
            "notes": "0.1.11 schema_version=0.1 policy with structural selector, legacy line_hint, baseline_debt, and lifecycle dates loads under 0.2.0 without silent broadening and restores byte-identically for 0.1.11.",
        },
        {
            "surface": "doctor",
            "category": "command",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["old-0.1.11:doctor", "candidate-0.2.0:doctor", "rollback-0.1.11:doctor"],
            "notes": "Read-only diagnostics pass on both clean and stateful fixtures with optional profiles absent.",
        },
        {
            "surface": "audit",
            "category": "command",
            "disposition": "ReadUnchanged",
            "evidence_step_ids": ["old-0.1.11:audit", "candidate-0.2.0:audit", "rollback-0.1.11:audit"],
            "notes": "Read-only audit emits valid JSON without mutating repository state.",
        },
        {
            "surface": "check",
            "category": "command",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["old-0.1.11:check", "candidate-0.2.0:check", "candidate-0.2.0:post-mutation-check", "rollback-0.1.11:check"],
            "notes": "no-new gate preserves reviewed and baseline_debt posture across upgrade, post-mutation check, and rollback.",
        },
        {
            "surface": "list",
            "category": "command",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["old-0.1.11:list", "candidate-0.2.0:list", "rollback-0.1.11:list"],
            "notes": "Preserves exact allow_ids, baseline_debt, location_drift, and stale classifications across legs.",
        },
        {
            "surface": "explain",
            "category": "command",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["old-0.1.11:explain", "candidate-0.2.0:explain", "rollback-0.1.11:explain"],
            "notes": "Explains reviewed structural-selector entry allow-0001 across all three legs.",
        },
        {
            "surface": "worklist",
            "category": "command",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["old-0.1.11:worklist", "candidate-0.2.0:worklist", "rollback-0.1.11:worklist"],
            "notes": "Generates bounded JSON worklist without mutating repository state.",
        },
        {
            "surface": "diff",
            "category": "diff",
            "disposition": "ReadUnchanged",
            "evidence_step_ids": ["old-0.1.11:diff", "candidate-0.2.0:diff", "rollback-0.1.11:diff"],
            "notes": "Exact base-commit diff passes identically across all three legs.",
        },
        {
            "surface": "shallow_missing_base_diff",
            "category": "diff",
            "disposition": "ReadUnchanged",
            "evidence_step_ids": ["candidate-0.2.0:diff-missing-base"],
            "notes": "Missing/unresolvable base commit fails closed non-zero without mutating policy state.",
        },
        {
            "surface": "refresh",
            "category": "mutation",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["candidate-0.2.0:refresh-preview", "candidate-0.2.0:refresh-write"],
            "notes": "Previewed --dry-run leaves policy untouched; --write agrees on changed_allow_ids and emits mutation receipt.",
        },
        {
            "surface": "prune",
            "category": "mutation",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["candidate-0.2.0:prune-preview", "candidate-0.2.0:prune-write"],
            "notes": "Previewed --stale --dry-run leaves policy untouched; --write agrees on stale IDs and emits mutation receipt.",
        },
        {
            "surface": "migrate",
            "category": "mutation",
            "disposition": "ReadWithCompatibleProjection",
            "evidence_step_ids": ["candidate-0.2.0:migrate"],
            "notes": "Canonical migration writes owned policy output, emits JSON summary, preserves baseline_debt, and passes check.",
        },
        {
            "surface": "why_add_from_plan_stale_input",
            "category": "artifact",
            "disposition": "TransactionInputRejectedAsStale",
            "evidence_step_ids": ["candidate-0.2.0:why-plan-stale-reject"],
            "notes": "Stale why --plan artifact fails closed under add --from-plan --update without mutating policy/allow.toml.",
        },
    ],
    "mutation_journey": {
        "pre_policy_sha256": pre_policy_sha,
        "post_readonly_policy_sha256": post_readonly_policy_sha,
        "post_mutation_policy_sha256": post_mutation_policy_sha,
        "refresh": {
            "target_allow_id": "allow-0003",
            "preview_unchanged_policy": refresh_preview_unchanged,
            "preview_apply_agree": refresh_agree,
            "mutation_receipt_schema_id": "cargo-allow.refresh.v1",
            "mutation_result": "written",
        },
        "prune": {
            "removed_allow_ids": sorted(prn_write_ids),
            "preview_unchanged_policy": prune_preview_unchanged,
            "preview_apply_agree": prune_agree,
            "mutation_receipt_schema_id": "cargo-allow.prune.v1",
            "mutation_result": "written",
        },
        "migrate": {
            "output_path": migrated_policy_rel,
            "output_sha256": migrated_policy_sha,
            "summary_schema_id": mig_schema_id,
            "preserved_allow_ids": mig_list_snap["allow_ids"],
            "validated_with_check": True,
        },
        "stale_plan_rejection": {
            "plan_path": plan_rel,
            "rejected_nonzero": True,
            "policy_unchanged": stale_plan_policy_unchanged,
            "disposition": "TransactionInputRejectedAsStale",
        },
        "owned_write_set": {
            "modified_files": modified_files,
            "created_owned_files": created_owned_files,
        },
    },
    "steps": steps,
    "negative_controls": [
        {"id": "old_binary_exact_version", "result_class": "ExactVersion", "passed": True},
        {"id": "candidate_binary_exact_version", "result_class": "ExactVersion", "passed": True},
        {"id": "checkout_binary_not_used", "result_class": "CheckoutIsolated", "passed": True},
        {"id": "read_only_commands_no_mutation", "result_class": "ReadOnlyPreserved", "passed": True},
        {"id": "preview_apply_subject_agreement", "result_class": "PreviewApplyAgree", "passed": True},
        {
            "id": "stale_plan_rejected_as_write_authority",
            "result_class": "TransactionInputRejectedAsStale",
            "passed": True,
        },
        {"id": "missing_base_diff_fails_closed", "result_class": "MissingBaseFailClosed", "passed": True},
        {"id": "unrelated_file_survives_rollback", "result_class": "UnrelatedStatePreserved", "passed": True},
        {
            "id": "unrelated_output_root_file_survives_rollback",
            "result_class": "UnrelatedStatePreserved",
            "passed": True,
        },
        {"id": "owned_state_restore_matches_preimage", "result_class": "OwnedStateRestored", "passed": True},
    ],
    "limitations": [
        "candidate_leg_consumes_the_existing_exact_candidate_package_receipt",
        "post_publication_crates_io_0_2_0_leg_requires_separate_refresh_after_publish",
        "rust_1_85_msrv_guidance_is_governed_by_the_separate_msrv_lane",
    ],
}

receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY

python3 "${ROOT}/scripts/validate-upgrade-rollback-journey.py" --receipt "${RECEIPT}" --schema "${SCHEMA}" --fixture "${FIXTURE}"
printf 'exact-upgrade-rollback: passed; receipt: %s\n' "${RECEIPT}"
