use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn release_0_1_1_publish_order_matches_internal_dependency_graph() {
    let root = workspace_root();
    let release_doc = fs::read_to_string(root.join("docs/release/0.1.1.md"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read 0.1.1 release doc: {err}")));
    let publish_order = parse_publish_order(&release_doc);
    let package_manifests = workspace_package_manifests(&root);
    let package_names = package_manifests.keys().cloned().collect::<BTreeSet<_>>();

    assert_eq!(
        publish_order.iter().cloned().collect::<BTreeSet<_>>(),
        package_names,
        "0.1.1 publish order should include every workspace package exactly once"
    );
    assert_eq!(
        publish_order.len(),
        package_names.len(),
        "0.1.1 publish order should not contain duplicate packages"
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
fn release_0_1_1_version_handoff_matches_workspace_versions() {
    let root = workspace_root();
    let release_doc = fs::read_to_string(root.join("docs/release/0.1.1.md"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read 0.1.1 release doc: {err}")));
    let workspace_manifest = fs::read_to_string(root.join("Cargo.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read workspace manifest: {err}")));
    let lockfile = fs::read_to_string(root.join("Cargo.lock"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read lockfile: {err}")));
    let package_manifests = workspace_package_manifests(&root);
    let workspace_version = workspace_package_version(&workspace_manifest);

    assert_eq!(
        workspace_version, "0.1.1",
        "0.1.1 release prep should keep the workspace package version at 0.1.1"
    );
    assert!(
        release_doc.contains("# 0.1.1 Release Record"),
        "release handoff should name the target patch release"
    );
    assert!(
        release_doc.contains("to `0.1.1`"),
        "release handoff should document the intended version bump"
    );
    assert!(
        release_doc.contains("--version 0.1.1"),
        "release handoff should smoke-test the published 0.1.1 binary"
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
            "{dependency} workspace dependency should require the release version"
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
            "{package} lockfile entry should carry the release version"
        );
    }
}

#[test]
fn release_0_1_1_install_examples_use_published_version() {
    let root = workspace_root();
    let release_doc = read_workspace_file(&root, "docs/release/0.1.1.md");

    assert!(
        release_doc.contains("README, CI docs, and GitHub Actions examples now install"),
        "release record should note that install examples were updated after crates.io visibility"
    );

    for relative_path in release_install_surfaces() {
        let content = read_workspace_file(&root, relative_path);
        assert!(
            content.contains("cargo install cargo-allow --version 0.1.1 --locked"),
            "{relative_path} should install the published cargo-allow 0.1.1 release"
        );
        assert!(
            !content.contains("cargo install cargo-allow --version 0.1.0 --locked"),
            "{relative_path} should not keep advertising the previous cargo-allow release"
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
        std::panic::panic_any("0.1.1 release doc should contain Publish Order section");
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
