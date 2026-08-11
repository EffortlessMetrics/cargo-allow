#!/usr/bin/env bash
set -euo pipefail

branch="release/token-topology-publication-3389"

git config user.name "EffortlessSteven"
git config user.email "git@effortlesssteven.com"
git fetch origin "${branch}" main
git checkout -B one-shot-release-token-topology "origin/${branch}"
git merge --no-edit -X theirs origin/main

python3 - <<'PY'
from pathlib import Path
import re

manifests = [
    "crates/effortless-rust-source-index/Cargo.toml",
    "crates/intent-model/Cargo.toml",
    "crates/intent-protocol/Cargo.toml",
    "crates/intent-engine/Cargo.toml",
    "crates/intent-edit/Cargo.toml",
    "crates/cargo-intent/Cargo.toml",
    "crates/proof-protocol/Cargo.toml",
    "crates/proof-engine/Cargo.toml",
    "crates/cargo-proof/Cargo.toml",
]
for relative in manifests:
    path = Path(relative)
    text = path.read_text(encoding="utf-8")
    if "publish = false" not in text:
        raise SystemExit(f"{relative} does not contain the expected publish = false gate")
    path.write_text(text.replace("publish = false", "publish = true", 1), encoding="utf-8")

selected = {
    "effortless-rust-source-index",
    "intent-model",
    "intent-protocol",
    "intent-compiler",
    "intent-edit",
    "cargo-intent",
    "proof-protocol",
    "proof-orchestrator",
    "cargo-proof",
}
topology_path = Path("policy/product-package-topology-v2.toml")
topology = topology_path.read_text(encoding="utf-8")
blocks = topology.split("[[package]]")
changed = set()
for index in range(1, len(blocks)):
    block = blocks[index]
    match = re.search(r'^cargo_package_name = "([^"]+)"$', block, re.MULTILINE)
    if match is None or match.group(1) not in selected:
        continue
    if "publish = false" not in block:
        raise SystemExit(f"topology row {match.group(1)} lacks publish = false")
    blocks[index] = block.replace("publish = false", "publish = true", 1)
    changed.add(match.group(1))
if changed != selected:
    raise SystemExit(f"topology publication selection mismatch: {sorted(changed)}")
topology_path.write_text("[[package]]".join(blocks), encoding="utf-8")

workflow_path = Path(".github/workflows/release.yml")
workflow = workflow_path.read_text(encoding="utf-8")
valid_on = '''on:
  push:
    tags:
      - v*
  workflow_dispatch:
    inputs:
      publish_recovery:
        description: "Publish an exact tagged candidate after a partial release; requires all recovery identity inputs"
        required: false
        default: false
        type: boolean
      recovery_version:
        description: "The already-tagged version to recover (e.g. 0.2.0). Must match an existing v* tag."
        required: false
        type: string
      recovery_commit:
        description: "The exact commit currently tagged for the recovery version"
        required: false
        type: string
      recovery_tree:
        description: "The exact tree currently tagged for the recovery version"
        required: false
        type: string
      recovery_authorization:
        description: "Incident/recovery authorization reference for a publishing recovery"
        required: false
        type: string

'''
workflow, count = re.subn(r'on:\n.*?\npermissions:\n', valid_on + 'permissions:\n', workflow, count=1, flags=re.DOTALL)
if count != 1:
    raise SystemExit("release workflow trigger block did not match")

