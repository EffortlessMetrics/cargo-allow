use allow_core::{Finding, FindingKind, Span, StructuralIdentity};
use std::path::{Path, PathBuf};

pub fn scan_files(files: &[PathBuf]) -> Vec<Finding> {
    files
        .iter()
        .filter_map(|path| classify_path(path))
        .collect()
}

pub fn classify_path(path: &Path) -> Option<Finding> {
    if is_rust_source(path) || is_builtin_allowed(path) {
        return None;
    }
    let family = file_family(path);
    let mut identity = StructuralIdentity::new("file", "tracked_file");
    identity.symbol = Some(path.to_string_lossy().replace('\\', "/"));
    identity.target_fingerprint = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    Some(Finding {
        kind: if is_generated_path(path) {
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

fn is_generated_path(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    text.contains("/generated/") || text.ends_with(".generated.rs") || text.contains("/gen/")
}

fn file_family(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with(".github/workflows/") {
        return "ci_declarative".to_string();
    }
    if text.starts_with("docs/") || text.ends_with(".md") {
        return "documentation".to_string();
    }
    if text.starts_with("scripts/") {
        return "script".to_string();
    }
    if is_generated_path(path) {
        return "generated_code".to_string();
    }
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "sh" | "bash" => "shell_script".to_string(),
        "py" => "python_tool".to_string(),
        "js" | "ts" | "mjs" | "cjs" => "javascript_tool".to_string(),
        "yml" | "yaml" => "yaml_config".to_string(),
        "json" | "toml" => "configuration".to_string(),
        other if !other.is_empty() => format!("{other}_file"),
        _ => "unknown_file".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_is_not_non_rust() {
        assert!(classify_path(Path::new("src/lib.rs")).is_none());
    }

    #[test]
    fn workflow_is_non_rust() {
        let finding =
            classify_path(Path::new(".github/workflows/ci.yml")).expect("workflow finding");
        assert_eq!(finding.family.as_deref(), Some("ci_declarative"));
    }
}
