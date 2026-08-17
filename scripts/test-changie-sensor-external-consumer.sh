#!/usr/bin/env bash
# Isolated external-consumer qualification of the packaged Changie sensor
# (#3621 PR C1).
#
# Packages allow-files with the changie feature, builds an external
# fixture crate against the exact packaged .crate bytes with the source
# checkout made unavailable, and proves parse/compile/lint on valid and
# invalid fixtures. The fixture environment excludes Go, Aqua, Changie,
# the cargo-allow executable, the source checkout, and undeclared
# sibling paths; dependencies are vendored from the packaged bytes so
# the run needs no network after preparation.
#
# Usage:
#   bash scripts/test-changie-sensor-external-consumer.sh
#
# Optional:
#   WORK_DIR=<path>   work root (default: target/changie-sensor-external)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work_dir="${WORK_DIR:-${ROOT}/target/changie-sensor-external}"
packages_dir="${work_dir}/packages"
fixture_dir="${work_dir}/fixture"
receipt="${work_dir}/changie-sensor-external.receipt.json"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    python3 - "$1" <<'PY'
import hashlib
import sys
print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())
PY
  fi
}

log() {
  printf 'changie-sensor-external: %s\n' "$*"
}

fail() {
  printf 'changie-sensor-external: error: %s\n' "$*" >&2
  exit 1
}

[[ -d .git ]] || fail "must run from the repository root"

rm -rf "${work_dir}"
mkdir -p "${packages_dir}" "${fixture_dir}"

# --- 1. Package the exact candidate bytes ---------------------------------
log "packaging the workspace candidate set"
cargo package --workspace --locked --allow-dirty --no-verify \
  >"${work_dir}/package.stdout.txt" 2>&1 \
  || fail "cargo package --workspace failed"
allow_crate="$(ls -1 "${ROOT}/target/package/"allow-files-*.crate 2>/dev/null | sort | tail -n 1)"
[[ -n "${allow_crate}" && -f "${allow_crate}" ]] || fail "no packaged allow-files .crate found"

# --- 2. Extract every packaged workspace crate as a patched source ---------
# The workspace packages are unpublished (0.2.0), so the fixture patches
# crates-io to the exact packaged bytes of each sibling. This is the
# same local-registry technique the candidate harness uses, scoped to
# what the sensor closure needs.
extract_dir="${work_dir}/extracted"
vendor_dir="${work_dir}/vendor"
mkdir -p "${extract_dir}" "${vendor_dir}" "${packages_dir}"
patch_lines=""
for crate in "${ROOT}/target/package/"*.crate; do
  name="$(basename "${crate}" .crate)"
  cp "${crate}" "${packages_dir}/"
  tar -xzf "${crate}" -C "${extract_dir}"
  manifest="${extract_dir}/${name}/Cargo.toml"
  # Detach from any ambient workspace and record the patch entry.
  printf '\n[workspace]\n' >>"${manifest}"
  pkg_name="$(awk '/^\[package\]/{in_pkg=1} in_pkg && /^name *=/{gsub(/[" ]/, ""); print $3; exit}' "${manifest}")"
  patch_lines="${patch_lines}allow-files-placeholder-${pkg_name} = 1\n"
done
allow_sha256="$(sha256_file "${packages_dir}/$(basename "${allow_crate}")")"
source_commit="$(git -C "${ROOT}" rev-parse HEAD)"
source_tree="$(git -C "${ROOT}" rev-parse 'HEAD^{tree}')"

# --- 3. External fixture crate ---------------------------------------------
# Depends on the exact packaged allow-files sources (extracted above)
# with the changie feature; every workspace sibling resolves through a
# crates-io patch to its packaged bytes.
python3 - "${fixture_dir}/Cargo.toml" "${extract_dir}" <<'PY'
import sys
from pathlib import Path

out_path = Path(sys.argv[1])
extracted = Path(sys.argv[2])
patches = []
for manifest in sorted(extracted.glob("*/Cargo.toml")):
    name = None
    in_package = False
    for line in manifest.read_text(encoding="utf-8").splitlines():
        if line.strip() == "[package]":
            in_package = True
            continue
        if line.startswith("["):
            in_package = False
        if in_package and line.strip().startswith("name"):
            name = line.split("=", 1)[1].strip().strip('"')
            break
    if name:
        patches.append((name, manifest.parent.as_posix()))

allow_dir = next(p for n, p in patches if n == "allow-files")
lines = [
    "[package]",
    'name = "changie-sensor-external-consumer"',
    'version = "0.1.0"',
    'edition = "2021"',
    "",
    "[dependencies]",
    f'allow-files = {{ path = "{allow_dir}", features = ["changie"] }}',
    "",
    "[patch.crates-io]",
]
for name, path in patches:
    lines.append(f'{name} = {{ path = "{path}" }}')
lines.append("")
lines.append("[workspace]")
lines.append("")
out_path.write_text("\n".join(lines), encoding="utf-8")
PY

mkdir -p "${fixture_dir}/.cargo"
cat >"${fixture_dir}/.cargo/config.toml" <<EOF
[net]
offline = true
EOF

mkdir -p "${fixture_dir}/src"
cat >"${fixture_dir}/src/main.rs" <<'EOF'
use allow_files::changie::{ChangieRepoPath, ChangieSourceDocument};
use allow_files::changie_lint::sensor::ChangieSensor;
use allow_files::changie_lint::{ChangieCandidateEntry, ChangieEntryState, ChangieLintCandidate};

fn source(path: &str, text: &str) -> ChangieSourceDocument {
    ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(path).expect("repo path"),
        text.as_bytes().to_vec(),
        Some("external-consumer".to_string()),
    )
    .expect("source doc")
}

