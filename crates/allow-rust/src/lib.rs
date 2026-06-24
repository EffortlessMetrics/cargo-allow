//! Source-syntax Rust scanners for cargo-allow exception findings.
//!
//! This crate scans `.rs` file text for syntax-visible unsafe, panic-family,
//! indexing/slicing, and lint-suppression surfaces. It parses source directly
//! without invoking Cargo, rustc, Clippy, build scripts, proc macros, macro
//! expansion, type analysis, or MIR.

use allow_core::{CargoAllowResult, Finding};
use std::fs;
use std::path::{Path, PathBuf};

mod finding_builder;
mod line_context;
mod line_facts;
mod line_findings;
mod line_index_findings;
mod line_lint_findings;
mod line_panic_findings;
mod line_scan;
mod line_unsafe_findings;
mod package;
mod safety_comments;
mod syntax_facts;
mod syntax_kinds;
mod syntax_tree;
mod text;

use line_scan::scan_source_lines;
use package::source_package_contexts;
use syntax_facts::syntax_facts;

pub use package::{
    SourcePackageContext, apply_source_package_context, source_package_contexts_from_sources,
};
pub use syntax_tree::{RustSyntaxContainer, RustSyntaxTree, parse_rust_syntax};

pub fn scan_rust_files(
    root: impl AsRef<Path>,
    files: &[PathBuf],
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut out = Vec::new();
    let packages = source_package_contexts(root, files)?;
    for rel in files {
        if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let path = root.join(rel);
        // Read each file independently — a single unreadable or non-UTF8
        // file must NOT abort the entire workspace scan (#1882). Skip the
        // file and continue scanning the rest.
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("warning: skipping {} (read error: {e})", path.display());
                continue;
            }
        };
        // Strip leading UTF-8 BOM so crate-level #![...] attributes are
        // detected on BOM-prefixed files (common on Windows-edited sources)
        // (#1881).
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let mut findings = scan_rust_source(rel, text);
        apply_source_package_context(rel, &packages, &mut findings);
        out.extend(findings);
    }
    Ok(out)
}

pub fn scan_rust_source(path: impl AsRef<Path>, source: &str) -> Vec<Finding> {
    let path = path.as_ref().to_path_buf();
    let syntax = syntax_facts(source);
    scan_source_lines(&path, source, syntax)
}

#[cfg(test)]
mod tests;

/// Heuristically detect whether a path is a test-only source file (#1798).
///
/// Returns true for:
/// - Files under a `tests/` directory (integration tests)
/// - Files ending in `_tests.rs` (inline test modules)
/// - Files named `tests.rs`
///
/// Findings from these files are typically not production exceptions
/// and can be filtered by callers to avoid polluting production scans.
pub fn is_likely_test_file(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    let text = path.to_string_lossy();
    let normalized = text.replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or("");
    normalized.contains("/tests/") || file_name.ends_with("_tests.rs") || file_name == "tests.rs"
}
