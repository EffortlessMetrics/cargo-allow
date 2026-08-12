use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const PUBLISHED_RELEASE_VERSION: &str = "0.1.11";
const PREVIOUS_PUBLISHED_VERSION: &str = "0.1.10";
const PUBLISHED_RELEASE_DOC: &str = "docs/release/0.1.11.md";
const PREVIOUS_RELEASE_DOC: &str = "docs/release/0.1.10.md";
const PUBLISHED_INSTALL_PIN_PHRASE: &str =
    "Public install examples now pin the published `0.1.11` release";
const CANDIDATE_RELEASE_VERSION: &str = "0.2.0";
const PACKAGE_TOPOLOGY: &str = "policy/product-package-topology-v2.toml";

const RELEASE_WORKFLOW: &str = ".github/workflows/release.yml";
const AUTHORIZED_RELEASE_WORKFLOW: &str = ".github/workflows/release-authorized.yml";
const RELEASE_DOC: &str = "docs/release/README.md";
const RELEASE_TOPOLOGY_PUBLISHER: &str = "scripts/release-topology-publisher.py";

const CANDIDATE_RELEASE_DOC: &str = "docs/release/0.2.0.md";
const CANDIDATE_RELEASE_RECORD: &str = include_str!("../../../docs/release/0.2.0.md");
const CAPABILITIES_SOURCE: &str = include_str!("capabilities.rs");

#[test]
fn release_workflow_exists_and_lists_publish_order() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, RELEASE_WORKFLOW);
    let release_doc = read_workspace_file(&root, RELEASE_DOC);
    let topology_publisher = read_workspace_file(&root, RELEASE_TOPOLOGY_PUBLISHER);
    let workspace_manifest = read_workspace_file(&root, "Cargo.toml");
    let workspace_version = workspace_package_version(&workspace_manifest);
    let publish_order = parse_publish_order(&read_workspace_file(
        &root,
        active_publish_order_doc(&workspace_version),
    ));

    assert!(
        workflow.contains("on:") && workflow.contains("tags:") && workflow.contains("v*"),
        "{RELEASE_WORKFLOW} should trigger on version tags"
    );
    assert!(
        workflow.contains("secrets.CARGO_REGISTRY_TOKEN")
            && workflow.contains("source=crates_io_api_token")
            && !workflow.contains("crates-io-auth-action@"),
        "{RELEASE_WORKFLOW} should use the token-backed crates.io authentication contract"
    );
    assert!(
        release_doc.contains(RELEASE_WORKFLOW),
        "{RELEASE_DOC} should document the release workflow"
    );
    assert!(
        workflow.contains(RELEASE_TOPOLOGY_PUBLISHER),
        "{RELEASE_WORKFLOW} should delegate publication to the topology publisher"
    );
    assert!(
        workflow.contains("--mode cargo-allow"),
        "{RELEASE_WORKFLOW} should select the cargo-allow topology family"
    );
    assert!(
        workflow.contains("TOPOLOGY_RECEIPT: target/cargo-allow/topology-publish.receipt.json")
            && workflow.contains("release-manifest-v2.json")
            && !workflow.contains("release-manifest-v1.json")
            && !workflow.contains("ReleaseManifestV1"),
        "{RELEASE_WORKFLOW} should attach the topology-derived V2 manifest only"
    );
    assert!(
        topology_publisher.contains("DEFAULT_TOPOLOGY")
            && topology_publisher.contains("load_rows")
            && topology_publisher.contains("release_order")
            && topology_publisher.contains("schema_version")
            && topology_publisher.contains("logical_id"),
        "{RELEASE_TOPOLOGY_PUBLISHER} should derive publication order from the V2 topology"
    );
    assert!(
        topology_publisher.contains("\"cargo-allow\": {\"shared\", \"cargo-allow\"}"),
        "cargo-allow release mode should include its topology-approved shared dependencies"
    );
    let receipt_schema =
        read_workspace_file(&root, "docs/schemas/topology-publish-receipt.schema.json");
    assert!(
        receipt_schema.contains("cargo-allow.topology-publish-receipt.v1")
            && receipt_schema.contains("logical_id")
            && receipt_schema.contains("schema_version"),
        "topology publish receipt should have a machine-readable contract"
    );

    let authorized = read_workspace_file(&root, AUTHORIZED_RELEASE_WORKFLOW);
    assert!(
        authorized.contains("--mode namespace")
            && !authorized.contains("Push exact v0.2.0 tag")
            && !authorized.contains("gh workflow run release.yml"),
        "{AUTHORIZED_RELEASE_WORKFLOW} must stop after namespace publication"
    );

    assert!(
        !publish_order.is_empty(),
        "the active release document should define a non-empty publish order"
    );
}

