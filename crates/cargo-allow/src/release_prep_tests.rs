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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| std::panic::panic_any("cargo-allow manifest should be under crates/"))
        .to_path_buf()
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
            if package_names.contains(name) && value.contains("workspace = true") {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
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