replacement = '''      - name: Resolve crates.io token
        id: registry_token
        env:
          SECRET_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
        run: |
          set -euo pipefail
          if [ -z "${SECRET_TOKEN}" ]; then
            echo "::error::CARGO_REGISTRY_TOKEN is absent; no upload was attempted" >&2
            exit 1
          fi
          echo "source=secret" >> "${GITHUB_OUTPUT}"
          echo "token=${SECRET_TOKEN}" >> "${GITHUB_OUTPUT}"
      - name: Publish topology-selected cargo-allow release
        env:
          CARGO_REGISTRY_TOKEN: ${{ steps.registry_token.outputs.token }}
          DRY_RUN: ${{ github.event_name == 'workflow_dispatch' && !inputs.publish_recovery && 'true' || 'false' }}
        run: |
          set -euo pipefail
          args=(
            --mode cargo-allow
            --receipt target/cargo-allow/topology-publish.receipt.json
          )
          if [ "${DRY_RUN}" != "true" ]; then
            args+=(--publish)
          fi
          python3 scripts/release-topology-publisher.py "${args[@]}"
'''
workflow, count = re.subn(
    r'      - name: Authenticate with crates\.io \(Trusted Publishing\).*?(?=      - name: Record publish receipt)',
    replacement,
    workflow,
    count=1,
    flags=re.DOTALL,
)
if count != 1:
    raise SystemExit("release workflow auth/publish block did not match")
workflow_path.write_text(workflow, encoding="utf-8")

release_test_path = Path("crates/cargo-allow/src/release_prep_tests.rs")
release_test = release_test_path.read_text(encoding="utf-8")
new_workflow_test = '''#[test]
fn release_workflow_uses_token_backed_topology_publication() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, RELEASE_WORKFLOW);
    let release_doc = read_workspace_file(&root, RELEASE_DOC);
    let publisher = read_workspace_file(&root, "scripts/release-topology-publisher.py");

    assert!(
        workflow.contains("on:") && workflow.contains("tags:") && workflow.contains("v*"),
        "{RELEASE_WORKFLOW} should trigger on version tags"
    );
    assert!(
        workflow.contains("CARGO_REGISTRY_TOKEN")
            && workflow.contains("source=secret")
            && !workflow.contains("rust-lang/crates-io-auth-action@"),
        "{RELEASE_WORKFLOW} should use the selected token-backed crates.io path"
    );
    assert!(
        workflow.contains("scripts/release-topology-publisher.py")
            && workflow.contains("--mode cargo-allow")
            && !workflow.contains("crates=("),
        "{RELEASE_WORKFLOW} should derive package names and versions from topology"
    );
    assert!(
        publisher.contains("product-package-topology-v2.toml")
            && publisher.contains("registry checksum conflict")
            && publisher.contains("CARGO_REGISTRY_TOKEN is required before the first upload"),
        "the publisher should bind topology, exact checksums, and pre-upload authentication"
    );
    assert!(
        release_doc.contains(RELEASE_WORKFLOW),
        "{RELEASE_DOC} should document the release workflow"
    );
}

'''
release_test, count = re.subn(
    r'#\[test\]\nfn release_workflow_exists_and_lists_publish_order\(\) \{.*?\n\}\n\n(?=#\[test\])',
    new_workflow_test,
    release_test,
    count=1,
    flags=re.DOTALL,
)
if count != 1:
    raise SystemExit("release workflow contract test did not match")

new_order_test = '''#[test]
fn release_publish_order_matches_internal_dependency_graph() {
    let root = workspace_root();
    let topology = read_workspace_file(&root, PACKAGE_TOPOLOGY);
    let publish_order = topology_publish_order(&topology);
    let package_manifests = workspace_package_manifests(&root)
        .into_iter()
        .filter(|(_, (_, manifest))| is_publishable_workspace_package(manifest))
        .collect::<BTreeMap<_, _>>();
    let package_names = package_manifests.keys().cloned().collect::<BTreeSet<_>>();

    assert_eq!(
        publish_order.iter().cloned().collect::<BTreeSet<_>>(),
        package_names,
        "{PACKAGE_TOPOLOGY} publish rows should include every publishable workspace package exactly once"
    );
    assert_eq!(
        publish_order.len(),
        package_names.len(),
        "{PACKAGE_TOPOLOGY} publish order should not contain duplicate packages"
    );

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(index, package)| (package.as_str(), index))
        .collect::<BTreeMap<_, _>>();

    for (package, (_, manifest)) in package_manifests {
        let package_index = release_order_index(&order_index, package.as_str());
        for dependency in internal_workspace_dependencies(&manifest, &package_names) {
            let dependency_index = release_order_index(&order_index, dependency.as_str());
            assert!(
                dependency_index < package_index,
                "{package} depends on {dependency}, so {dependency} must be published first"
            );
        }
    }
}

'''
release_test, count = re.subn(
    r'#\[test\]\nfn release_publish_order_matches_internal_dependency_graph\(\) \{.*?\n\}\n\n(?=#\[test\])',
    new_order_test,
    release_test,
    count=1,
    flags=re.DOTALL,
)
if count != 1:
    raise SystemExit("release publish-order test did not match")

