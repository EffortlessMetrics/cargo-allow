use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::Path;

pub(crate) fn validate_path_scope(id: &str, path: &Path) -> CargoAllowResult<()> {
    validate_source_tree_scope(id, &path.to_string_lossy(), SourceTreeScopeDiagnostic::Path)
}

pub(crate) fn validate_glob(label: &str, glob: &str) -> CargoAllowResult<()> {
    validate_source_tree_scope(label, glob, SourceTreeScopeDiagnostic::Glob)
}

pub(crate) fn normalize_source_tree_scope(scope: &str) -> String {
    scope.replace('\\', "/")
}

#[derive(Debug, Clone, Copy)]
enum SourceTreeScopeDiagnostic {
    Path,
    Glob,
}

fn validate_source_tree_scope(
    label: &str,
    scope: &str,
    diagnostic: SourceTreeScopeDiagnostic,
) -> CargoAllowResult<()> {
    let text = normalize_source_tree_scope(scope);
    if text.trim().is_empty() {
        return Err(CargoAllowError::new(diagnostic.empty_message(label)));
    }
    if text.trim() != text {
        return Err(CargoAllowError::new(
            diagnostic.surrounding_whitespace_message(label),
        ));
    }
    if text.starts_with('/') || text.contains(':') {
        return Err(CargoAllowError::new(
            diagnostic.source_tree_relative_message(label),
        ));
    }
    if text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::new(
            diagnostic.parent_segment_message(label),
        ));
    }
    if text != "." && text.split('/').any(|part| part == ".") {
        return Err(CargoAllowError::new(
            diagnostic.current_segment_message(label),
        ));
    }
    if text.split('/').any(|part| part.is_empty()) {
        return Err(CargoAllowError::new(
            diagnostic.empty_segment_message(label),
        ));
    }
    match diagnostic {
        SourceTreeScopeDiagnostic::Path => validate_exact_path_syntax(label, &text)?,
        SourceTreeScopeDiagnostic::Glob => validate_supported_glob_syntax(label, &text)?,
    }
    Ok(())
}

fn validate_exact_path_syntax(label: &str, path: &str) -> CargoAllowResult<()> {
    if let Some(ch) = path.chars().find(|ch| matches!(ch, '*' | '?')) {
        return Err(CargoAllowError::new(format!(
            "{label} path uses wildcard token `{ch}`; use `glob` for source-tree patterns"
        )));
    }
    Ok(())
}

fn validate_supported_glob_syntax(label: &str, glob: &str) -> CargoAllowResult<()> {
    if glob_covers_entire_source_tree(glob) {
        return Err(CargoAllowError::new(format!(
            "{label} covers the entire source tree; use a narrower path or glob scope"
        )));
    }
    if let Some(ch) = glob.chars().find(|ch| matches!(ch, '[' | ']' | '{' | '}')) {
        return Err(CargoAllowError::new(format!(
            "{label} uses unsupported glob token `{ch}`; supported source-tree glob tokens are `*`, `?`, and whole-segment `**`"
        )));
    }
    if glob
        .split('/')
        .any(|segment| segment.contains("**") && segment != "**")
    {
        return Err(CargoAllowError::new(format!(
            "{label} uses unsupported glob token `**`; `**` must occupy a whole source-tree path segment"
        )));
    }
    Ok(())
}

fn glob_covers_entire_source_tree(glob: &str) -> bool {
    let mut has_segment = false;
    let mut globstar_segments = 0;
    let mut wildcard_segments = 0;
    for segment in glob.split('/').filter(|segment| !segment.is_empty()) {
        has_segment = true;
        match segment {
            "**" => globstar_segments += 1,
            "*" => wildcard_segments += 1,
            _ => return false,
        }
    }
    has_segment && globstar_segments > 0 && wildcard_segments <= 1
}