#[test]
fn topology_publish_receipt_preserves_incident_recovery_boundary() {
    let root = workspace_root();
    let schema = read_workspace_file(&root, "docs/schemas/topology-publish-receipt.schema.json");
    let publisher = read_workspace_file(&root, RELEASE_TOPOLOGY_PUBLISHER);

    assert!(
        schema.contains("incident_state")
            && schema.contains("first_irreversible_row")
            && schema.contains("release_incident")
            && schema.contains("partial"),
        "topology receipt schema should expose bounded incident and recovery state"
    );
    assert!(
        publisher.contains("incident_state")
            && publisher.contains("first_irreversible_row")
            && publisher.contains("--recovery-receipt")
            && publisher.contains("--authorization")
            && publisher.contains("load_recovery_receipt")
            && publisher.contains("receipt[\"incident_state\"] = \"partial\"")
            && publisher.contains("receipt[\"incident_state\"] = \"release_incident\""),
        "publisher should persist redacted incident state before failing"
    );
    let workflow = read_workspace_file(&root, RELEASE_WORKFLOW);
    assert!(
        workflow.contains("recovery_receipt_run_id")
            && workflow.contains("actions/download-artifact")
            && workflow.contains("run-id: ${{ needs.preflight.outputs.recovery_receipt_run_id }}")
            && workflow.contains("--recovery-receipt")
            && workflow.contains("--authorization")
            && workflow.contains("RECOVERY_RECEIPT: target/cargo-allow/recovery-receipt/topology-publish.receipt.json")
            && workflow.contains("actions: read"),
        "recovery should consume the original run receipt through a bounded artifact identity"
    );
    let authorized = read_workspace_file(&root, AUTHORIZED_RELEASE_WORKFLOW);
    assert!(
        authorized.contains("secrets.CARGO_REGISTRY_TOKEN")
            && authorized.contains("--mode namespace")
            && !authorized.contains("crates-io-auth-action@"),
        "namespace publication should use the shared token-backed publisher"
    );
}

#[test]
fn release_workflow_rehearsal_skips_secret_lookup_but_publication_fails_closed() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, RELEASE_WORKFLOW);
    let token_step = workflow
        .split("      - name: Resolve crates.io API token")
        .nth(1)
        .and_then(|section| {
            section
                .split("      - name: Require crates.io API token for publication")
                .next()
        })
        .unwrap_or_else(|| std::panic::panic_any("release token step should be present"));

    assert!(
        token_step.contains("DRY_RUN:")
            && token_step.contains("if [ \"${DRY_RUN:-false}\" = \"true\" ]")
            && token_step.contains("token lookup skipped")
            && token_step.contains("source=crates_io_api_token")
            && !token_step.contains("CARGO_REGISTRY_TOKEN"),
        "workflow_dispatch rehearsal should record the selected auth class without reading a token"
    );
    let require_token_step = workflow
        .split("      - name: Require crates.io API token for publication")
        .nth(1)
        .and_then(|section| {
            section
                .split("      - name: Publish cargo-allow topology rows")
                .next()
        })
        .unwrap_or_else(|| std::panic::panic_any("publication token step should be present"));
    assert!(
        require_token_step.contains("CARGO_REGISTRY_TOKEN is absent; no upload was attempted")
            && require_token_step.contains("if [ -z \"${CARGO_REGISTRY_TOKEN}\" ]")
            && require_token_step.contains(
                "if: github.event_name != 'workflow_dispatch' || inputs.publish_recovery"
            ),
        "tag and recovery publication should fail closed before upload when the token is absent"
    );

    let publish_step = workflow
        .split("      - name: Publish cargo-allow topology rows")
        .nth(1)
        .and_then(|section| section.split("      - name: Record publish receipt").next())
        .unwrap_or_else(|| std::panic::panic_any("release publish step should be present"));
    assert!(
        publish_step.contains("if [ \"${DRY_RUN}\" = \"true\" ]")
            && publish_step.contains("exit 0")
            && publish_step.contains("--publish")
            && publish_step.contains(
                "CARGO_REGISTRY_TOKEN: ${{ (github.event_name != 'workflow_dispatch' || inputs.publish_recovery) && secrets.CARGO_REGISTRY_TOKEN || '' }}"
            ),
        "rehearsal should exit before the publisher upload path without receiving the token, while real publication retains --publish"
    );
}

