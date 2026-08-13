//! Source-syntax Rust scanners for cargo-allow exception findings.
//!
//! This crate scans `.rs` file text for syntax-visible unsafe, panic-family,
//! indexing/slicing, lint-suppression, and exact source-declared test surfaces.
//! Test inventory establishes source identity and reports cfg, generated-case,
//! and target-membership limits explicitly; implicit binary roots do not claim
//! unrelated library modules. It parses source directly without invoking Cargo,
//! rustc, Clippy, build scripts, proc macros, macro expansion, type analysis, or
//! MIR.

use allow_core::{CargoAllowResult, Finding, read_text_file_capped};
use rayon::prelude::*;
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
mod scan_cache;
mod scan_result;
mod syntax_coupling;
mod syntax_facts;
mod syntax_kinds;
mod syntax_tree;
mod test_subjects;
mod text;

use line_scan::scan_source_lines;
use package::source_package_contexts;

pub use package::{
    SourcePackageContext, apply_source_package_context, source_package_contexts_from_sources,
};
pub use scan_cache::ScanCache;
pub use scan_result::{RustFileScanOutcome, RustFileScanStatus, RustScanResult};
pub use syntax_coupling::{
    RustSourceCoupling, RustSourceCouplingKind, RustSourceCouplingPathBase, RustSourceCouplingScan,
    rust_source_declares_no_std, scan_rust_source_coupling,
    scan_rust_source_coupling_with_manifest_env,
};
pub use syntax_tree::{RustSyntaxContainer, RustSyntaxTree, parse_rust_syntax};
pub use test_subjects::{
    RustTestInventory, RustTestInventoryDiagnostic, RustTestInventoryDiagnosticKind,
    RustTestInventoryOptions, RustTestInventoryStatus, RustTestResolution, RustTestSelector,
    RustTestSourceRange, RustTestSubject, RustTestTargetIdentity, RustTestTargetKind,
    inventory_rust_test_subjects, inventory_rust_test_subjects_from_sources,
    resolve_rust_test_selector,
};

/// Finding families emitted by the source-syntax scanner.
///
/// Adding a new public family requires adding its capability row in the
/// cargo-allow catalog in the same change.
pub const SOURCE_FINDING_FAMILIES: &[(&str, &str)] = &[
    ("panic", "unwrap"),
    ("panic", "expect"),
    ("panic", "panic_macro"),
    ("panic", "todo"),
    ("panic", "unimplemented"),
    ("panic", "unreachable"),
    ("panic", "indexing"),
    ("panic", "string_slice"),
    ("unsafe", "unsafe_fn"),
    ("unsafe", "unsafe_impl"),
    ("unsafe", "unsafe_trait"),
    ("unsafe", "unsafe_extern_block"),
    ("unsafe", "unsafe_block"),
    ("unsafe", "unsafe_const"),
    ("unsafe", "unsafe_static"),
    ("unsafe", "unsafe_attr"),
    ("lint_exception", "allow_attribute"),
    ("lint_exception", "expect_attribute"),
    ("lint_exception", "deny_attribute"),
    ("lint_exception", "forbid_attribute"),
    ("lint_exception", "warn_attribute"),
];

