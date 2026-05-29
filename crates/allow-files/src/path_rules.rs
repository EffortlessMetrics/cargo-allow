use allow_core::{glob_matches, normalize_path};
use std::path::Path;

pub fn is_rust_source(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("rs")
}

pub(crate) fn is_builtin_allowed(path: &Path) -> bool {
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

pub(crate) fn is_generated_path(path: &Path, generated_patterns: &[String]) -> bool {
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

pub(crate) fn file_fingerprint(path: &Path) -> Option<String> {
    lower_extension(path).or_else(|| {
        let file_name = lower_file_name(path);
        (!file_name.is_empty()).then_some(file_name)
    })
}

pub(crate) fn lower_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

pub(crate) fn lower_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default()
}