#[test]
fn candidate_release_record_exposes_the_checked_capability_contract() {
    assert!(
        CANDIDATE_RELEASE_RECORD.contains("## Scanner capability contract"),
        "{CANDIDATE_RELEASE_DOC} should contain a dedicated capability section"
    );
    let Some((_, after_heading)) =
        CANDIDATE_RELEASE_RECORD.split_once("## Scanner capability contract")
    else {
        return;
    };
    let capability_section = after_heading
        .split_once("\n## ")
        .map_or(after_heading, |(section, _)| section);

    assert!(
        capability_section.contains("cargo-allow capabilities --format json"),
        "{CANDIDATE_RELEASE_DOC} should teach the installed capability command"
    );
    assert!(
        capability_section.contains("cargo-allow.sensor-capabilities.v1"),
        "{CANDIDATE_RELEASE_DOC} should name the versioned capability schema"
    );
    assert!(
        capability_section.contains("generation 1")
            && capability_section.contains("source-tree sensors")
            && capability_section.contains(
                "catalog does not claim compilation, type analysis, macro expansion, MIR"
            )
            && capability_section.contains("runtime behavior, or test adequacy"),
        "{CANDIDATE_RELEASE_DOC} should preserve the capability claim boundary"
    );
    assert!(
        capability_section.contains("#2570")
            && capability_section.contains("docs/support-matrix.toml"),
        "{CANDIDATE_RELEASE_DOC} should link the capability source of truth"
    );
    assert!(
        CAPABILITIES_SOURCE.contains("pub(crate) const SENSOR_CAPABILITY_SCHEMA: &str = \"cargo-allow.sensor-capabilities.v1\""),
        "the release record should remain tied to the CLI capability schema source"
    );
}

#[test]
fn release_workflow_gates_linux_binary_attachment_on_identity_and_attestation() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, RELEASE_WORKFLOW);

    for required in [
        "Build tagged Linux release binary",
        "Package tagged Linux executable archive",
        "Verify tagged Linux executable archive",
        "Attest tagged Linux executable archive",
        "Verify tagged Linux executable attestation",
        "BINARY_PACKAGE_RECEIPT:",
        "BINARY_INSTALL_RECEIPT:",
        "ATTESTATION_VERIFIED=true",
        "gh attestation verify",
        "release-binary.receipt.json",
        "release-binary-install.receipt.json",
        "cargo-allow-${{ github.ref_name }}-x86_64-unknown-linux-gnu.tar.gz",
    ] {
        assert!(
            workflow.contains(required),
            "{RELEASE_WORKFLOW} should contain {required}"
        );
    }
    assert!(
        !workflow.contains("VERSION: \"${GITHUB_REF_NAME#v}\""),
        "{RELEASE_WORKFLOW} should derive the tag-stripped version in the shell"
    );
    assert!(
        workflow.contains("needs: [install-smoke, publish]"),
        "{RELEASE_WORKFLOW} should directly depend on publish for auth_source"
    );

    let package = workflow
        .find("Package tagged Linux executable archive")
        .unwrap_or(usize::MAX);
    let verify = workflow
        .find("Verify tagged Linux executable archive")
        .unwrap_or(usize::MAX);
    let attest = workflow
        .find("Attest tagged Linux executable archive")
        .unwrap_or(usize::MAX);
    let verify_attestation = workflow
        .find("Verify tagged Linux executable attestation")
        .unwrap_or(usize::MAX);
    let manifest = workflow
        .find("Generate release manifest")
        .unwrap_or(usize::MAX);
    let attachment = workflow
        .find("Attach release manifest to GitHub Release")
        .unwrap_or(usize::MAX);
    assert!(
        package < verify
            && verify < attest
            && attest < verify_attestation
            && verify_attestation < manifest
            && manifest < attachment,
        "Linux binary attachment must follow package, clean-install, attestation, and manifest gates"
    );
}