pub fn scan_rust_files(
    root: impl AsRef<Path>,
    files: &[PathBuf],
) -> CargoAllowResult<RustScanResult> {
    let root = root.as_ref();
    let mut out = Vec::new();
    let packages = source_package_contexts(root, files)?;
    let rust_files = files
        .iter()
        .filter(|rel| rel.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect::<Vec<_>>();
    let files_considered = rust_files.len();
    let mut files_skipped = 0usize;
    let mut files_with_parse_errors = 0usize;
    let mut file_statuses = Vec::with_capacity(files_considered);
    let scanned = rust_files
        .par_iter()
        .map(|rel| {
            let path = root.join(rel);
            // Read each file independently — a single unreadable, non-UTF8,
            // or oversized file must NOT abort the entire workspace scan
            // (#1882, #1916). Keep the outcome indexed so aggregation below
            // can preserve input order and warning order.
            let outcome = match read_text_file_capped(&path) {
                Ok(text) => {
                    // Strip leading UTF-8 BOM so crate-level #![...] attributes
                    // are detected on BOM-prefixed files (common on
                    // Windows-edited sources) (#1881).
                    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
                    FileScanOutcome::Scanned(scan_rust_source_with_completeness(rel, text))
                }
                Err(error) => FileScanOutcome::Skipped(error.to_string()),
            };
            ((*rel).clone(), outcome)
        })
        .collect::<Vec<_>>();

    for (rel, outcome) in scanned {
        let path = root.join(&rel);
        match outcome {
            FileScanOutcome::Scanned(scan) => {
                if scan.has_parse_error {
                    files_with_parse_errors += 1;
                }
                file_statuses.push(RustFileScanStatus {
                    path: rel.clone(),
                    outcome: if scan.has_parse_error {
                        RustFileScanOutcome::ParseError
                    } else {
                        RustFileScanOutcome::Scanned
                    },
                });
                let mut findings = scan.findings;
                apply_source_package_context(&rel, &packages, &mut findings);
                out.extend(findings);
            }
            FileScanOutcome::Skipped(error) => {
                eprintln!("warning: skipping {} (read error: {error})", path.display());
                files_skipped += 1;
                file_statuses.push(RustFileScanStatus {
                    path: rel.clone(),
                    outcome: RustFileScanOutcome::Skipped { reason: error },
                });
            }
        }
    }
    file_statuses.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(RustScanResult {
        findings: out,
        files_considered,
        files_skipped,
        files_with_parse_errors,
        file_statuses,
    })
}

enum FileScanOutcome {
    Scanned(RustSourceScan),
    Skipped(String),
}

/// Scan outcome for a single Rust source file, including parse completeness.
#[derive(Debug, Clone)]
pub struct RustSourceScan {
    pub findings: Vec<Finding>,
    pub has_parse_error: bool,
}

pub fn scan_rust_source(path: impl AsRef<Path>, source: &str) -> Vec<Finding> {
    scan_rust_source_with_completeness(path, source).findings
}

pub fn scan_rust_source_with_completeness(path: impl AsRef<Path>, source: &str) -> RustSourceScan {
    let path = path.as_ref().to_path_buf();
    let outcome = syntax_facts::syntax_facts_with_outcome(source);
    let findings = scan_source_lines(&path, source, outcome.facts);
    RustSourceScan {
        findings,
        has_parse_error: outcome.has_parse_error,
    }
}

/// Scan Rust files with an mtime+size cache for incremental re-evaluation.
/// On a repeat scan, files whose mtime+size hasn't changed are served from
/// the cache instead of re-parsing. Falls through to a full re-parse on any
/// cache miss. Package context is still applied per-file.
pub fn scan_rust_files_cached(
    root: impl AsRef<Path>,
    files: &[PathBuf],
    cache: &mut ScanCache,
) -> CargoAllowResult<RustScanResult> {
    let root = root.as_ref();
    let mut out = Vec::new();
    let packages = source_package_contexts(root, files)?;
    let mut files_considered = 0usize;
    let mut files_skipped = 0usize;
    let mut files_with_parse_errors = 0usize;
    let mut file_statuses = Vec::new();
    for rel in files {
        if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        files_considered += 1;
        let path = root.join(rel);
        // Scan the file via cache; scan_file returns a skipped flag when the
        // file can't be read (oversized, binary, permission-denied) (#2801).
        // This eliminates the previous double-read: the probe read at line 140
        // was redundant with scan_file's own read.
        let (mut findings, has_parse_error, skipped) = cache.scan_file(root, rel)?;
        if skipped {
            files_skipped += 1;
            eprintln!("warning: skipping {} (read error)", path.display());
            file_statuses.push(RustFileScanStatus {
                path: rel.clone(),
                outcome: RustFileScanOutcome::Skipped {
                    reason: "read failed, non-UTF-8, or file exceeded the scanner cap".to_string(),
                },
            });
            continue;
        }
        if has_parse_error {
            files_with_parse_errors += 1;
        }
        file_statuses.push(RustFileScanStatus {
            path: rel.clone(),
            outcome: if has_parse_error {
                RustFileScanOutcome::ParseError
            } else {
                RustFileScanOutcome::Scanned
            },
        });
        apply_source_package_context(rel, &packages, &mut findings);
        out.extend(findings);
    }
    file_statuses.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(RustScanResult {
        findings: out,
        files_considered,
        files_skipped,
        files_with_parse_errors,
        file_statuses,
    })
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
