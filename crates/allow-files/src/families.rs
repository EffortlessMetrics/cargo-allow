use allow_core::{FileFamilyRule, glob_matches, normalize_path};
use std::collections::BTreeSet;
use std::path::Path;

use crate::path_rules::{lower_extension, lower_file_name};

#[derive(Debug, Clone, PartialEq, Eq)]
/// The deterministic result of applying built-in and repository-defined rules
/// to one source-tree path.
pub enum FileFamilyClassification {
    Generated,
    BuiltIn(String),
    Custom {
        rule_id: String,
        family: String,
    },
    Ambiguous {
        rule_ids: Vec<String>,
        families: Vec<String>,
    },
}

impl FileFamilyClassification {
    pub fn family(&self) -> &str {
        match self {
            Self::Generated => "generated_code",
            Self::BuiltIn(family) | Self::Custom { family, .. } => family,
            Self::Ambiguous { .. } => "ambiguous_file_family",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RuleSpecificity {
    exact: bool,
    literal_segments: usize,
    literal_chars: usize,
    wildcard_segments: usize,
    wildcard_chars: usize,
}

pub(crate) fn file_family(
    path: &Path,
    generated: bool,
    rules: &[FileFamilyRule],
) -> FileFamilyClassification {
    let text = normalize_path(path);
    let extension = lower_extension(path);
    let file_name = lower_file_name(path);
    if generated {
        return FileFamilyClassification::Generated;
    }

    let mut matches = rules
        .iter()
        .filter(|rule| glob_matches(&rule.glob, path))
        .map(|rule| (rule, rule_specificity(&rule.glob)))
        .collect::<Vec<_>>();
    if let Some(strongest) = matches.iter().map(|(_, specificity)| *specificity).max() {
        matches.retain(|(_, specificity)| *specificity == strongest);
        let families = matches
            .iter()
            .map(|(rule, _)| rule.family.clone())
            .collect::<BTreeSet<_>>();
        if families.len() > 1 {
            let rule_ids = matches
                .iter()
                .map(|(rule, _)| rule.id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            return FileFamilyClassification::Ambiguous {
                rule_ids,
                families: families.into_iter().collect(),
            };
        }
        if let Some((rule, _)) = matches.iter().min_by_key(|(rule, _)| rule.id.as_str()) {
            return FileFamilyClassification::Custom {
                rule_id: rule.id.clone(),
                family: rule.family.clone(),
            };
        }
    }

    if text.starts_with(".github/workflows/") {
        return FileFamilyClassification::BuiltIn("ci_declarative".to_string());
    }
    if is_editor_extension(&text, &file_name) {
        return FileFamilyClassification::BuiltIn("editor_extension".to_string());
    }
    if is_package_metadata(&file_name) {
        return FileFamilyClassification::BuiltIn("package_metadata".to_string());
    }
    if is_test_fixture(&text) {
        return FileFamilyClassification::BuiltIn("test_fixture".to_string());
    }
    if is_release_script(&text, &file_name) {
        return FileFamilyClassification::BuiltIn("release_script".to_string());
    }
    if is_documentation(&text, extension.as_deref()) {
        return FileFamilyClassification::BuiltIn("documentation".to_string());
    }
    FileFamilyClassification::BuiltIn(
        classify_by_extension(extension.as_deref(), &file_name).to_string(),
    )
}

fn rule_specificity(glob: &str) -> RuleSpecificity {
    let normalized = glob.replace('\\', "/");
    let segments = normalized.split('/').filter(|segment| !segment.is_empty());
    let mut literal_segments = 0;
    let mut literal_chars = 0;
    let mut wildcard_segments = 0;
    let mut wildcard_chars = 0;
    for segment in segments {
        let wildcard_count = segment.chars().filter(|ch| matches!(ch, '*' | '?')).count();
        if wildcard_count == 0 {
            literal_segments += 1;
        } else {
            wildcard_segments += 1;
            wildcard_chars += wildcard_count;
        }
        literal_chars += segment.chars().count().saturating_sub(wildcard_count);
    }
    RuleSpecificity {
        exact: wildcard_chars == 0,
        literal_segments,
        literal_chars,
        wildcard_segments,
        wildcard_chars,
    }
}

fn is_documentation(path: &str, extension: Option<&str>) -> bool {
    path.starts_with("docs/") || matches!(extension, Some("md" | "mdx" | "rst" | "adoc" | "txt"))
}

fn classify_by_extension(extension: Option<&str>, file_name: &str) -> &'static str {
    match extension.unwrap_or("") {
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" => "shell_script",
        "py" => "python_tool",
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => "javascript_tool",
        "yml" | "yaml" | "json" | "toml" | "xml" | "ini" | "cfg" | "conf" | "env"
        | "properties" => "configuration",
        _ if is_configuration_file(file_name) => "configuration",
        _ => "unknown_non_rust",
    }
}

fn is_editor_extension(path: &str, file_name: &str) -> bool {
    path.starts_with(".vscode/")
        || path.starts_with(".idea/")
        || file_name.ends_with(".code-workspace")
}

fn is_package_metadata(file_name: &str) -> bool {
    matches!(
        file_name,
        "cargo.toml"
            | "cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lockb"
            | "npm-shrinkwrap.json"
            | "deno.json"
            | "deno.lock"
            | "pyproject.toml"
            | "requirements.txt"
            // Modern lockfiles (#1878)
            | "uv.lock"
            | "poetry.lock"
            | "go.mod"
            | "go.sum"
            | "gemfile.lock"
            | "composer.lock"
            | "mix.lock"
            | "packages.lock.json"
            | "flake.lock"
            | "cargo-vet.toml"
    )
}

/// Classify a file as a test fixture when it sits under a `fixtures/`,
/// `testdata/`, or `snapshots/` directory.
///
/// The match is on whole path segments, not a bare substring: the leading and
/// trailing slashes in `/fixtures/` (and the `starts_with` checks) mean a
/// sibling like `myfixtures/x` or `fixtures_old/x` is not matched, so the
/// family cannot over-broaden to files that merely contain the word (#1876).
///
/// This deliberately runs before [`is_documentation`] in [`file_family`]:
/// files under these directories are fixture *data* (inputs to tests), not
/// project documentation, even when they carry a `.md`/`.txt` extension — e.g.
/// `tests/fixtures/import/kiro/.kiro/specs/auth-feature/design.md` is a fixture,
/// not a doc. Keeping `test_fixture` ahead of `documentation` classifies such
/// fixture inputs by where they live rather than by their extension.
fn is_test_fixture(path: &str) -> bool {
    path.starts_with("fixtures/")
        || path.starts_with("testdata/")
        || path.starts_with("snapshots/")
        || path.contains("/fixtures/")
        || path.contains("/testdata/")
        || path.contains("/snapshots/")
}

fn is_release_script(path: &str, file_name: &str) -> bool {
    path.starts_with("scripts/")
        && (file_name.contains("release")
            || file_name.contains("publish")
            || file_name.contains("deploy")
            || file_name.contains("package"))
}

fn is_configuration_file(file_name: &str) -> bool {
    file_name.starts_with('.')
        && matches!(
            file_name,
            ".gitignore"
                | ".gitattributes"
                | ".dockerignore"
                | ".editorconfig"
                | ".prettierrc"
                | ".eslintrc"
                | ".npmrc"
                | ".env"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_family_applies_classifier_precedence() {
        assert_eq!(
            file_family(Path::new(".github/workflows/ci.yml"), true, &[]),
            FileFamilyClassification::Generated
        );
        assert_eq!(
            file_family(Path::new(".github/workflows/ci.yml"), false, &[]),
            FileFamilyClassification::BuiltIn("ci_declarative".to_string())
        );
        assert_eq!(
            file_family(Path::new(".vscode/extensions.json"), false, &[]),
            FileFamilyClassification::BuiltIn("editor_extension".to_string())
        );
        assert_eq!(
            file_family(Path::new("fixtures/package.json"), false, &[]),
            FileFamilyClassification::BuiltIn("package_metadata".to_string())
        );
        assert_eq!(
            file_family(Path::new("crates/parser/fixtures/input.txt"), false, &[]),
            FileFamilyClassification::BuiltIn("test_fixture".to_string())
        );
        assert_eq!(
            file_family(Path::new("scripts/release.sh"), false, &[]),
            FileFamilyClassification::BuiltIn("release_script".to_string())
        );
        assert_eq!(
            file_family(Path::new("docs/design.yaml"), false, &[]),
            FileFamilyClassification::BuiltIn("documentation".to_string())
        );
        assert_eq!(
            file_family(Path::new("tools/audit.py"), false, &[]),
            FileFamilyClassification::BuiltIn("python_tool".to_string())
        );
        assert_eq!(
            file_family(Path::new("assets/logo.bin"), false, &[]),
            FileFamilyClassification::BuiltIn("unknown_non_rust".to_string())
        );
    }

    #[test]
    fn custom_file_family_uses_specificity_not_rule_order() {
        let broad = FileFamilyRule {
            id: "model-files".to_string(),
            family: "ml_model".to_string(),
            glob: "models/**/*.onnx".to_string(),
            reason: "Model files are governed.".to_string(),
        };
        let exact = FileFamilyRule {
            id: "release-model".to_string(),
            family: "release_model".to_string(),
            glob: "models/release/model.onnx".to_string(),
            reason: "Release model is separately governed.".to_string(),
        };
        let first = file_family(
            Path::new("models/release/model.onnx"),
            false,
            &[broad.clone(), exact.clone()],
        );
        let reversed = file_family(
            Path::new("models/release/model.onnx"),
            false,
            &[exact, broad],
        );
        assert_eq!(first, reversed);
        assert_eq!(
            first,
            FileFamilyClassification::Custom {
                rule_id: "release-model".to_string(),
                family: "release_model".to_string(),
            }
        );
    }

    #[test]
    fn equal_strongest_custom_families_are_explicitly_ambiguous() {
        let rules = vec![
            FileFamilyRule {
                id: "model".to_string(),
                family: "ml_model".to_string(),
                glob: "models/*.onnx".to_string(),
                reason: "Model files are governed.".to_string(),
            },
            FileFamilyRule {
                id: "artifact".to_string(),
                family: "artifact".to_string(),
                glob: "models/*.onnx".to_string(),
                reason: "Artifacts are governed.".to_string(),
            },
        ];
        assert_eq!(
            file_family(Path::new("models/current.onnx"), false, &rules),
            FileFamilyClassification::Ambiguous {
                rule_ids: vec!["artifact".to_string(), "model".to_string()],
                families: vec!["artifact".to_string(), "ml_model".to_string()],
            }
        );
    }

    #[test]
    fn documentation_detection_accepts_docs_paths_and_doc_extensions() {
        for (path, extension) in [
            ("docs/design.yaml", Some("yaml")),
            ("guide.md", Some("md")),
            ("guide.mdx", Some("mdx")),
            ("guide.rst", Some("rst")),
            ("guide.adoc", Some("adoc")),
            ("guide.txt", Some("txt")),
        ] {
            assert!(is_documentation(path, extension), "{path}");
        }

        assert!(!is_documentation("src/lib.rs", Some("rs")));
        assert!(!is_documentation("guide", None));
    }

    #[test]
    fn extension_classifier_covers_each_family_arm() {
        for extension in ["sh", "bash", "zsh", "fish", "ps1", "bat", "cmd"] {
            assert_eq!(
                classify_by_extension(Some(extension), "script"),
                "shell_script",
                "{extension}"
            );
        }
        assert_eq!(classify_by_extension(Some("py"), "tool.py"), "python_tool");
        for extension in ["js", "jsx", "ts", "tsx", "mjs", "cjs"] {
            assert_eq!(
                classify_by_extension(Some(extension), "tool"),
                "javascript_tool",
                "{extension}"
            );
        }
        for extension in [
            "yml",
            "yaml",
            "json",
            "toml",
            "xml",
            "ini",
            "cfg",
            "conf",
            "env",
            "properties",
        ] {
            assert_eq!(
                classify_by_extension(Some(extension), "config"),
                "configuration",
                "{extension}"
            );
        }
        assert_eq!(classify_by_extension(None, ".gitignore"), "configuration");
        assert_eq!(
            classify_by_extension(Some("bin"), "logo.bin"),
            "unknown_non_rust"
        );
    }

    #[test]
    fn editor_extension_detection_checks_directory_and_workspace_suffixes() {
        assert!(is_editor_extension(
            ".vscode/extensions.json",
            "extensions.json"
        ));
        assert!(is_editor_extension(".idea/workspace.xml", "workspace.xml"));
        assert!(is_editor_extension(
            "project.code-workspace",
            "project.code-workspace"
        ));
        assert!(!is_editor_extension(
            "config/workspace.xml",
            "workspace.xml"
        ));
    }

    #[test]
    fn package_metadata_detection_covers_known_manifest_and_lock_files() {
        for file_name in [
            "cargo.toml",
            "cargo.lock",
            "package.json",
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lockb",
            "npm-shrinkwrap.json",
            "deno.json",
            "deno.lock",
            "pyproject.toml",
            "requirements.txt",
        ] {
            assert!(is_package_metadata(file_name), "{file_name}");
        }

        assert!(!is_package_metadata("requirements-dev.txt"));
        assert!(!is_package_metadata("Cargo.toml"));
    }

    #[test]
    fn fixture_detection_covers_root_and_nested_fixture_directories() {
        for path in [
            "fixtures/input.txt",
            "testdata/input.txt",
            "snapshots/output.snap",
            "crates/parser/fixtures/input.txt",
            "crates/parser/testdata/input.txt",
            "crates/parser/snapshots/output.snap",
        ] {
            assert!(is_test_fixture(path), "{path}");
        }

        assert!(!is_test_fixture("crates/parser/tests/input.txt"));
    }

    #[test]
    fn release_script_detection_requires_scripts_path_and_release_word() {
        for file_name in ["release.sh", "publish.ps1", "deploy.cmd", "package.bat"] {
            assert!(
                is_release_script("scripts/release.sh", file_name),
                "{file_name}"
            );
        }

        assert!(!is_release_script("tools/release.sh", "release.sh"));
        assert!(!is_release_script("scripts/check.sh", "check.sh"));
    }

    #[test]
    fn configuration_file_detection_requires_known_dotfile_name() {
        for file_name in [
            ".gitignore",
            ".gitattributes",
            ".dockerignore",
            ".editorconfig",
            ".prettierrc",
            ".eslintrc",
            ".npmrc",
            ".env",
        ] {
            assert!(is_configuration_file(file_name), "{file_name}");
        }

        assert!(!is_configuration_file("gitignore"));
        assert!(!is_configuration_file(".unknownrc"));
    }
}