#[test]
fn release_publish_order_matches_internal_dependency_graph() {
    let root = workspace_root();
    let workspace_manifest = read_workspace_file(&root, "Cargo.toml");
    let workspace_version = workspace_package_version(&workspace_manifest);
    let release_doc = fs::read_to_string(root.join(active_publish_order_doc(&workspace_version)))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "read {}: {err}",
                active_publish_order_doc(&workspace_version)
            ))
        });
    let publish_order = parse_publish_order(&release_doc);
    let package_manifests = workspace_package_manifests(&root);
    let package_names = package_manifests.keys().cloned().collect::<BTreeSet<_>>();

    assert_eq!(
        publish_order.iter().cloned().collect::<BTreeSet<_>>(),
        package_names,
        "{PUBLISHED_RELEASE_VERSION} publish order should include every workspace package exactly once"
    );
    assert_eq!(
        publish_order.len(),
        package_names.len(),
        "{PUBLISHED_RELEASE_VERSION} publish order should not contain duplicate packages"
    );

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(index, package)| (package.as_str(), index))
        .collect::<BTreeMap<_, _>>();

    for (package, manifest) in package_manifests {
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

#[test]
fn previous_release_record_keeps_completed_publication_evidence() {
    let root = workspace_root();
    let release_doc = fs::read_to_string(root.join(PREVIOUS_RELEASE_DOC))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {PREVIOUS_RELEASE_DOC}: {err}")));

    assert!(
        release_doc.contains(&format!("# {PREVIOUS_PUBLISHED_VERSION} Release Record")),
        "release record should name the completed patch release"
    );
    assert!(
        release_doc.contains(&format!("to `{PREVIOUS_PUBLISHED_VERSION}`"))
            || release_doc.contains(&format!("completed `{PREVIOUS_PUBLISHED_VERSION}`")),
        "release record should document the completed version bump"
    );
    assert!(
        release_doc.contains(&format!("--version {PREVIOUS_PUBLISHED_VERSION}")),
        "release record should smoke-test the published {PREVIOUS_PUBLISHED_VERSION} binary"
    );
}

#[test]
fn published_release_versions_match_workspace() {
    let root = workspace_root();
    let release_doc = read_workspace_file(&root, PUBLISHED_RELEASE_DOC);
    let workspace_manifest = read_workspace_file(&root, "Cargo.toml");
    let lockfile = read_workspace_file(&root, "Cargo.lock");
    let package_manifests = workspace_package_manifests(&root);
    let workspace_version = workspace_package_version(&workspace_manifest);

    assert!(
        workspace_version == PUBLISHED_RELEASE_VERSION
            || workspace_version == CANDIDATE_RELEASE_VERSION,
        "workspace version should be the published ({PUBLISHED_RELEASE_VERSION}) or candidate ({CANDIDATE_RELEASE_VERSION}) release version, got {workspace_version}"
    );
    assert!(
        release_doc.contains(&format!("# {PUBLISHED_RELEASE_VERSION} Release Record")),
        "release note should name itself as a completed release record"
    );
    assert!(
        release_doc.contains(&format!(
            "Workspace package versions were bumped to `{PUBLISHED_RELEASE_VERSION}`"
        )),
        "release note should document the completed version bump"
    );
    assert!(
        release_doc.contains(PUBLISHED_INSTALL_PIN_PHRASE),
        "release note should document the published install pin"
    );

    // Generation-2 topology is the version authority, not the workspace version
    // alone: product-neutral and intent crates carry their own version source
    // while the cargo-allow family stays on the workspace version.
    let topology_versions =
        topology_package_versions(&read_workspace_file(&root, PACKAGE_TOPOLOGY));
    let package_names = package_manifests.keys().cloned().collect::<BTreeSet<_>>();
    let undeclared = package_names
        .iter()
        .filter(|package| !topology_versions.contains_key(*package))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert!(
        undeclared.is_empty(),
        "{PACKAGE_TOPOLOGY} should declare a version for every publishable workspace package, missing {undeclared:?}"
    );

    for (package, manifest) in &package_manifests {
        assert_eq!(
            &package_declared_version(manifest, &workspace_version),
            expected_package_version(&topology_versions, package),
            "{package} manifest version should match its declared topology version"
        );
    }

    let workspace_dependency_versions =
        workspace_internal_dependency_versions(&workspace_manifest, &package_names);
    let mut expected_workspace_dependency_names = package_names.clone();
    expected_workspace_dependency_names.remove("cargo-allow");
    assert_eq!(
        workspace_dependency_versions
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        expected_workspace_dependency_names,
        "workspace dependencies should version every internal library crate"
    );
    for (dependency, version) in workspace_dependency_versions {
        assert_eq!(
            &version,
            expected_package_version(&topology_versions, &dependency),
            "{dependency} workspace dependency should require its declared topology version"
        );
    }

    let lock_versions = lockfile_package_versions(&lockfile, &package_names);
    assert_eq!(
        lock_versions.keys().cloned().collect::<BTreeSet<_>>(),
        package_names,
        "Cargo.lock should contain every workspace package"
    );
    for (package, version) in lock_versions {
        assert_eq!(
            &version,
            expected_package_version(&topology_versions, &package),
            "{package} lockfile entry should carry its declared topology version"
        );
    }
}

#[test]
fn completed_release_record_versions_match_published_release() {
    let root = workspace_root();
    let release_doc = read_workspace_file(&root, PREVIOUS_RELEASE_DOC);

    assert!(
        release_doc.contains(&format!("# {PREVIOUS_PUBLISHED_VERSION} Release Record")),
        "published release note should name itself as a completed release record"
    );
    assert!(
        release_doc.contains(&format!(
            "completed `{PREVIOUS_PUBLISHED_VERSION}` patch release"
        )),
        "published release note should document the completed patch release"
    );
    assert!(
        release_doc.contains("Published Registry State")
            && release_doc.contains(&format!("cargo-allow {PREVIOUS_PUBLISHED_VERSION}")),
        "published release note should record registry visibility"
    );
    assert!(
        release_doc.contains("Final Verification")
            && release_doc.contains(&format!("cargo-allow {PREVIOUS_PUBLISHED_VERSION}")),
        "published release note should record installed-binary verification"
    );
}

#[test]
fn install_examples_use_published_release() {
    let root = workspace_root();
    let release_doc = read_workspace_file(&root, PUBLISHED_RELEASE_DOC);

    assert!(
        release_doc.contains(PUBLISHED_INSTALL_PIN_PHRASE),
        "release record should note that public install examples pin the published release"
    );

    for relative_path in release_install_surfaces() {
        let content = read_workspace_file(&root, relative_path);
        assert!(
            content.contains(&format!(
                "cargo install cargo-allow --version {PUBLISHED_RELEASE_VERSION} --locked"
            )),
            "{relative_path} should install the published cargo-allow {PUBLISHED_RELEASE_VERSION} release"
        );
        assert!(
            !content.contains(&format!(
                "cargo install cargo-allow --version {PREVIOUS_PUBLISHED_VERSION} --locked"
            )),
            "{relative_path} should not keep the previous {PREVIOUS_PUBLISHED_VERSION} install pin after publication"
        );
    }
}

#[test]
fn release_packages_use_crate_local_readmes() {
    let root = workspace_root();
    let workspace_manifest = read_workspace_file(&root, "Cargo.toml");
    let package_manifests = all_workspace_package_manifest_entries(&root);

    assert!(
        workspace_manifest_contains(
            &workspace_manifest,
            "[workspace.package]",
            r#"readme = "README.md""#
        ),
        "workspace package metadata should keep the root product README"
    );
    for (package, (manifest_path, manifest)) in package_manifests {
        let readme_relative = package_table_value(&manifest, "readme").unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "{} should declare a crate-local readme",
                manifest_path.display()
            ))
        });
        assert_eq!(
            readme_relative, "README.md",
            "{package} should publish its crate-local README"
        );
        assert!(
            !manifest.contains("readme.workspace = true"),
            "{package} should not inherit the root product README as package docs"
        );
        let readme_path = manifest_path
            .parent()
            .unwrap_or_else(|| std::panic::panic_any("crate manifest should have a parent"))
            .join(&readme_relative);
        let readme = fs::read_to_string(&readme_path).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read {}: {err}", readme_path.display()))
        });
        assert!(
            readme.contains(&format!("# {package}")),
            "{} should identify the crate",
            readme_path.display()
        );
        assert!(
            readme.contains("Most users should"),
            "{} should route normal users back to the cargo-allow product",
            readme_path.display()
        );
    }
}

