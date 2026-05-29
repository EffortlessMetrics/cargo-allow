use super::*;

#[test]
fn rust_is_not_non_rust() {
    assert!(classify_path(Path::new("src/lib.rs")).is_none());
}

#[test]
fn workflow_is_non_rust() {
    assert_classification(
        ".github/workflows/ci.yml",
        FindingKind::NonRustFile,
        "ci_declarative",
    );
}

#[test]
fn classifies_non_rust_governance_families() {
    assert_classification("docs/design.md", FindingKind::NonRustFile, "documentation");
    assert_classification(
        "generated/schema.json",
        FindingKind::GeneratedCode,
        "generated_code",
    );
    assert_classification(
        "src/schema.generated.json",
        FindingKind::GeneratedCode,
        "generated_code",
    );
    assert_classification(
        "crates/parser/fixtures/input.txt",
        FindingKind::NonRustFile,
        "test_fixture",
    );
    assert_classification(
        "scripts/release.sh",
        FindingKind::NonRustFile,
        "release_script",
    );
    assert_classification("tools/check.sh", FindingKind::NonRustFile, "shell_script");
    assert_classification("tools/audit.py", FindingKind::NonRustFile, "python_tool");
    assert_classification(
        "tools/report.ts",
        FindingKind::NonRustFile,
        "javascript_tool",
    );
    assert_classification("package.json", FindingKind::NonRustFile, "package_metadata");
    assert_classification(
        ".vscode/extensions.json",
        FindingKind::NonRustFile,
        "editor_extension",
    );
    assert_classification(".gitignore", FindingKind::NonRustFile, "configuration");
    assert_classification(
        "config/settings.yaml",
        FindingKind::NonRustFile,
        "configuration",
    );
    assert_classification(
        "assets/logo.bin",
        FindingKind::NonRustFile,
        "unknown_non_rust",
    );
}

#[test]
fn generated_globs_override_file_family() {
    let options = FileScanOptions {
        generated: vec!["schemas/**".to_string()],
    };
    let generated = classify_path_with_options(Path::new("schemas/api.yaml"), &options)
        .unwrap_or_else(|| {
            std::panic::panic_any("expected generated file finding from configured glob")
        });
    let normal = classify_path(Path::new("schemas/api.yaml"))
        .unwrap_or_else(|| std::panic::panic_any("expected normal file finding"));

    assert_eq!(generated.kind, FindingKind::GeneratedCode);
    assert_eq!(generated.family.as_deref(), Some("generated_code"));
    assert_eq!(normal.kind, FindingKind::NonRustFile);
    assert_eq!(normal.family.as_deref(), Some("configuration"));
}

#[test]
fn configured_generated_globs_match_nested_files_and_windows_separators() {
    let options = FileScanOptions {
        generated: vec![r"vendor\**\*.json".to_string()],
    };

    let finding = classify_path_with_options(Path::new(r"vendor\api\schema.json"), &options)
        .unwrap_or_else(|| std::panic::panic_any("expected generated file finding"));

    assert_eq!(finding.kind, FindingKind::GeneratedCode);
    assert_eq!(finding.family.as_deref(), Some("generated_code"));
    assert_eq!(
        finding.identity.symbol.as_deref(),
        Some("vendor/api/schema.json")
    );
    assert_eq!(finding.identity.target_fingerprint.as_deref(), Some("json"));
}

#[test]
fn built_in_generated_directory_markers_are_detected_at_root_and_nested() {
    for path in ["gen/bindings.rs.snap", "src/generated/bindings.txt"] {
        let finding = classify_path(Path::new(path))
            .unwrap_or_else(|| std::panic::panic_any(format!("expected finding for {path}")));

        assert_eq!(finding.kind, FindingKind::GeneratedCode);
        assert_eq!(finding.family.as_deref(), Some("generated_code"));
    }
}

#[test]
fn hidden_configuration_and_file_name_fingerprints_are_recorded() {
    let finding = classify_path(Path::new("config/.env"))
        .unwrap_or_else(|| std::panic::panic_any("expected .env classification"));

    assert_eq!(finding.kind, FindingKind::NonRustFile);
    assert_eq!(finding.family.as_deref(), Some("configuration"));
    assert_eq!(finding.identity.symbol.as_deref(), Some("config/.env"));
    assert_eq!(finding.identity.target_fingerprint.as_deref(), Some(".env"));
    assert_eq!(finding.span, Some(Span { line: 1, column: 1 }));
}

#[test]
fn scan_files_filters_allowed_inputs_and_preserves_input_order() {
    let files = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("README.md"),
        PathBuf::from("tools/check.sh"),
        PathBuf::from("docs/design.md"),
    ];

    let findings = scan_files(&files);

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].path, PathBuf::from("tools/check.sh"));
    assert_eq!(findings[0].family.as_deref(), Some("shell_script"));
    assert_eq!(findings[1].path, PathBuf::from("docs/design.md"));
    assert_eq!(findings[1].family.as_deref(), Some("documentation"));
}

#[test]
fn builtin_cargo_and_license_files_are_not_findings() {
    assert!(classify_path(Path::new("Cargo.toml")).is_none());
    assert!(classify_path(Path::new("Cargo.lock")).is_none());
    assert!(classify_path(Path::new("crates/allow-files/Cargo.toml")).is_none());
    assert!(classify_path(Path::new("crates/allow-files/README.md")).is_none());
    assert!(classify_path(Path::new("README.md")).is_none());
    assert!(classify_path(Path::new("LICENSE-MIT")).is_none());
}

fn assert_classification(path: &str, kind: FindingKind, family: &str) {
    let finding = classify_path(Path::new(path))
        .unwrap_or_else(|| std::panic::panic_any(format!("expected file finding for {path}")));

    assert_eq!(finding.kind, kind);
    assert_eq!(finding.family.as_deref(), Some(family));
}
