#!/usr/bin/env bash
# DefaultSensorBurdenAuditV1 harness (#3883).
#
# Measures the current default sensor and policy burden on one real subject
# (this repository, read-only) and two temporary synthetic fixtures (clean
# small library, brownfield ordinary repository). Synthetic fixtures are
# generated at runtime into the work directory and are labeled synthetic in
# the receipt; no external repository is mutated and no live policy is
# changed. Emits target/sensor-burden/default-sensor-burden.receipt.json.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work="${WORK_DIR:-${ROOT}/target/sensor-burden}"
receipt="${work}/default-sensor-burden.receipt.json"
schema_id="cargo-allow.default-sensor-burden-audit.v1"

log() {
  printf 'default-sensor-burden-audit: %s\n' "$*"
}

fail() {
  printf 'default-sensor-burden-audit: error: %s\n' "$*" >&2
  exit 1
}

py="${PYTHON3:-python3}"
if ! command -v "${py}" >/dev/null 2>&1; then
  if command -v python >/dev/null 2>&1 && python -c 'import sys; sys.exit(0 if sys.version_info[0] >= 3 else 1)' >/dev/null 2>&1; then
    py=python
  else
    fail "python3 is required"
  fi
fi
mkdir -p "${work}"

bin="${CARGO_ALLOW_BIN:-${ROOT}/target/debug/cargo-allow}"
if [[ ! -x "${bin}" ]] && [[ -x "${bin}.exe" ]]; then
  bin="${bin}.exe"
fi
[[ -x "${bin}" ]] || fail "cargo-allow binary not found at ${bin}; build with: cargo build -p cargo-allow"
bin_python="$("${py}" -c 'import sys; print(sys.argv[1].replace("\\", "/"))' "$(cygpath -m "${bin}" 2>/dev/null || echo "${bin}")")"

repo_commit="$(git rev-parse HEAD)"
repo_tree="$(git rev-parse "HEAD^{tree}")"

# ---------------------------------------------------------------------------
# Fixture 1: synthetic clean small library (no findings at all).
# ---------------------------------------------------------------------------
clean_root="${work}/fixture-clean"
rm -rf "${clean_root}"
mkdir -p "${clean_root}/src"
cat > "${clean_root}/Cargo.toml" <<'TOML'
[package]
name = "burden-clean-fixture"
version = "0.1.0"
edition = "2021"
TOML
cat > "${clean_root}/src/lib.rs" <<'RS'
//! Synthetic clean fixture for the #3883 default sensor burden audit.

pub fn add(left: u64, right: u64) -> u64 {
    left.saturating_add(right)
}

#[cfg(test)]
mod tests {
    use super::add;

    #[test]
    fn adds() {
        assert!(add(1, 1) > 0);
    }
}
RS
(
  cd "${clean_root}"
  git init -q
  "${bin}" init >/dev/null
  git add -A
  git -c user.email=audit@invalid -c user.name=burden-audit commit -qm "synthetic clean fixture"
)

# ---------------------------------------------------------------------------
# Fixture 2: synthetic brownfield ordinary repository (Rust findings plus the
# ordinary docs/config/CI/script/metadata presence families).
# ---------------------------------------------------------------------------
brown_root="${work}/fixture-brownfield"
rm -rf "${brown_root}"
mkdir -p "${brown_root}/src" "${brown_root}/tests" "${brown_root}/.github/workflows" "${brown_root}/scripts" "${brown_root}/docs" "${brown_root}/config"
cat > "${brown_root}/Cargo.toml" <<'TOML'
[package]
name = "burden-brownfield-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1"
TOML
cat > "${brown_root}/src/lib.rs" <<'RS'
//! Synthetic brownfield fixture for the #3883 default sensor burden audit.

pub struct Registry {
    entries: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            entries: vec![String::from("alpha"), String::from("beta")],
        }
    }

    pub fn first(&self) -> &str {
        self.entries.first().expect("registry is never empty")
    }

    pub fn get(&self, index: usize) -> &str {
        &self.entries[index]
    }
}

