use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{scan_rust_files, scan_rust_source_with_completeness};

fn temp_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-rust-parse-error-{label}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create temp root: {err}"));
    root
}

#[test]
fn scan_rust_source_reports_tree_sitter_parse_errors() {
    let source = "fn broken( {\n    let _ = Some(1).unwrap();\n}\n";
    let scan = scan_rust_source_with_completeness("src/broken.rs", source);
    assert!(
        scan.has_parse_error,
        "syntax-error source must not fail open silently"
    );
}

#[test]
fn scan_rust_files_counts_parse_errors_without_aborting() {
    let root = temp_root("workspace-parse-error");
    let src = root.join("src");
    fs::create_dir_all(&src)
        .unwrap_or_else(|err| panic!("mkdir src: {err}"));
    fs::write(src.join("ok.rs"), "fn ok() { let _ = Some(1).unwrap(); }\n")
        .unwrap_or_else(|err| panic!("write ok.rs: {err}"));
    fs::write(
        src.join("broken.rs"),
        "fn broken( {\n    let _ = Some(1).unwrap();\n}\n",
    )
    .unwrap_or_else(|err| panic!("write broken.rs: {err}"));

    let mixed = scan_rust_files(
        &root,
        &[PathBuf::from("src/ok.rs"), PathBuf::from("src/broken.rs")],
    )
    .unwrap_or_else(|err| panic!("scan mixed: {err}"));

    assert!(
        mixed.findings.iter().any(|f| f.path.ends_with("ok.rs")),
        "parse-error sibling must not abort the scan"
    );
    assert_eq!(
        mixed.files_with_parse_errors, 1,
        "broken file must be counted as a partial parse"
    );
    assert!(
        mixed.has_parse_errors(),
        "scan result must expose parse-error completeness"
    );
    assert_eq!(mixed.files_skipped, 0, "readable files must not be skipped");

    let _ = fs::remove_dir_all(&root);
}
