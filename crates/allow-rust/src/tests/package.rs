use std::fs;
use std::path::{Path, PathBuf};

use crate::package::{source_package_contexts, source_package_for_path, source_package_name};
use crate::{SourcePackageContext, scan_rust_files, source_package_contexts_from_sources};

use super::temp_root;

#[test]
fn scan_rust_files_adds_source_package_context_from_manifest() {
    let root = temp_root("source-package");
    let crate_dir = root.join("crates").join("parser");
    fs::create_dir_all(crate_dir.join("src"))
        .unwrap_or_else(|err| panic!("crate dir: {err}"));
    fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"parser\"\nversion = \"0.1.0\"\n",
    )
    .unwrap_or_else(|err| panic!("manifest write: {err}"));
    fs::write(
        crate_dir.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("rust write: {err}"));
    let files = vec![
        PathBuf::from("crates/parser/Cargo.toml"),
        PathBuf::from("crates/parser/src/lib.rs"),
    ];
    assert_eq!(
        source_package_name("[package]\nname = \"parser\"\n"),
        Some("parser".to_string())
    );
    let packages = source_package_contexts(&root, &files)
        .unwrap_or_else(|err| panic!("package contexts: {err}"));
    assert_eq!(
        packages,
        vec![SourcePackageContext {
            root: "crates/parser".to_string(),
            name: "parser".to_string()
        }]
    );
    assert!(source_package_for_path(&files[1], &packages).is_some());

    let scan_result = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| panic!("scan rust files: {err}"));

    let unwrap = scan_result
        .findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name.as_deref(), Some("parser"));
    fs::remove_dir_all(root).unwrap_or_else(|err| panic!("cleanup: {err}"));
}

#[test]
fn source_package_context_prefers_nested_manifest() {
    let packages = source_package_contexts_from_sources([
        (
            PathBuf::from("Cargo.toml"),
            "[package]\nname = \"root\"\n".to_string(),
        ),
        (
            PathBuf::from("crates/parser/Cargo.toml"),
            "[package]\nname = \"parser\"\n".to_string(),
        ),
    ]);

    let package = source_package_for_path(Path::new("crates/parser/src/lib.rs"), &packages)
        .unwrap_or_else(|| std::panic::panic_any("expected nested package context"));

    assert_eq!(package.name, "parser");
}

#[test]
fn source_package_context_does_not_match_sibling_prefixes() {
    let packages = source_package_contexts_from_sources([(
        PathBuf::from("crates/parser/Cargo.toml"),
        "[package]\nname = \"parser\"\n".to_string(),
    )]);

    assert!(
        source_package_for_path(Path::new("crates/parser-extra/src/lib.rs"), &packages).is_none()
    );
}

#[test]
fn scan_rust_files_ignores_workspace_manifest_without_package_name() {
    let root = temp_root("workspace-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("src dir: {err}"));
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .unwrap_or_else(|err| panic!("manifest write: {err}"));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("rust write: {err}"));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let scan_result = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| panic!("scan rust files: {err}"));

    let unwrap = scan_result
        .findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| panic!("cleanup: {err}"));
}

#[test]
fn scan_rust_files_preserves_input_order_after_parallel_scan() {
    let root = temp_root("parallel-order");
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create root: {err}"));
    fs::write(
        root.join("first.rs"),
        "fn first(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write first: {err}"));
    fs::write(
        root.join("second.rs"),
        "fn second(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write second: {err}"));

    let result = scan_rust_files(
        &root,
        &[PathBuf::from("first.rs"), PathBuf::from("second.rs")],
    )
    .unwrap_or_else(|err| panic!("scan files: {err}"));

    let paths = result
        .findings
        .iter()
        .map(|finding| finding.path.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![PathBuf::from("first.rs"), PathBuf::from("second.rs")]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn scan_rust_files_ignores_invalid_manifest_source_text() {
    let root = temp_root("invalid-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("src dir: {err}"));
    fs::write(root.join("Cargo.toml"), "[package\nname = \"broken\"\n")
        .unwrap_or_else(|err| panic!("manifest write: {err}"));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("rust write: {err}"));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let scan_result = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| panic!("scan rust files: {err}"));

    let unwrap = scan_result
        .findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| panic!("cleanup: {err}"));
}

#[test]
fn scan_rust_files_ignores_non_utf8_manifest_source_text() {
    let root = temp_root("non-utf8-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("src dir: {err}"));
    fs::write(
        root.join("Cargo.toml"),
        b"[package]\nname = \"broken\"\n\xFF",
    )
    .unwrap_or_else(|err| panic!("manifest write: {err}"));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("rust write: {err}"));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let scan_result = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| panic!("scan rust files: {err}"));

    let unwrap = scan_result
        .findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| panic!("cleanup: {err}"));
}

#[test]
fn scan_rust_files_ignores_unreadable_manifest_context() {
    let root = temp_root("unreadable-manifest");
    fs::create_dir_all(root.join("Cargo.toml"))
        .unwrap_or_else(|err| panic!("manifest dir: {err}"));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("src dir: {err}"));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("rust write: {err}"));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let scan_result = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| panic!("scan rust files: {err}"));

    let unwrap = scan_result
        .findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| panic!("cleanup: {err}"));
}