pub fn load(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}
RS
cat > "${brown_root}/tests/smoke.rs" <<'RS'
#[test]
fn smoke() {
    assert_eq!(1 + 1, 2);
}
RS
cat > "${brown_root}/README.md" <<'MD'
# burden-brownfield-fixture

Synthetic brownfield subject for the default sensor burden audit.
MD
cat > "${brown_root}/.github/workflows/ci.yml" <<'YML'
name: ci
on: [push]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test
YML
cat > "${brown_root}/scripts/build.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
cargo build "$@"
SH
cat > "${brown_root}/config/default.toml" <<'TOML'
# Ordinary configuration file.
level = "info"
TOML
(
  cd "${brown_root}"
  git init -q
  "${bin}" init >/dev/null
  git add -A
  git -c user.email=audit@invalid -c user.name=burden-audit commit -qm "synthetic brownfield fixture"
)

# ---------------------------------------------------------------------------
# Measurement driver: python3 orchestrates the CLI invocations per scenario,
# captures JSON artifacts, times the honest no-new routes, and asserts the
# audit invariants.
# ---------------------------------------------------------------------------
export BURDEN_BIN="${bin_python}"
export BURDEN_CLEAN="${clean_root}"
export BURDEN_BROWN="${brown_root}"
export BURDEN_REPO="${ROOT}"
export BURDEN_RECEIPT="${receipt}"
export BURDEN_SCHEMA_ID="${schema_id}"
export BURDEN_COMMIT="${repo_commit}"
export BURDEN_TREE="${repo_tree}"
export BURDEN_WORK="${work}"

"$py" - <<'PY'
import json, os, subprocess, time
from collections import Counter
from pathlib import Path

bin_ = os.environ["BURDEN_BIN"]
clean = os.environ["BURDEN_CLEAN"]
brown = os.environ["BURDEN_BROWN"]
repo = os.environ["BURDEN_REPO"]
work = Path(os.environ["BURDEN_WORK"])
receipt_path = Path(os.environ["BURDEN_RECEIPT"])
schema_id = os.environ["BURDEN_SCHEMA_ID"]

def run(cmd, cwd, timeout=300):
    t0 = time.monotonic()
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    return p, round(time.monotonic() - t0, 3)

subjects = []
failures = []

def require(cond, msg):
    if not cond:
        failures.append(msg)

# --- Subject: synthetic clean small library ---------------------------------
# The measured clean-repository posture is the audit datum, not a harness
# failure: with an empty policy the default sensor may report findings on a
# pristine subject, and that observation is recorded verbatim.
p, t_check = run([bin_, "check", "--mode", "no-new", "--format", "json"], clean)
clean_check = json.loads(p.stdout)
clean_no_new_green = p.returncode == 0

p, t_audit = run([bin_, "audit", "--format", "json"], clean)
clean_audit = json.loads(p.stdout)
clean_findings = clean_audit["summary"]["findings"]
clean_finding_rows = [
    {"kind": f["kind"], "family": f["family"], "path": f["path"]}
    for f in clean_audit.get("findings", [])
]

p, t_propose = run([bin_, "propose", "--write", "target/candidate.toml",
                    "--summary-format", "json", "--summary-output", "target/propose-summary.json"], clean)
require(p.returncode == 0, f"clean propose exited {p.returncode}: {p.stderr[-400:]}")
clean_propose = json.loads((Path(clean) / "target" / "propose-summary.json").read_text(encoding="utf-8"))
clean_candidate = (Path(clean) / "target" / "candidate.toml").read_text(encoding="utf-8")
clean_candidate_rows = clean_candidate.count("[[allow]]")
clean_candidate_families = Counter(
    line.split("=", 1)[1].strip().strip('"')
    for line in clean_candidate.splitlines()
    if line.strip().startswith("family =")
)

# The gate must be reachable honestly once the proposed candidate covers the
# observed findings: no-new against the candidate policy, still no live write.
p, t_clean_green = run([bin_, "check", "--config", "target/candidate.toml",
                        "--mode", "no-new", "--format", "json"], clean)
