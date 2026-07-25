//! Minimal scanner completeness tracking for false-clean prevention (#2486).
//!
//! When the scanner skips a file (unreadable, non-UTF8, oversized), the
//! result must be visible to callers so `check --mode no-new` can fail
//! closed instead of printing a green receipt over an incomplete scan.

/// Result of scanning Rust files, carrying both findings and completeness
/// metadata so callers can distinguish "no findings" from "skipped files."
#[derive(Debug, Clone)]
pub struct RustScanResult {
    /// All findings from successfully-scanned files.
    pub findings: Vec<crate::Finding>,
    /// Number of `.rs` files the scanner attempted to read.
    pub files_considered: usize,
    /// Number of `.rs` files that were skipped due to read errors.
    pub files_skipped: usize,
    /// Number of `.rs` files whose tree-sitter parse tree contains errors.
    pub files_with_parse_errors: usize,
}

impl RustScanResult {
    /// Merge two scan results (used when combining base scan + companion).
    pub fn merge(mut self, other: RustScanResult) -> RustScanResult {
        self.findings.extend(other.findings);
        self.files_considered += other.files_considered;
        self.files_skipped += other.files_skipped;
        self.files_with_parse_errors += other.files_with_parse_errors;
        self
    }

    /// Whether any tracked Rust files were skipped during the scan.
    pub fn has_skipped(&self) -> bool {
        self.files_skipped > 0
    }

    /// Whether any successfully-read Rust files had tree-sitter parse errors.
    pub fn has_parse_errors(&self) -> bool {
        self.files_with_parse_errors > 0
    }
}

impl From<RustScanResult> for Vec<crate::Finding> {
    fn from(result: RustScanResult) -> Self {
        result.findings
    }
}
