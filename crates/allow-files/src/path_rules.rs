use allow_core::{glob_matches, normalize_path};
use std::path::Path;

pub(crate) fn is_scannable_non_rust(path: &Path) -> bool {
    !is_rust_source(path) && !is_builtin_allowed(path)
}

pub fn is_rust_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"))
}

pub(crate) fn is_builtin_allowed(path: &Path) -> bool {
    let text = normalize_path(path);
    let lower = text.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "rust-toolchain.toml" | "rustfmt.toml" | "clippy.toml"
    ) || crate_root_builtin(&lower, "/readme.md")
        || crate_root_builtin(&lower, "/license")
        || crate_root_builtin(&lower, "/license-mit")
        || crate_root_builtin(&lower, "/license-apache")
        || lower == "readme.md"
        || lower == "license"
        || lower == "license-mit"
        || lower == "license-apache"
}

/// Each published crate embeds its own license text so crates.io packaging
/// carries it per package; those copies are packaging metadata, not
/// policy-relevant content.
fn crate_root_builtin(lower: &str, suffix: &str) -> bool {
    lower.starts_with("crates/") && lower.ends_with(suffix)
}

pub(crate) fn is_generated_path(path: &Path, generated_patterns: &[String]) -> bool {
    let text = normalize_path(path);
    let file_name = lower_file_name(path);
    generated_patterns
        .iter()
        .any(|pattern| glob_matches(pattern, path))
        || is_builtin_generated_file_name(&file_name)
        || text.contains("/generated/")
        || text.starts_with("generated/")
        || has_generated_extension(&file_name)
        || text.contains("/gen/")
        || text.starts_with("gen/")
}

/// Detect files whose extension starts with `generated` — e.g.
/// `bindings.generated.rs`, `schema.generated.json`, `output.generated`.
/// This matches `.generated` as a standalone extension or as a
/// double-extension prefix, but NOT compound words like
/// `report-pre-generated.json` where `generated` is part of a larger
/// word (#1877).
fn has_generated_extension(file_name: &str) -> bool {
    // Split extensions: "bindings.generated.rs" → ["bindings", "generated", "rs"].
    // The first component is the file stem; everything after is an extension layer.
    // We check if any extension layer is exactly "generated".
    let mut parts = file_name.split('.');
    // Skip the stem (first component).
    parts.next();
    parts.any(|ext| ext == "generated")
}

fn is_builtin_generated_file_name(file_name: &str) -> bool {
    matches!(
        file_name,
        name if name.ends_with(".pb.go")
            || name.ends_with(".grpc.pb.go")
            || name.ends_with(".pb.rs")
            || name.ends_with(".pb.dart")
            || name.ends_with(".pb.cc")
            || name.ends_with(".pb.h")
            || name.ends_with("_pb2.py")
            || name.ends_with(".g.dart")
            || name.ends_with("_mock.go")
    )
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
    fn is_rust_source_matches_rs_extension_case_insensitive() {
        assert!(is_rust_source(Path::new("src/lib.rs")));
        assert!(is_rust_source(Path::new("src/lib.RS")));
        assert!(is_rust_source(Path::new("src/lib.Rs")));
        assert!(!is_rust_source(Path::new("src/lib.rs.bak")));
        assert!(!is_rust_source(Path::new("src/lib")));
    }

    #[test]
    fn is_builtin_allowed_matches_root_and_crate_readme_license_boundaries() {
        for path in [
            "rust-toolchain.toml",
            "rustfmt.toml",
            "clippy.toml",
            "README.md",
            "LICENSE",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            "crates/allow-core/README.md",
            "crates/allow-core/LICENSE",
            "crates/allow-core/LICENSE-MIT",
            "crates/allow-core/LICENSE-APACHE",
            // Case-insensitive on case-insensitive filesystems (#1822)
            "README.MD",
            "readme.md",
            "license",
        ] {
            assert!(
                is_builtin_allowed(Path::new(path)),
                "{path} should be allowed"
            );
        }

        for path in [
            "docs/README.md",
            "docs/LICENSE-MIT",
            "crates/allow-core/guide.md",
            "crates/allow-core/COPYING-MIT",
            "tools/clippy.toml",
        ] {
            assert!(
                !is_builtin_allowed(Path::new(path)),
                "{path} should not be allowed"
            );
        }
    }

    #[test]
    fn is_builtin_allowed_normalizes_windows_separators() {
        assert!(is_builtin_allowed(Path::new(
            "crates\\allow-core\\README.md"
        )));
        assert!(is_builtin_allowed(Path::new(
            "crates\\allow-core\\LICENSE-MIT"
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
    fn is_generated_path_does_not_fire_on_compound_word_generated() {
        // #1877: the old `file_name.contains(".generated.")` heuristic
        // matched compound words like `report-pre-generated.json` where
        // `generated` is part of a larger word, not a file extension.
        let patterns: Vec<String> = Vec::new();
        for path in [
            "target/report-pre-generated.json",
            "docs/pre-generated-summary.md",
            "src/recently-generated-check.rs",
        ] {
            assert!(
                !is_generated_path(Path::new(path), &patterns),
                "{path} should NOT be classified as generated"
            );
        }
        // But legitimate `.generated` extensions still match.
        for path in [
            "src/api.generated.rs",
            "src/api.generated",
            "src/bindings.generated.go",
        ] {
            assert!(
                is_generated_path(Path::new(path), &patterns),
                "{path} SHOULD be classified as generated (legitimate .generated extension)"
            );
        }
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
