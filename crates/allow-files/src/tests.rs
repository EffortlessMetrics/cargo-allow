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
fn builtin_cargo_and_license_files_are_not_findings() {
    assert!(classify_path(Path::new("Cargo.toml")).is_none());
    assert!(classify_path(Path::new("Cargo.lock")).is_none());
    assert!(classify_path(Path::new("crates/allow-files/Cargo.toml")).is_none());
    assert!(classify_path(Path::new("README.md")).is_none());
    assert!(classify_path(Path::new("LICENSE-MIT")).is_none());
}

fn assert_classification(path: &str, kind: FindingKind, family: &str) {
    let finding = classify_path(Path::new(path))
        .unwrap_or_else(|| std::panic::panic_any(format!("expected file finding for {path}")));

    assert_eq!(finding.kind, kind);
    assert_eq!(finding.family.as_deref(), Some(family));
}

#[test]
fn scan_files_with_options_returns_only_classified_paths() {
    let options = FileScanOptions {
        generated: vec!["schemas/**".to_string()],
    };
    let files = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("Cargo.lock"),
        PathBuf::from("schemas/api.yaml"),
        PathBuf::from("tools/report.ts"),
    ];

    let findings = scan_files_with_options(&files, &options);

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].path, PathBuf::from("schemas/api.yaml"));
    assert_eq!(findings[0].kind, FindingKind::GeneratedCode);
    assert_eq!(findings[0].family.as_deref(), Some("generated_code"));
    assert_eq!(findings[1].path, PathBuf::from("tools/report.ts"));
    assert_eq!(findings[1].kind, FindingKind::NonRustFile);
    assert_eq!(findings[1].family.as_deref(), Some("javascript_tool"));
}

#[test]
fn classification_populates_stable_identity_and_default_span() {
    let finding = classify_path(Path::new(r"docs\Guide.MD"))
        .unwrap_or_else(|| std::panic::panic_any("expected documentation finding"));

    assert_eq!(finding.span, Some(Span { line: 1, column: 1 }));
    assert_eq!(finding.identity.language, "file");
    assert_eq!(finding.identity.ast_kind, "tracked_file");
    assert_eq!(finding.identity.symbol.as_deref(), Some("docs/Guide.MD"));
    assert_eq!(finding.identity.target_fingerprint.as_deref(), Some("md"));
    assert!(
        finding
            .message
            .contains("tracked non-Rust file classified as documentation")
    );
}

#[test]
fn extensionless_non_rust_files_use_file_name_fingerprint() {
    let finding = classify_path(Path::new("scripts/deploy"))
        .unwrap_or_else(|| std::panic::panic_any("expected extensionless script finding"));

    assert_eq!(finding.family.as_deref(), Some("release_script"));
    assert_eq!(
        finding.identity.target_fingerprint.as_deref(),
        Some("deploy")
    );
}

#[test]
fn generated_detection_covers_gen_directories_and_suffixes() {
    for path in [
        "gen/bindings.rs.txt",
        "src/gen/bindings.json",
        "src/schema.generated",
        "src/schema.generated.yaml",
    ] {
        let finding = classify_path(Path::new(path)).unwrap_or_else(|| {
            std::panic::panic_any(format!("expected generated finding for {path}"))
        });
        assert_eq!(finding.kind, FindingKind::GeneratedCode, "{path}");
        assert_eq!(finding.family.as_deref(), Some("generated_code"), "{path}");
    }
}