#[test]
fn release_manifest_validate_produces_complete_with_all_checksums() {
    let checksums: Vec<Option<String>> = allow_report::PUBLISH_ORDER
        .iter()
        .map(|_| Some("sha256:abc123".to_string()))
        .collect();
    let manifest = allow_report::generate_release_manifest(&allow_report::ManifestInput {
        version: "0.2.0",
        repository: "EffortlessMetrics/cargo-allow",
        tag: "v0.2.0",
        commit: "abc123",
        tree: "def456",
        auth_source: "oidc",
        workflow_run_id: Some(12345),
        msrv: "1.95",
        platforms_proven: &["linux"],
        crate_checksums: &checksums,
        binary_assets: &[],
        generated_at: "2026-07-20T00:00:00Z",
    });
    let (result, gaps) = allow_report::validate_release_manifest(&manifest);
    assert_eq!(result, allow_report::ManifestResult::Complete);
    assert!(gaps.is_empty());
}

#[test]
fn release_recovery_binds_one_exact_candidate_context() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, RELEASE_WORKFLOW);

    for required in [
        "recovery_commit:",
        "recovery_tree:",
        "recovery_authorization:",
        "steps.release_context.outputs.version",
        "steps.release_context.outputs.commit",
        "steps.release_context.outputs.tree",
        "needs.preflight.outputs.version",
        "needs.preflight.outputs.commit",
        "needs.preflight.outputs.tree",
        "recovery tag does not match the supplied commit/tree",
        "recovery checkout is not the exact tagged commit/tree",
        "publish checkout commit differs from preflight",
        "publish checkout tree differs from preflight",
    ] {
        assert!(
            workflow.contains(required),
            "{RELEASE_WORKFLOW} should contain exact recovery binding `{required}`"
        );
    }

    let publish = workflow
        .split("  publish:")
        .nth(1)
        .and_then(|section| section.split("  install-smoke:").next())
        .unwrap_or_else(|| std::panic::panic_any("publish job should be present"));
    assert!(
        publish.contains("refs/tags/{0}"),
        "recovery publish should check out the preflight-selected tag"
    );
    assert!(
        publish.contains(r#"version="${RELEASE_VERSION}""#),
        "publish should consume the preflight-resolved version"
    );
    assert!(
        !publish.contains("awk"),
        "publish must not recompute the version from the current workspace manifest"
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| std::panic::panic_any("cargo-allow manifest should be under crates/"))
        .to_path_buf()
}