impl SourceTreeScopeDiagnostic {
    fn empty_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} has empty path"),
            Self::Glob => format!("{label} is empty"),
        }
    }

    fn source_tree_relative_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must be source-tree-relative"),
            Self::Glob => format!("{label} must be source-tree-relative"),
        }
    }

    fn surrounding_whitespace_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not have leading or trailing whitespace"),
            Self::Glob => format!("{label} must not have leading or trailing whitespace"),
        }
    }

    fn parent_segment_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not contain parent directory segments"),
            Self::Glob => format!("{label} must not contain parent directory segments"),
        }
    }

    fn current_segment_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not contain current directory segments"),
            Self::Glob => format!("{label} must not contain current directory segments"),
        }
    }

    fn empty_segment_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not contain empty path segments"),
            Self::Glob => format!("{label} must not contain empty path segments"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use allow_core::CargoAllowResult;

    use super::*;

    fn err_text(result: CargoAllowResult<()>) -> String {
        match result {
            Ok(()) => String::new(),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn normalize_source_tree_scope_converts_windows_separators() {
        assert_eq!(
            normalize_source_tree_scope(r"crates\allow-policy\src\lib.rs"),
            "crates/allow-policy/src/lib.rs"
        );
        assert_eq!(normalize_source_tree_scope("src/lib.rs"), "src/lib.rs");
    }

    #[test]
    fn validate_path_scope_accepts_source_tree_relative_paths() {
        for path in [
            Path::new("."),
            Path::new("src/lib.rs"),
            Path::new(r"src\lib.rs"),
        ] {
            assert!(validate_path_scope("allow-path", path).is_ok());
        }
    }

    #[test]
    fn validate_path_scope_rejects_non_source_tree_path_boundaries() {
        let cases = [
            (Path::new(""), "allow-empty has empty path"),
            (
                Path::new(" src/lib.rs"),
                "allow-space path must not have leading or trailing whitespace",
            ),
            (
                Path::new("/src/lib.rs"),
                "allow-absolute path must be source-tree-relative",
            ),
            (
                Path::new("C:/src/lib.rs"),
                "allow-drive path must be source-tree-relative",
            ),
            (
                Path::new("src/../lib.rs"),
                "allow-parent path must not contain parent directory segments",
            ),
            (
                Path::new("src/./lib.rs"),
                "allow-current path must not contain current directory segments",
            ),
            (
                Path::new("src//lib.rs"),
                "allow-empty-segment path must not contain empty path segments",
            ),
            (
                Path::new("src/*.rs"),
                "allow-wildcard path uses wildcard token `*`; use `glob` for source-tree patterns",
            ),
            (
                Path::new("src/file?.rs"),
                "allow-question path uses wildcard token `?`; use `glob` for source-tree patterns",
            ),
        ];

        for (index, (path, expected)) in cases.iter().enumerate() {
            let label = match index {
                0 => "allow-empty",
                1 => "allow-space",
                2 => "allow-absolute",
                3 => "allow-drive",
                4 => "allow-parent",
                5 => "allow-current",
                6 => "allow-empty-segment",
                7 => "allow-wildcard",
                _ => "allow-question",
            };
            assert_eq!(err_text(validate_path_scope(label, path)), *expected);
        }
    }

    #[test]
    fn validate_glob_accepts_supported_relative_glob_tokens() {
        for glob in ["src/*.rs", "src/?.rs", "src/**/*.rs", r"docs\*.md"] {
            assert!(validate_glob("glob", glob).is_ok(), "{glob}");
        }
    }

    #[test]
    fn validate_glob_rejects_invalid_scope_boundaries() {
        let cases = [
            ("", "glob-empty is empty"),
            (
                " src/*.rs",
                "glob-space must not have leading or trailing whitespace",
            ),
            ("/src/*.rs", "glob-absolute must be source-tree-relative"),
            ("C:/src/*.rs", "glob-drive must be source-tree-relative"),
            (
                "src/../*.rs",
                "glob-parent must not contain parent directory segments",
            ),
            (
                "src/./*.rs",
                "glob-current must not contain current directory segments",
            ),
            (
                "src//*.rs",
                "glob-empty-segment must not contain empty path segments",
            ),
        ];

        for (index, (glob, expected)) in cases.iter().enumerate() {
            let label = match index {
                0 => "glob-empty",
                1 => "glob-space",
                2 => "glob-absolute",
                3 => "glob-drive",
                4 => "glob-parent",
                5 => "glob-current",
                _ => "glob-empty-segment",
            };
            assert_eq!(err_text(validate_glob(label, glob)), *expected);
        }
    }

    #[test]
    fn validate_glob_rejects_unsupported_and_repository_wide_patterns() {
        let cases = [
            (
                "**",
                "glob-wide covers the entire source tree; use a narrower path or glob scope",
            ),
            (
                "**/*",
                "glob-wide covers the entire source tree; use a narrower path or glob scope",
            ),
            (
                "*/**",
                "glob-wide covers the entire source tree; use a narrower path or glob scope",
            ),
            (
                "src/[ab].rs",
                "glob-token uses unsupported glob token `[`; supported source-tree glob tokens are `*`, `?`, and whole-segment `**`",
            ),
            (
                "src/{a,b}.rs",
                "glob-token uses unsupported glob token `{`; supported source-tree glob tokens are `*`, `?`, and whole-segment `**`",
            ),
            (
                "src/**.rs",
                "glob-token uses unsupported glob token `**`; `**` must occupy a whole source-tree path segment",
            ),
        ];

        for (glob, expected) in cases {
            let label = if glob.starts_with("src/") {
                "glob-token"
            } else {
                "glob-wide"
            };
            assert_eq!(err_text(validate_glob(label, glob)), expected);
        }
    }

    #[test]
    fn direct_syntax_helpers_report_exact_error_discriminators() {
        assert!(validate_exact_path_syntax("path", "src/lib.rs").is_ok());
        assert_eq!(
            err_text(validate_exact_path_syntax("path", "src/*.rs")),
            "path path uses wildcard token `*`; use `glob` for source-tree patterns"
        );
        assert_eq!(
            err_text(validate_exact_path_syntax("path", "src/file?.rs")),
            "path path uses wildcard token `?`; use `glob` for source-tree patterns"
        );

        assert!(validate_supported_glob_syntax("glob", "src/**/*.rs").is_ok());
        assert_eq!(
            err_text(validate_supported_glob_syntax("glob", "**")),
            "glob covers the entire source tree; use a narrower path or glob scope"
        );
        assert_eq!(
            err_text(validate_supported_glob_syntax("glob", "src/[ab].rs")),
            "glob uses unsupported glob token `[`; supported source-tree glob tokens are `*`, `?`, and whole-segment `**`"
        );
        assert_eq!(
            err_text(validate_supported_glob_syntax("glob", "src/{a,b}.rs")),
            "glob uses unsupported glob token `{`; supported source-tree glob tokens are `*`, `?`, and whole-segment `**`"
        );
        assert_eq!(
            err_text(validate_supported_glob_syntax("glob", "src/**.rs")),
            "glob uses unsupported glob token `**`; `**` must occupy a whole source-tree path segment"
        );
    }

    #[test]
    fn glob_covers_entire_source_tree_distinguishes_root_wide_from_scoped() {
        for glob in ["**", "**/*", "*/**"] {
            assert!(glob_covers_entire_source_tree(glob), "{glob}");
        }

        for glob in [
            "*/**/*",
            "src/**",
            "src/**/*",
            "src/*.rs",
            "src/**/mod.rs",
            "src",
        ] {
            assert!(!glob_covers_entire_source_tree(glob), "{glob}");
        }
    }

    #[test]
    fn diagnostic_messages_keep_path_and_glob_wording_distinct() {
        let diagnostics = [
            (
                SourceTreeScopeDiagnostic::Path.empty_message("item"),
                "item has empty path",
            ),
            (
                SourceTreeScopeDiagnostic::Glob.empty_message("item"),
                "item is empty",
            ),
            (
                SourceTreeScopeDiagnostic::Path.source_tree_relative_message("item"),
                "item path must be source-tree-relative",
            ),
            (
                SourceTreeScopeDiagnostic::Glob.source_tree_relative_message("item"),
                "item must be source-tree-relative",
            ),
            (
                SourceTreeScopeDiagnostic::Path.surrounding_whitespace_message("item"),
                "item path must not have leading or trailing whitespace",
            ),
            (
                SourceTreeScopeDiagnostic::Glob.surrounding_whitespace_message("item"),
                "item must not have leading or trailing whitespace",
            ),
            (
                SourceTreeScopeDiagnostic::Path.parent_segment_message("item"),
                "item path must not contain parent directory segments",
            ),
            (
                SourceTreeScopeDiagnostic::Glob.parent_segment_message("item"),
                "item must not contain parent directory segments",
            ),
            (
                SourceTreeScopeDiagnostic::Path.current_segment_message("item"),
                "item path must not contain current directory segments",
            ),
            (
                SourceTreeScopeDiagnostic::Glob.current_segment_message("item"),
                "item must not contain current directory segments",
            ),
            (
                SourceTreeScopeDiagnostic::Path.empty_segment_message("item"),
                "item path must not contain empty path segments",
            ),
            (
                SourceTreeScopeDiagnostic::Glob.empty_segment_message("item"),
                "item must not contain empty path segments",
            ),
        ];

        for (actual, expected) in diagnostics {
            assert_eq!(actual, expected);
        }
    }
}
