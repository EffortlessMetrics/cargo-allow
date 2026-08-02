use super::*;
use allow_core::{FileFamilyRule, FindingKind, Span};
use std::path::{Path, PathBuf};

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

    assert_eq!(findings.len(), 3);
    assert_eq!(findings[0].path, PathBuf::from("Cargo.toml"));
    assert_eq!(findings[0].family.as_deref(), Some("package_metadata"));
    assert_eq!(findings[1].path, PathBuf::from("docs/guide.md"));
    assert_eq!(findings[1].family.as_deref(), Some("documentation"));
    assert_eq!(findings[2].path, PathBuf::from("tools/check.py"));
    assert_eq!(findings[2].family.as_deref(), Some("python_tool"));
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
fn generated_detection_covers_common_ecosystem_markers() {
    for path in [
        "proto/user.pb.go",
        "proto/user.grpc.pb.go",
        "proto/user.pb.rs",
        "proto/user.pb.dart",
        "proto/user.pb.cc",
        "proto/user.pb.h",
        "python/user_pb2.py",
        "lib/user.g.dart",
        "internal/user_mock.go",
    ] {
        assert_classification(path, FindingKind::GeneratedCode, "generated_code");
    }
}

#[test]
fn generated_detection_does_not_match_loose_marker_words() {
    assert_classification(
        "internal/mock.go",
        FindingKind::NonRustFile,
        "unknown_non_rust",
    );
    assert_classification(
        "internal/feedback.go",
        FindingKind::NonRustFile,
        "unknown_non_rust",
    );
}

