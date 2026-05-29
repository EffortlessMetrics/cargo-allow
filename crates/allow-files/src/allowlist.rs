use std::path::Path;

pub(crate) fn is_rust_source(path: &Path) -> bool {
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
