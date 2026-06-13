use allow_core::{glob_matches, normalize_path};
use std::path::Path;

pub(crate) fn is_scannable_non_rust(path: &Path) -> bool {
    !is_rust_source(path) && !is_builtin_allowed(path)
}

pub fn is_rust_source(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("rs")
}

pub(crate) fn is_builtin_allowed(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    matches!(
        text.as_str(),
        "rust-toolchain.toml" | "rustfmt.toml" | "clippy.toml"
    ) || text.starts_with("crates/") && text.ends_with("/README.md")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_scannable_non_rust_excludes_rust_sources_and_builtin_files() {
        assert!(!is_scannable_non_rust(Path::new("src/lib.rs")));
        assert!(!is_scannable_non_rust(Path::new("README.md")));
        assert!(!is_scannable_non_rust(Path::new(
            "crates/allow-core/README.md"
        )));
        assert!(is_scannable_non_rust(Path::new("docs/guide.md")));
    }

    #[test]
    fn is_rust_source_matches_lowercase_rs_extension_only() {
        assert!(is_rust_source(Path::new("src/lib.rs")));
        assert!(!is_rust_source(Path::new("src/lib.RS")));
        assert!(!is_rust_source(Path::new("src/lib.rs.bak")));
        assert!(!is_rust_source(Path::new("src/lib")));
    }

    #[test]
    fn is_builtin_allowed_matches_root_and_crate_readme_boundaries() {
        for path in [
            "rust-toolchain.toml",
            "rustfmt.toml",
            "clippy.toml",
            "README.md",
            "LICENSE",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            "crates/allow-core/README.md",
        ] {
            assert!(is_builtin_allowed(Path::new(path)), "{path}");
        }

        for path in [
            "docs/README.md",
            "crates/allow-core/guide.md",
            "README.MD",
            "LICENSE.md",
            "tools/clippy.toml",
        ] {
            assert!(!is_builtin_allowed(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn is_builtin_allowed_normalizes_windows_separators() {
        assert!(is_builtin_allowed(Path::new(
            "crates\\allow-core\\README.md"
        )));
    }

    #[test]
    fn is_generated_path_matches_configured_globs_and_generated_segments() {
        let patterns = vec!["fixtures/generated/**".to_string()];

        for path in [
            "fixtures/generated/output.txt",
            "generated/schema.json",
            "src/generated/bindings.rs",
            "src/gen/bindings.rs",
            "gen/bindings.rs",
            "src/api.generated.rs",
            "src/api.generated",
        ] {
            assert!(is_generated_path(Path::new(path), &patterns), "{path}");
        }

        assert!(!is_generated_path(
            Path::new("src/general/bindings.rs"),
            &patterns
        ));
    }

    #[test]
    fn lower_extension_lowercases_unicode_safe_extensions() {
        assert_eq!(
            lower_extension(Path::new("docs/Guide.MD")),
            Some("md".to_string())
        );
        assert_eq!(lower_extension(Path::new("Makefile")), None);
    }

    #[test]
    fn lower_file_name_lowercases_file_name_or_defaults() {
        assert_eq!(
            lower_file_name(Path::new("docs/Guide.MD")),
            "guide.md".to_string()
        );
        assert_eq!(lower_file_name(Path::new("docs")), "docs".to_string());
        assert_eq!(lower_file_name(PathBuf::new().as_path()), "");
    }
}