fn release_install_surfaces() -> &'static [&'static str] {
    &[
        "README.md",
        "docs/ci.md",
        "docs/getting-started.md",
        "examples/github-actions/cargo-allow-check.yml",
        "examples/github-actions/cargo-allow-diff.yml",
    ]
}

fn workspace_manifest_contains(manifest: &str, section: &str, expected: &str) -> bool {
    let mut in_section = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if in_section && line == expected {
            return true;
        }
    }
    false
}

fn read_workspace_file(root: &Path, relative_path: &str) -> String {
    fs::read_to_string(root.join(relative_path))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {relative_path}: {err}")))
}

fn workspace_package_manifests(root: &Path) -> BTreeMap<String, String> {
    all_workspace_package_manifests(root)
        .into_iter()
        .filter(|(_, manifest)| is_publishable_workspace_package(manifest))
        .collect()
}

fn all_workspace_package_manifests(root: &Path) -> BTreeMap<String, String> {
    all_workspace_package_manifest_entries(root)
        .into_iter()
        .map(|(package, (_, manifest))| (package, manifest))
        .collect()
}

fn all_workspace_package_manifest_entries(root: &Path) -> BTreeMap<String, (PathBuf, String)> {
    let crates_dir = root.join("crates");
    let entries = fs::read_dir(&crates_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read crates dir: {err}")));
    let mut manifests = BTreeMap::new();
    for entry in entries {
        let entry =
            entry.unwrap_or_else(|err| std::panic::panic_any(format!("read crate dir: {err}")));
        if !entry
            .file_type()
            .unwrap_or_else(|err| std::panic::panic_any(format!("read crate file type: {err}")))
            .is_dir()
        {
            continue;
        }
        let manifest_path = entry.path().join("Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read {}: {err}", manifest_path.display()))
        });
        let Some(package) = manifest_value(&manifest, "name") else {
            std::panic::panic_any(format!(
                "{} should declare package name",
                manifest_path.display()
            ));
        };
        manifests.insert(package, (manifest_path, manifest));
    }
    manifests
}