require(p.returncode == 0, f"clean check against candidate policy exited {p.returncode}: {p.stderr[-400:]}")

subjects.append({
    "subject": "synthetic_clean_small_library",
    "provenance": "synthetic_runtime_fixture",
    "root": clean.replace("\\", "/"),
    "tracked_files": clean_audit["inventory"]["files_scanned"],
    "findings_before_policy": clean_findings,
    "findings_on_empty_policy": clean_finding_rows,
    "no_new_with_empty_policy_green": clean_no_new_green,
    "check_no_new_seconds": t_check,
    "propose_generated_rows": clean_candidate_rows,
    "propose_rows_by_family": dict(sorted(clean_candidate_families.items(), key=lambda kv: -kv[1])),
    "no_new_against_candidate_green": True,
    "operator_disposition": "noisy_first_run_presence_registration_on_pristine_subject",
})

# --- Subject: synthetic brownfield ordinary repository ----------------------
p, t_audit = run([bin_, "audit", "--format", "json"], brown)
brown_audit = json.loads(p.stdout)
brown_findings = brown_audit["summary"]["findings"]

family_rows = Counter()
kind_rows = Counter()
for f in brown_audit.get("findings", []):
    family_rows[f["family"]] += 1
    kind_rows[f"{f['kind']}.{f['family']}"] += 1

p, t_propose = run([bin_, "propose", "--write", "target/candidate.toml",
                    "--summary-format", "json", "--summary-output", "target/propose-summary.json"], brown)
require(p.returncode == 0, f"brownfield propose exited {p.returncode}: {p.stderr[-400:]}")
brown_propose = json.loads((Path(brown) / "target" / "propose-summary.json").read_text(encoding="utf-8"))

candidate = (Path(brown) / "target" / "candidate.toml").read_text(encoding="utf-8")
candidate_rows = candidate.count("[[allow]]")
candidate_glob_rows = sum(1 for line in candidate.splitlines()
                          if line.strip().startswith("path =") and "*" in line)
candidate_families = Counter(
    line.split("=", 1)[1].strip().strip('"')
    for line in candidate.splitlines()
    if line.strip().startswith("family =")
)

# Honest no-new against the proposed candidate policy, no live mutation.
p, t_green = run([bin_, "check", "--config", "target/candidate.toml",
                  "--mode", "no-new", "--format", "json"], brown)
require(p.returncode == 0, f"brownfield check against candidate policy exited {p.returncode}: {p.stdout[-400:]} {p.stderr[-400:]}")
brown_green = json.loads(p.stdout)

# Deliberate post-baseline finding must turn the gate red and name the path.
lib = Path(brown) / "src" / "lib.rs"
original = lib.read_text(encoding="utf-8")
lib.write_text(original + "\npub fn late() -> String {\n    let v: Vec<u8> = Vec::new();\n    v.first().unwrap().to_string()\n}\n", encoding="utf-8")
p, t_red = run([bin_, "check", "--config", "target/candidate.toml",
                "--mode", "no-new", "--format", "json"], brown)
red_output = p.stdout + p.stderr
require(p.returncode != 0, "deliberate post-baseline finding did not turn no-new red")
require("src/lib.rs" in red_output, "red no-new result does not name the new finding path")
lib.write_text(original, encoding="utf-8")
p, t_restored = run([bin_, "check", "--config", "target/candidate.toml",
                     "--mode", "no-new", "--format", "json"], brown)
require(p.returncode == 0, "gate did not return green after the deliberate finding was removed")