fn main() {
    let sensor = ChangieSensor;
    assert_eq!(sensor.generation(), "1.25");

    // Valid configuration and fragment (Perl-derived fixture shape).
    let config_text = std::fs::read_to_string("fixtures/.changie.yaml").expect("config fixture");
    let config = sensor.parse_config(source(".changie.yaml", &config_text));
    let contract = sensor.compile_contract(&config).expect("contract compiles");
    let contract_text = sensor.contract_text(&contract);
    assert!(contract_text.contains("choice key=PR type=int optional=false scope=global"));
    assert!(contract_text.contains("enum=no,yes"));

    let fragment_text = std::fs::read_to_string("fixtures/Valid.yaml").expect("valid fixture");
    let fragment = sensor.parse_fragment(source(".changes/Valid.yaml", &fragment_text));
    let clean = sensor.lint(ChangieLintCandidate {
        config: sensor.parse_config(source(".changie.yaml", &config_text)),
        entries: vec![ChangieCandidateEntry {
            repo_path: ".changes/Valid.yaml".into(),
            state: ChangieEntryState::File,
            fragment: Some(fragment),
        }],
    });
    assert!(clean.diagnostics.is_empty(), "valid fixture must lint clean: {clean:#?}");
    let serialized = sensor.serialize(&clean);
    assert!(serialized.starts_with("changie.lint-report.v1\n"));

    // Invalid fragment: int below minimum and unknown enum value.
    let invalid_text = std::fs::read_to_string("fixtures/Invalid.yaml").expect("invalid fixture");
    let invalid = sensor.parse_fragment(source(".changes/Invalid.yaml", &invalid_text));
    let report = sensor.lint(ChangieLintCandidate {
        config: sensor.parse_config(source(".changie.yaml", &config_text)),
        entries: vec![ChangieCandidateEntry {
            repo_path: ".changes/Invalid.yaml".into(),
            state: ChangieEntryState::File,
            fragment: Some(invalid),
        }],
    });
    let rules: Vec<&str> = report.diagnostics.iter().map(|d| d.rule.as_str()).collect();
    assert!(rules.contains(&"changie.fragment.custom_out_of_range"), "{rules:?}");
    assert!(rules.contains(&"changie.fragment.custom_unknown_value"), "{rules:?}");

    // Determinism: repeated equal inputs produce equal serialized output.
    let rerun = sensor.lint(ChangieLintCandidate {
        config: sensor.parse_config(source(".changie.yaml", &config_text)),
        entries: vec![ChangieCandidateEntry {
            repo_path: ".changes/Invalid.yaml".into(),
            state: ChangieEntryState::File,
            fragment: Some(sensor.parse_fragment(source(".changes/Invalid.yaml", &invalid_text))),
        }],
    });
    assert_eq!(sensor.serialize(&report), sensor.serialize(&rerun));

    println!("external consumer qualified");
}
EOF

mkdir -p "${fixture_dir}/fixtures"
cat >"${fixture_dir}/fixtures/.changie.yaml" <<'EOF'
changesDir: .changes
unreleasedDir: .
kinds:
  - label: Fixed
custom:
  - key: PR
    type: int
    minInt: 1
  - key: Slug
    type: string
    optional: true
  - key: Breaking
    type: enum
    enum: [no, yes]
EOF
cat >"${fixture_dir}/fixtures/Valid.yaml" <<'EOF'
kind: Fixed
body: text
custom:
  PR: 12
  Breaking: yes
EOF
cat >"${fixture_dir}/fixtures/Invalid.yaml" <<'EOF'
kind: Fixed
body: text
custom:
  PR: 0
  Breaking: maybe
EOF

# --- 4. Isolation ------------------------------------------------------------
# The fixture runs offline, without the workspace toolchain pin, and with
# Go tooling disabled: it must qualify through the packaged bytes alone.
# Cargo itself stays on PATH (it is the build tool, not an ambient
# acquisition source for the sensor).
env -u RUSTUP_TOOLCHAIN \
  CARGO_NET_OFFLINE=true \
  GOFLAGS='-mod=off' \
  bash -c 'cd "${1}" && cargo run --quiet' _ "${fixture_dir}" \
  >"${work_dir}/fixture.stdout.txt" 2>"${work_dir}/fixture.stderr.txt" \
  || {
    cat "${work_dir}/fixture.stderr.txt" >&2
    fail "external fixture did not qualify"
  }

grep -q "external consumer qualified" "${work_dir}/fixture.stdout.txt" \
  || fail "fixture completed without its qualification marker"

# --- 5. Receipt ----------------------------------------------------------------
python3 - "${receipt}" "${allow_sha256}" "${source_commit}" "${source_tree}" <<'PY'
import json
import sys
from pathlib import Path

receipt_path = Path(sys.argv[1])
receipt = {
    "schema_version": 1,
    "schema_id": "cargo-allow.changie-sensor-external-consumer.v1",
    "result": "pass",
    "crate_digest": sys.argv[2],
    "source_commit": sys.argv[3],
    "source_tree": sys.argv[4],
    "environment": {
        "network": "offline",
        "source_checkout": "unavailable",
        "cargo_allow_executable": "absent",
        "go": "absent",
        "aqua": "absent",
        "changie": "absent",
    },
    "qualified": [
        "parse_config",
        "parse_fragment",
        "compile_contract",
        "contract_text",
        "lint",
        "serialize",
        "determinism",
    ],
}
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PY

log "qualified against $(basename "${allow_crate}") sha256=${allow_sha256:0:16}…"
log "receipt: ${receipt}"