fn is_publishable_workspace_package(manifest: &str) -> bool {
    !manifest.contains("publish = false")
}

/// The version a crate's own manifest declares.
///
/// A crate either inherits the workspace release line with
/// `version.workspace = true` or pins its own literal.
///
/// Resolution is scoped to the `[package]` table and tolerates a trailing
/// comment, because both looser readings fail silently rather than loudly. A
/// substring test for `"version.workspace = true"` is satisfied by every
/// crate's `rust-version.workspace = true`, making the check vacuous; and an
/// unscoped search for `version = "..."` would happily return the version of a
/// dependency declared under `[dependencies.serde]`, naming an archive that
/// does not exist.
fn package_declared_version(manifest: &str, workspace_version: &str) -> String {
    if package_table_value(manifest, "version.workspace").as_deref() == Some("true") {
        return workspace_version.to_string();
    }
    package_table_value(manifest, "version").unwrap_or_else(|| {
        std::panic::panic_any(
            "package manifest should either inherit the workspace version or declare its own"
                .to_string(),
        )
    })
}

/// Read package version pairs from the generation-2 package topology authority.
fn topology_package_versions(topology: &str) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    let mut current_name = None::<String>;
    for line in topology.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            current_name = None;
            continue;
        }
        if let Some(name) = manifest_value(line, "cargo_package_name") {
            current_name = Some(name);
            continue;
        }
        if let Some(version) = manifest_value(line, "package_version")
            && let Some(name) = current_name.take()
        {
            versions.insert(name, version);
        }
    }
    versions
}

fn expected_package_version<'a>(
    topology_versions: &'a BTreeMap<String, String>,
    package: &str,
) -> &'a String {
    if let Some(version) = topology_versions.get(package) {
        version
    } else {
        std::panic::panic_any(format!("{PACKAGE_TOPOLOGY} should declare {package}"));
    }
}

#[test]
fn package_declared_version_reads_only_the_package_table() {
    let inherited_with_comment = "\
[package]
name = \"inherit\"
version.workspace = true # valid TOML comment
rust-version.workspace = true
";
    assert_eq!(
        package_declared_version(inherited_with_comment, "0.2.0"),
        "0.2.0",
        "a trailing comment must not defeat workspace inheritance"
    );

    // The decoy that an unscoped search would return instead of the real
    // package version.
    let decoy = "\
[package]
name = \"decoy\"
version.workspace = true

[dependencies.serde]
version = \"1.0.219\"
";
    assert_eq!(
        package_declared_version(decoy, "0.2.0"),
        "0.2.0",
        "a dependency table version must never be read as the package version"
    );

    let literal = "\
[package]
name = \"literal\"
version = \"0.1.0\"

[dependencies.serde]
version = \"1.0.219\"
";
    assert_eq!(
        package_declared_version(literal, "0.2.0"),
        "0.1.0",
        "an independently versioned crate keeps its own literal"
    );

    // `rust-version` is the substring that made the previous inheritance check
    // vacuous for every crate in the workspace.
    let rust_version_only = "\
[package]
name = \"rust-version-only\"
version = \"0.3.0\"
rust-version.workspace = true
";
    assert_eq!(
        package_declared_version(rust_version_only, "0.2.0"),
        "0.3.0",
        "rust-version.workspace must not be mistaken for version.workspace"
    );
}

