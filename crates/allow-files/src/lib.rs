use allow_core::{Finding, FindingKind, Span, StructuralIdentity, glob_matches, normalize_path};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct FileScanOptions {
    pub generated: Vec<String>,
}

pub fn scan_files(files: &[PathBuf]) -> Vec<Finding> {
    scan_files_with_options(files, &FileScanOptions::default())
}

pub fn scan_files_with_options(files: &[PathBuf], options: &FileScanOptions) -> Vec<Finding> {
    files
        .iter()
        .filter_map(|path| classify_path_with_options(path, options))
        .collect()
}

pub fn classify_path(path: &Path) -> Option<Finding> {
    classify_path_with_options(path, &FileScanOptions::default())
}

pub fn classify_path_with_options(path: &Path, options: &FileScanOptions) -> Option<Finding> {
    if is_rust_source(path) || is_builtin_allowed(path) {
        return None;
    }
    let generated = is_generated_path(path, &options.generated);
    let family = file_family(path, generated);
    let mut identity = StructuralIdentity::new("file", "tracked_file");
    identity.symbol = Some(normalize_path(path));
    identity.target_fingerprint = file_fingerprint(path);
    Some(Finding {
        kind: if generated {
            FindingKind::GeneratedCode
        } else {
            FindingKind::NonRustFile
        },
        family: Some(family.clone()),
        path: path.to_path_buf(),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: format!("tracked non-Rust file classified as {family}"),
    })
}

pub fn is_rust_source(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("rs")
}

fn is_builtin_allowed(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    matches!(
        text.as_str(),
        "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml" | "rustfmt.toml" | "clippy.toml"
    ) || text.starts_with("crates/")
        && (text.ends_with("/Cargo.toml") || text.ends_with("/README.md"))
        || text == "README.md"
        || text == "LICENSE"
        || text == "LICENSE-MIT"
        || text == "LICENSE-APACHE"
}

fn is_generated_path(path: &Path, generated_patterns: &[String]) -> bool {
    let text = normalize_path(path);
    let file_name = lower_file_name(path);
    generated_patterns
        .iter()
        .any(|pattern| glob_matches(pattern, path))
        || text.contains("/generated/")
        || text.starts_with("generated/")
        || file_name.contains(".generated.")
        || file_name.ends_with(".generated")
        || text.contains("/gen/")
        || text.starts_with("gen/")
}

fn file_family(path: &Path, generated: bool) -> String {
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

fn file_fingerprint(path: &Path) -> Option<String> {
    lower_extension(path).or_else(|| {
        let file_name = lower_file_name(path);
        (!file_name.is_empty()).then_some(file_name)
    })
}

fn lower_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

fn lower_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default()
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

#[cfg(test)]
mod tests;
