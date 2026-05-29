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
fn scan_files_filters_allowed_paths_and_preserves_order() {
    let files = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("src/lib.rs"),
        PathBuf::from("docs/guide.md"),
        PathBuf::from("tools/check.py"),
    ];

    let findings = scan_files(&files);

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].path, PathBuf::from("docs/guide.md"));
    assert_eq!(findings[0].family.as_deref(), Some("documentation"));
    assert_eq!(findings[1].path, PathBuf::from("tools/check.py"));
    assert_eq!(findings[1].family.as_deref(), Some("python_tool"));
}

#[test]
fn classification_populates_identity_span_fingerprint_and_message() {
    let finding = classify_path(Path::new("tools/Build.SH"))
        .unwrap_or_else(|| std::panic::panic_any("expected shell script finding"));

    assert_eq!(finding.kind, FindingKind::NonRustFile);
    assert_eq!(finding.family.as_deref(), Some("shell_script"));
    assert_eq!(finding.path, PathBuf::from("tools/Build.SH"));
    assert_eq!(finding.span, Some(Span { line: 1, column: 1 }));
    assert_eq!(finding.identity.language, "file");
    assert_eq!(finding.identity.ast_kind, "tracked_file");
    assert_eq!(finding.identity.symbol.as_deref(), Some("tools/Build.SH"));
    assert_eq!(finding.identity.target_fingerprint.as_deref(), Some("sh"));
    assert_eq!(
        finding.message,
        "tracked non-Rust file classified as shell_script"
    );
}

#[test]
fn generated_detection_covers_directory_abbreviation_and_suffixes() {
    for path in [
        "src/gen/client.rs.txt",
        "schemas/gen/api.yaml",
        "src/schema.generated",
    ] {
        assert_classification(path, FindingKind::GeneratedCode, "generated_code");
    }
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
    assert_classification("tools/check.bash", FindingKind::NonRustFile, "shell_script");
    assert_classification("tools/check.ps1", FindingKind::NonRustFile, "shell_script");
    assert_classification("tools/audit.py", FindingKind::NonRustFile, "python_tool");
    assert_classification(
        "tools/report.ts",
        FindingKind::NonRustFile,
        "javascript_tool",
    );
    assert_classification("package.json", FindingKind::NonRustFile, "package_metadata");
    assert_classification(
        "requirements.txt",
        FindingKind::NonRustFile,
        "package_metadata",
    );
    assert_classification(
        ".vscode/extensions.json",
        FindingKind::NonRustFile,
        "editor_extension",
    );
    assert_classification(
        "project.code-workspace",
        FindingKind::NonRustFile,
        "editor_extension",
    );
    assert_classification(".gitignore", FindingKind::NonRustFile, "configuration");
    assert_classification(".editorconfig", FindingKind::NonRustFile, "configuration");
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
fn scan_files_with_options_marks_globbed_files_as_generated() {
    let options = FileScanOptions {
        generated: vec!["schemas/**".to_string()],
    };
    let files = vec![
        PathBuf::from("schemas/api.yaml"),
        PathBuf::from("tools/audit.py"),
    ];

    let findings = scan_files_with_options(&files, &options);

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].kind, FindingKind::GeneratedCode);
    assert_eq!(findings[0].family.as_deref(), Some("generated_code"));
    assert_eq!(findings[1].kind, FindingKind::NonRustFile);
    assert_eq!(findings[1].family.as_deref(), Some("python_tool"));
}

#[test]
fn builtin_cargo_and_license_files_are_not_findings() {
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "rustfmt.toml",
        "clippy.toml",
        "crates/allow-files/Cargo.toml",
        "crates/allow-files/README.md",
        "README.md",
        "LICENSE",
        "LICENSE-MIT",
        "LICENSE-APACHE",
    ] {
        assert!(classify_path(Path::new(path)).is_none(), "{path}");
    }
}

#[test]
fn files_without_extensions_fingerprint_by_lowercase_file_name() {
    let finding = classify_path(Path::new("bin/TOOL"))
        .unwrap_or_else(|| std::panic::panic_any("expected extensionless file finding"));

    assert_eq!(finding.family.as_deref(), Some("unknown_non_rust"));
    assert_eq!(finding.identity.target_fingerprint.as_deref(), Some("tool"));
}

fn assert_classification(path: &str, kind: FindingKind, family: &str) {
    let finding = classify_path(Path::new(path))
        .unwrap_or_else(|| std::panic::panic_any(format!("expected file finding for {path}")));

    assert_eq!(finding.kind, kind);
    assert_eq!(finding.family.as_deref(), Some(family));
}