order_helper = '''
fn topology_publish_order(topology: &str) -> Vec<String> {
    let value: toml::Value = toml::from_str(topology)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse {PACKAGE_TOPOLOGY}: {err}")));
    let packages = value
        .get("package")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any(format!("{PACKAGE_TOPOLOGY} has no package rows")));
    let mut rows = packages
        .iter()
        .filter_map(|package| {
            let table = package.as_table().unwrap_or_else(|| {
                std::panic::panic_any(format!("{PACKAGE_TOPOLOGY} package row is not a table"))
            });
            if table.get("publish").and_then(toml::Value::as_bool) != Some(true) {
                return None;
            }
            let name = table
                .get("cargo_package_name")
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{PACKAGE_TOPOLOGY} publish row has no cargo_package_name"
                    ))
                })
                .to_string();
            let order = table
                .get("release_order")
                .and_then(toml::Value::as_integer)
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{PACKAGE_TOPOLOGY} publish row {name} has no release_order"
                    ))
                });
            Some((order, name))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.cmp(right));
    rows.into_iter().map(|(_, name)| name).collect()
}

'''
marker = "fn expected_package_version<'a>("
if "fn topology_publish_order(" not in release_test:
    if marker not in release_test:
        raise SystemExit("expected_package_version marker missing")
    release_test = release_test.replace(marker, order_helper + marker, 1)
release_test_path.write_text(release_test, encoding="utf-8")

fragment = Path(".changes/Changed-20260811-token-topology-publication.yaml")
fragment.write_text(
    "kind: Changed\n"
    "body: >-\n"
    "  Publish the mixed-version shared, intent, proof, and cargo-allow package graph\n"
    "  from the selected V2 topology with token-backed authentication and exact\n"
    "  crates.io checksum verification.\n",
    encoding="utf-8",
)

readme_path = Path("docs/release/README.md")
readme = readme_path.read_text(encoding="utf-8")
marker = "## Token-backed topology publication"
if marker not in readme:
    readme += '''

## Token-backed topology publication

The selected V2 topology is the package, version, family, and release-order
authority. `release-authorized.yml` publishes the twelve shared/intent/proof
`0.1.0` rows only after an exact authorization commit and checksum-verifies
each row before pushing `v0.2.0`. The tag-triggered `release.yml` then publishes
the ten cargo-allow `0.2.0` rows through the same publisher, runs Linux and
Windows crates.io install smoke tests, verifies the release assets and
attestation, and creates the GitHub Release. Both workflows require the
`CARGO_REGISTRY_TOKEN` repository secret and fail before the first upload when
it is absent.
'''
    readme_path.write_text(readme, encoding="utf-8")
PY

cargo fmt --all
python3 -m py_compile scripts/release-topology-publisher.py
python3 scripts/release-topology-publisher.py --mode namespace --list > target/namespace-release-order.txt
python3 scripts/release-topology-publisher.py --mode cargo-allow --list > target/cargo-allow-release-order.txt
[ "$(wc -l < target/namespace-release-order.txt)" -eq 12 ]
[ "$(wc -l < target/cargo-allow-release-order.txt)" -eq 10 ]

# Remove branch-local automation before proving or committing the release diff.
git checkout origin/main -- scripts/check-msrv-consistency.sh
rm -f scripts/one-shot-release-token-topology.sh

cargo fmt --all -- --check
cargo test -p cargo-allow --bin cargo-allow release_prep --locked
cargo run -p cargo-allow -- check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
git diff --check

git add -A
git diff --cached --check
git commit -m "build(release): publish mixed-version graph from V2 topology"
git push origin HEAD:"${branch}"
