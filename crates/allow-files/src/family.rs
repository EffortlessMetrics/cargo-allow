use crate::path_rules::{lower_extension, lower_file_name};
use allow_core::normalize_path;
use std::path::Path;

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
    if text.starts_with("docs/")
        || matches!(
            extension.as_deref(),
            Some("md" | "mdx" | "rst" | "adoc" | "txt")
        )
    {
        return "documentation".to_string();
    }
    match extension.as_deref().unwrap_or("") {
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" => "shell_script".to_string(),
        "py" => "python_tool".to_string(),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => "javascript_tool".to_string(),
        "yml" | "yaml" | "json" | "toml" | "xml" | "ini" | "cfg" | "conf" | "env"
        | "properties" => "configuration".to_string(),
        _ if is_configuration_file(&file_name) => "configuration".to_string(),
        _ => "unknown_non_rust".to_string(),
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
        "package.json"
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