/// Read `key` from the manifest's `[package]` table, ignoring other tables.
///
/// Returns the value with surrounding quotes and any trailing comment removed.
fn package_table_value(manifest: &str, key: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            // `version-something = ...` merely starts with the key text.
            continue;
        };
        let rest = rest.trim();
        return Some(match rest.strip_prefix('"') {
            // Quoted: the value ends at the closing quote, so a trailing
            // comment cannot leak in.
            Some(quoted) => quoted.split('"').next().unwrap_or_default().to_string(),
            // Bare: strip a trailing comment.
            None => rest
                .split('#')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string(),
        });
    }
    None
}

fn active_publish_order_doc(workspace_version: &str) -> &'static str {
    if workspace_version == CANDIDATE_RELEASE_VERSION {
        CANDIDATE_RELEASE_DOC
    } else {
        PUBLISHED_RELEASE_DOC
    }
}

fn parse_publish_order(release_doc: &str) -> Vec<String> {
    let Some(section) = release_doc.split("## Publish Order").nth(1) else {
        std::panic::panic_any(format!(
            "{PUBLISHED_RELEASE_VERSION} release doc should contain Publish Order section"
        ));
    };
    let Some(block) = section.split("```text").nth(1) else {
        std::panic::panic_any("Publish Order section should contain text code block");
    };
    let Some(block) = block.split("```").next() else {
        std::panic::panic_any("Publish Order code block should be closed");
    };

    block
        .lines()
        .filter_map(|line| line.split_once('.').map(|(_, package)| package.trim()))
        .filter(|package| !package.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn internal_workspace_dependencies(
    manifest: &str,
    package_names: &BTreeSet<String>,
) -> BTreeSet<String> {
    manifest
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            let name = name.trim();
            let dependency = name
                .strip_suffix(".workspace")
                .filter(|_| value.trim() == "true")
                .unwrap_or(name);
            if package_names.contains(dependency)
                && (value.contains("workspace = true") || name.ends_with(".workspace"))
            {
                Some(dependency.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn workspace_package_version(workspace_manifest: &str) -> String {
    let mut in_workspace_package = false;
    for line in workspace_manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_workspace_package = line == "[workspace.package]";
            continue;
        }
        if in_workspace_package && let Some(version) = manifest_value(line, "version") {
            return version;
        }
    }
    std::panic::panic_any("workspace manifest should declare workspace.package.version");
}

fn workspace_internal_dependency_versions(
    workspace_manifest: &str,
    package_names: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    workspace_manifest
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            let name = name.trim();
            if package_names.contains(name) {
                Some((name.to_string(), inline_table_version(value)))
            } else {
                None
            }
        })
        .collect()
}

fn inline_table_version(value: &str) -> String {
    value
        .split(',')
        .find_map(|field| manifest_value(field.trim().trim_end_matches('}').trim(), "version"))
        .unwrap_or_else(|| std::panic::panic_any(format!("dependency {value} should set version")))
}

fn lockfile_package_versions(
    lockfile: &str,
    package_names: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    let mut current_name = None::<String>;
    let mut current_version = None::<String>;

    for line in lockfile.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            record_lockfile_package(
                &mut versions,
                package_names,
                current_name.take(),
                current_version.take(),
            );
            continue;
        }
        if current_name.is_none() {
            current_name = manifest_value(line, "name");
        }
        if current_version.is_none() {
            current_version = manifest_value(line, "version");
        }
    }
    record_lockfile_package(&mut versions, package_names, current_name, current_version);
    versions
}

fn record_lockfile_package(
    versions: &mut BTreeMap<String, String>,
    package_names: &BTreeSet<String>,
    name: Option<String>,
    version: Option<String>,
) {
    let Some(name) = name else {
        return;
    };
    if !package_names.contains(&name) {
        return;
    }
    let Some(version) = version else {
        std::panic::panic_any(format!("lockfile package {name} should include version"));
    };
    versions.insert(name, version);
}

fn release_order_index(order_index: &BTreeMap<&str, usize>, package: &str) -> usize {
    if let Some(index) = order_index.get(package) {
        *index
    } else {
        std::panic::panic_any(format!("release publish order should include {package}"));
    }
}

fn manifest_value(manifest: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    manifest.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)?
            .trim()
            .strip_prefix('"')?
            .strip_suffix('"')
            .map(ToOwned::to_owned)
    })
}
