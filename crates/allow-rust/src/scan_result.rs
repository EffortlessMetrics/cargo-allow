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
    pub file_statuses: Vec<RustFileScanStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFileScanStatus {
    pub path: std::path::PathBuf,
    pub outcome: RustFileScanOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustFileScanOutcome {
    Scanned,
    ParseError,
    Skipped { reason: String },
}

impl RustScanResult {
    /// Merge two scan results (used when combining base scan + companion).
    pub fn merge(mut self, other: RustScanResult) -> RustScanResult {
        self.findings.extend(other.findings);
        self.files_considered += other.files_considered;
        self.files_skipped += other.files_skipped;
        self.files_with_parse_errors += other.files_with_parse_errors;
        self.file_statuses.extend(other.file_statuses);
        self.file_statuses
            .sort_by(|left, right| left.path.cmp(&right.path));
        self
    }

    pub fn status_for(&self, path: &std::path::Path) -> Option<&RustFileScanOutcome> {
        self.file_statuses
            .binary_search_by(|status| status.path.as_path().cmp(path))
            .ok()
            .and_then(|index| self.file_statuses.get(index))
            .map(|status| &status.outcome)
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

#[cfg(test)]
mod tests {
    use super::{RustFileScanOutcome, RustFileScanStatus, RustScanResult};
    use std::path::Path;

    #[test]
    fn status_for_distinguishes_scanned_parse_error_and_skipped_files() -> Result<(), String> {
        let result = RustScanResult {
            findings: Vec::new(),
            files_considered: 3,
            files_skipped: 1,
            files_with_parse_errors: 1,
            file_statuses: vec![
                RustFileScanStatus {
                    path: "src/clean.rs".into(),
                    outcome: RustFileScanOutcome::Scanned,
                },
                RustFileScanStatus {
                    path: "src/invalid.rs".into(),
                    outcome: RustFileScanOutcome::ParseError,
                },
                RustFileScanStatus {
                    path: "src/large.rs".into(),
                    outcome: RustFileScanOutcome::Skipped {
                        reason: "file exceeded the scanner cap".to_string(),
                    },
                },
            ],
        };

        if result.status_for(Path::new("src/clean.rs")) != Some(&RustFileScanOutcome::Scanned)
            || result.status_for(Path::new("src/invalid.rs"))
                != Some(&RustFileScanOutcome::ParseError)
            || result.status_for(Path::new("src/large.rs"))
                != Some(&RustFileScanOutcome::Skipped {
                    reason: "file exceeded the scanner cap".to_string(),
                })
            || result.status_for(Path::new("src/missing.rs")).is_some()
        {
            return Err("per-file statuses did not preserve their typed outcomes".to_string());
        }
        Ok(())
    }

    #[test]
    fn merge_keeps_deterministic_status_order() -> Result<(), String> {
        let left = RustScanResult {
            findings: Vec::new(),
            files_considered: 1,
            files_skipped: 0,
            files_with_parse_errors: 0,
            file_statuses: vec![RustFileScanStatus {
                path: "z.rs".into(),
                outcome: RustFileScanOutcome::Scanned,
            }],
        };
        let right = RustScanResult {
            findings: Vec::new(),
            files_considered: 1,
            files_skipped: 1,
            files_with_parse_errors: 0,
            file_statuses: vec![RustFileScanStatus {
                path: "a.rs".into(),
                outcome: RustFileScanOutcome::Skipped {
                    reason: "read failed".to_string(),
                },
            }],
        };

        let merged = left.merge(right);
        let first = merged
            .file_statuses
            .first()
            .ok_or_else(|| "merged status list was empty".to_string())?;
        let second = merged
            .file_statuses
            .get(1)
            .ok_or_else(|| "merged status list had only one item".to_string())?;
        if first.path != Path::new("a.rs") || second.path != Path::new("z.rs") {
            return Err("merged status list was not sorted by path".to_string());
        }
        Ok(())
    }
}
