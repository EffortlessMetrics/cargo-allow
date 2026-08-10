from pathlib import Path

path = Path("crates/cargo-allow/src/release_prep_tests.rs")
text = path.read_text(encoding="utf-8")

test_start = text.index("#[test]\nfn release_packages_use_crate_local_readmes()")
test_end = text.index("\n#[test]\nfn release_manifest_validate_produces_complete", test_start)
new_test = '''#[test]
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
'''
text = text[:test_start] + new_test + text[test_end:]

helper_start = text.index("fn all_workspace_package_manifests(root: &Path)")
helper_end = text.index("\nfn is_publishable_workspace_package", helper_start)
new_helper = '''fn all_workspace_package_manifests(root: &Path) -> BTreeMap<String, String> {
    all_workspace_package_manifest_entries(root)
        .into_iter()
        .map(|(package, (_, manifest))| (package, manifest))
        .collect()
}

fn all_workspace_package_manifest_entries(
    root: &Path,
) -> BTreeMap<String, (PathBuf, String)> {
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
'''
text = text[:helper_start] + new_helper + text[helper_end:]
path.write_text(text, encoding="utf-8")
