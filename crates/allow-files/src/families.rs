use allow_core::normalize_path;
use std::path::Path;

use crate::path_rules::{lower_extension, lower_file_name};

pub(crate) fn file_family(path: &Path, generated: bool) -> String {
    let text = normalize_path(path);
    let extension = lower_extension(path);
    let file_name = lower_file_name(path);
    if generated {
        return "generated_code".to_string();
    }
    if text.starts_with(".github/workflows/") {
        return "ci_declarative".to_string();
    }
    if is_editor_extension(&text, &file_name) {
        return "editor_extension".to_string();
    }
    if is_package_metadata(&file_name) {
        return "package_metadata".to_string();
    }
    if is_test_fixture(&text) {
        return "test_fixture".to_string();
    }
    if is_release_script(&text, &file_name) {
        return "release_script".to_string();
    }
    if is_documentation(&text, extension.as_deref()) {
        return "documentation".to_string();
    }
    classify_by_extension(extension.as_deref(), &file_name).to_string()
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
    )
}

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
            file_family(Path::new(".github/workflows/ci.yml"), true),
            "generated_code"
        );
        assert_eq!(
            file_family(Path::new(".github/workflows/ci.yml"), false),
            "ci_declarative"
        );
        assert_eq!(
            file_family(Path::new(".vscode/extensions.json"), false),
            "editor_extension"
        );
        assert_eq!(
            file_family(Path::new("fixtures/package.json"), false),
            "package_metadata"
        );
        assert_eq!(
            file_family(Path::new("crates/parser/fixtures/input.txt"), false),
            "test_fixture"
        );
        assert_eq!(
            file_family(Path::new("scripts/release.sh"), false),
            "release_script"
        );
        assert_eq!(
            file_family(Path::new("docs/design.yaml"), false),
            "documentation"
        );
        assert_eq!(
            file_family(Path::new("tools/audit.py"), false),
            "python_tool"
        );
        assert_eq!(
            file_family(Path::new("assets/logo.bin"), false),
            "unknown_non_rust"
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