subjects.append({
    "subject": "synthetic_brownfield_ordinary_repository",
    "provenance": "synthetic_runtime_fixture",
    "root": brown.replace("\\", "/"),
    "tracked_files": brown_audit["inventory"]["files_scanned"],
    "findings_before_policy": brown_findings,
    "findings_by_family": dict(sorted(family_rows.items(), key=lambda kv: -kv[1])),
    "propose_generated_rows": candidate_rows,
    "propose_generated_glob_rows": candidate_glob_rows,
    "propose_rows_by_family": dict(sorted(candidate_families.items(), key=lambda kv: -kv[1])),
    "propose_summary": brown_propose,
    "no_new_against_candidate_status": brown_green.get("status"),
    "commands_to_honest_no_new": ["propose --write", "check --config <candidate> --mode no-new"],
    "command_seconds_to_honest_no_new": round(t_propose + t_green, 3),
    "deliberate_finding_turned_red": True,
    "red_result_named_path": "src/lib.rs",
    "gate_green_after_removal": True,
    "operator_disposition": "see report: presence-family share of generated rows",
})

# --- Subject: this repository (self-hosted stress fixture, read-only) -------
p, t_self = run([bin_, "audit", "--format", "json"], repo, timeout=600)
self_audit = json.loads(p.stdout)
self_summary = self_audit["summary"]
self_family = Counter()
for f in self_audit.get("findings", []):
    self_family[f["family"]] += 1
presence_families = {"configuration", "documentation", "test_fixture", "shell_script",
                     "python_tool", "release_script", "package_metadata", "ci_declarative",
                     "unknown_non_rust"}
self_presence = sum(v for k, v in self_family.items() if k in presence_families)

subjects.append({
    "subject": "cargo_allow_self_hosted",
    "provenance": "real_repository_read_only",
    "commit": os.environ["BURDEN_COMMIT"],
    "tree": os.environ["BURDEN_TREE"],
    "root": repo.replace("\\", "/"),
    "tracked_files": self_audit["inventory"]["files_scanned"],
    "findings_before_policy": self_summary["findings"],
    "matched": self_summary["matched"],
    "new": self_summary["new"],
    "stale": self_summary["stale"],
    "location_drift": self_summary["location_drift"],
    "review_due": self_summary["review_due"],
    "weak_evidence_references": self_summary["weak_evidence_references"],
    "findings_by_family_top": dict(sorted(self_family.items(), key=lambda kv: -kv[1])[:12]),
    "presence_only_family_findings": self_presence,
    "operator_disposition": "governance_pressure_is_useful_here_by_explicit_receipts",
})

receipt = {
    "schema_id": schema_id,
    "tool": "cargo-allow",
    "kind": "default_sensor_burden_audit",
    "claim_boundary": [
        "read_only_audit_no_external_repository_mutated",
        "synthetic_fixtures_are_labeled_synthetic",
        "presence_only_findings_are_not_content_risk_findings",
        "not_a_cargo_allow_cli_product_artifact",
        "profile_variant_scenarios_not_proven_no_profile_selection_surface_yet",
    ],
    "subject_identities": {
        "harness_source_commit": os.environ["BURDEN_COMMIT"],
        "harness_source_tree": os.environ["BURDEN_TREE"],
    },
    "subjects": subjects,
    "scenario_posture": {
        "current_default": "proven",
        "clean_strict_empty_policy": "proven",
        "brownfield_propose_no_new_baseline": "proven",
        "deliberate_post_baseline_finding": "proven",
        "syntax_high_signal_presence_only": "not_proven_no_profile_selection_surface",
        "generic_docs_config_advisory": "not_proven_no_profile_selection_surface",
        "complete_tracked_file_governance_opt_in": "not_proven_no_profile_selection_surface",
    },
    "invariant_failures": failures,
}

receipt_path.parent.mkdir(parents=True, exist_ok=True)
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
print(json.dumps({
    "receipt": str(receipt_path).replace("\\", "/"),
    "subjects": [s["subject"] for s in subjects],
    "invariant_failures": failures,
}, indent=1))
PY

if ! "$py" - "${receipt}" <<'PY'
import json, sys
receipt = json.load(open(sys.argv[1], encoding="utf-8"))
failures = receipt.get("invariant_failures") or []
if failures:
    for f in failures:
        print(f"default-sensor-burden-audit: invariant failure: {f}", file=sys.stderr)
    raise SystemExit(1)
PY
then
  fail "invariant failures recorded in ${receipt}"
fi

log "receipt: ${receipt}"
log "pass"
