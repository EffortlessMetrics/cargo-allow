use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const PUBLISHED_RELEASE_VERSION: &str = "0.1.2";
const PREVIOUS_PUBLISHED_VERSION: &str = "0.1.1";
const PUBLISHED_RELEASE_DOC: &str = "docs/release/0.1.2.md";
const CANDIDATE_RELEASE_VERSION: &str = "0.1.3";
const CANDIDATE_RELEASE_DOC: &str = "docs/release/0.1.3.md";

#[test]
fn release_0_1_2_publish_order_matches_internal_dependency_graph() {
    let root = workspace_root();
    let release_doc = fs::read_to_string(root.join(PUBLISHED_RELEASE_DOC)).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read {PUBLISHED_RELEASE_DOC}: {err}"))
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
fn release_0_1_2_record_keeps_completed_publication_evidence() {
    let root = workspace_root();
    let release_doc = fs::read_to_string(root.join(PUBLISHED_RELEASE_DOC)).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read {PUBLISHED_RELEASE_DOC}: {err}"))
    });

    assert!(
        release_doc.contains("# 0.1.2 Release Record"),
        "release record should name the completed patch release"
    );
    assert!(
        release_doc.contains("to `0.1.2`"),
        "release record should document the completed version bump"
    );
    assert!(
        release_doc.contains("--version 0.1.2"),
        "release record should smoke-test the published {PUBLISHED_RELEASE_VERSION} binary"
    );
}

#[test]
fn release_0_1_3_candidate_versions_are_staged_without_publication_claims() {
    let root = workspace_root();
    let release_doc = read_workspace_file(&root, CANDIDATE_RELEASE_DOC);
    let workspace_manifest = read_workspace_file(&root, "Cargo.toml");
    let lockfile = read_workspace_file(&root, "Cargo.lock");
    let package_manifests = workspace_package_manifests(&root);
    let workspace_version = workspace_package_version(&workspace_manifest);

    assert_eq!(
        workspace_version, CANDIDATE_RELEASE_VERSION,
        "{CANDIDATE_RELEASE_VERSION} release candidate should stage the workspace package version"
    );
    assert!(
        release_doc.contains("# 0.1.3 Release Candidate"),
        "0.1.3 note should name itself as a candidate, not a completed release record"
    );
    assert!(
        release_doc.contains("not a publication record"),
        "0.1.3 candidate note should say it is not a publication record"
    );
    assert!(
        release_doc.contains("has not been") && release_doc.contains("tagged or published"),
        "0.1.3 candidate note should deny completed release actions"
    );
    assert!(
        release_doc.contains("workspace package version")
            && release_doc.contains("internal dependency requirements")
            && release_doc.contains("staged at `0.1.3`"),
        "0.1.3 candidate note should document the staged version bump"
    );

    for (package, manifest) in &package_manifests {
        assert!(
            manifest.contains("version.workspace = true"),
            "{package} should inherit the workspace release version"
        );
    }

    let package_names = package_manifests.keys().cloned().collect::<BTreeSet<_>>();
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
            version, workspace_version,
            "{dependency} workspace dependency should require the release-candidate version"
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
            version, workspace_version,
            "{package} lockfile entry should carry the release-candidate version"
        );
    }

    for relative_path in release_install_surfaces() {
        let content = read_workspace_file(&root, relative_path);
        assert!(
            content.contains(&format!(
                "cargo install cargo-allow --version {PUBLISHED_RELEASE_VERSION} --locked"
            )),
            "{relative_path} should keep the published {PUBLISHED_RELEASE_VERSION} install pin until {CANDIDATE_RELEASE_VERSION} is published"
        );
        assert!(
            !content.contains(&format!(
                "cargo install cargo-allow --version {CANDIDATE_RELEASE_VERSION} --locked"
            )),
            "{relative_path} should not advertise unpublished {CANDIDATE_RELEASE_VERSION}"
        );
    }
}

#[test]
fn release_0_1_2_install_examples_use_published_release() {
    let root = workspace_root();
    let release_doc = read_workspace_file(&root, PUBLISHED_RELEASE_DOC);

    assert!(
        release_doc.contains("Public install examples now pin the published `0.1.2` release"),
        "release record should note that public install examples moved to the published release"
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
        "examples/github-actions/cargo-allow-check.yml",
        "examples/github-actions/cargo-allow-diff.yml",
    ]
}

fn read_workspace_file(root: &Path, relative_path: &str) -> String {
    fs::read_to_string(root.join(relative_path))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {relative_path}: {err}")))
}

fn workspace_package_manifests(root: &Path) -> BTreeMap<String, String> {
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
        manifests.insert(package, manifest);
    }
    manifests
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
        if in_workspace_package {
            if let Some(version) = manifest_value(line, "version") {
                return version;
            }
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