#[test]
fn classifies_non_rust_governance_families() {
    assert_classification("Cargo.toml", FindingKind::NonRustFile, "package_metadata");
    assert_classification("Cargo.lock", FindingKind::NonRustFile, "package_metadata");
    assert_classification(
        "crates/allow-files/Cargo.toml",
        FindingKind::NonRustFile,
        "package_metadata",
    );
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
        ..FileScanOptions::default()
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
fn custom_file_family_options_override_built_in_classification() {
    let options = FileScanOptions {
        file_families: vec![FileFamilyRule {
            id: "release-manifest".to_string(),
            family: "release_metadata".to_string(),
            glob: "models/release/*.json".to_string(),
            reason: "Release metadata is governed separately.".to_string(),
        }],
        ..FileScanOptions::default()
    };

    let finding = classify_path_with_options(Path::new("models/release/manifest.json"), &options)
        .unwrap_or_else(|| std::panic::panic_any("expected custom file-family finding"));

    assert_eq!(finding.kind, FindingKind::NonRustFile);
    assert_eq!(finding.family.as_deref(), Some("release_metadata"));
    assert!(finding.message.contains("rule release-manifest"));
}

#[test]
fn ambiguous_custom_file_family_options_remain_visible_in_findings() {
    let options = FileScanOptions {
        file_families: vec![
            FileFamilyRule {
                id: "model".to_string(),
                family: "ml_model".to_string(),
                glob: "models/*.json".to_string(),
                reason: "Models are governed.".to_string(),
            },
            FileFamilyRule {
                id: "artifact".to_string(),
                family: "artifact".to_string(),
                glob: "models/*.json".to_string(),
                reason: "Artifacts are governed.".to_string(),
            },
        ],
        ..FileScanOptions::default()
    };

    let finding = classify_path_with_options(Path::new("models/current.json"), &options)
        .unwrap_or_else(|| std::panic::panic_any("expected ambiguous file-family finding"));

    assert_eq!(finding.family.as_deref(), Some("ambiguous_file_family"));
    assert!(
        finding
            .message
            .contains("conflicting rules artifact, model")
    );
    assert!(finding.message.contains("artifact, ml_model"));
}

#[test]
fn scan_files_with_options_marks_globbed_files_as_generated() {
    let options = FileScanOptions {
        generated: vec!["schemas/**".to_string()],
        ..FileScanOptions::default()
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
fn configured_generated_globs_match_nested_files_and_windows_separators() {
    let options = FileScanOptions {
        generated: vec![r"vendor\**\*.json".to_string()],
        ..FileScanOptions::default()
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
fn builtin_license_readme_and_tool_config_files_are_not_findings() {
    for path in [
        "rust-toolchain.toml",
        "rustfmt.toml",
        "clippy.toml",
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

#[test]
fn scan_files_only_returns_classified_paths_in_input_order() {
    let files = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("tools/check.py"),
        PathBuf::from("README.md"),
        PathBuf::from("assets/logo.bin"),
    ];

    let findings = scan_files(&files);

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].path, PathBuf::from("tools/check.py"));
    assert_eq!(findings[0].family.as_deref(), Some("python_tool"));
    assert_eq!(findings[1].path, PathBuf::from("assets/logo.bin"));
    assert_eq!(findings[1].family.as_deref(), Some("unknown_non_rust"));
}

#[test]
fn classification_populates_span_identity_message_and_fingerprint() {
    let finding = classify_path(Path::new("config/settings.JSON"))
        .unwrap_or_else(|| std::panic::panic_any("expected configuration file finding"));

    assert_eq!(finding.kind, FindingKind::NonRustFile);
    assert_eq!(finding.family.as_deref(), Some("configuration"));
    assert_eq!(finding.span, Some(allow_core::Span { line: 1, column: 1 }));
    assert_eq!(finding.identity.language, "file");
    assert_eq!(finding.identity.ast_kind, "tracked_file");
    assert_eq!(
        finding.identity.symbol.as_deref(),
        Some("config/settings.JSON")
    );
    assert_eq!(finding.identity.target_fingerprint.as_deref(), Some("json"));
    assert_eq!(
        finding.message,
        "tracked non-Rust file classified as configuration"
    );
}

#[test]
fn generated_detection_covers_gen_directories_and_name_suffixes() {
    for path in [
        "src/gen/schema.yaml",
        "gen/schema.yaml",
        "src/types.generated",
        "src/types.generated.yaml",
    ] {
        assert_classification(path, FindingKind::GeneratedCode, "generated_code");
    }
}

#[test]
fn builtin_workspace_readmes_and_tool_configs_are_not_findings() {
    assert!(classify_path(Path::new("crates/allow-files/README.md")).is_none());
    assert!(classify_path(Path::new("rust-toolchain.toml")).is_none());
    assert!(classify_path(Path::new("rustfmt.toml")).is_none());
    assert!(classify_path(Path::new("clippy.toml")).is_none());
}

#[test]
fn extension_and_file_name_matching_are_case_insensitive() {
    assert_classification("DOCS/README.MD", FindingKind::NonRustFile, "documentation");
    assert_classification("TOOLS/CHECK.PS1", FindingKind::NonRustFile, "shell_script");
    assert_classification("Package.JSON", FindingKind::NonRustFile, "package_metadata");
}

#[test]
fn doc_extension_files_under_fixture_dirs_are_test_fixtures() {
    // #1876: files under fixtures/testdata/snapshots are fixture data, not
    // project documentation, even with a doc extension. test_fixture wins over
    // documentation by design.
    for path in [
        "testdata/design.md",
        "fixtures/notes.md",
        "snapshots/output.txt",
        "crates/parser/tests/fixtures/spec.md",
        "tests/fixtures/import/kiro/.kiro/specs/auth-feature/design.md",
    ] {
        assert_classification(path, FindingKind::NonRustFile, "test_fixture");
    }
}

#[test]
fn fixture_family_matches_whole_segments_only() {
    // The fixture match is segment-anchored, so a sibling directory that merely
    // contains the word is NOT a fixture and a `.md` file there is documentation
    // (#1876: the substring check does not over-broaden).
    assert_classification(
        "myfixtures/design.md",
        FindingKind::NonRustFile,
        "documentation",
    );
    assert_classification(
        "src/fixtures_old/design.md",
        FindingKind::NonRustFile,
        "documentation",
    );
    // A real docs tree stays documentation.
    assert_classification("docs/guide.md", FindingKind::NonRustFile, "documentation");
}
