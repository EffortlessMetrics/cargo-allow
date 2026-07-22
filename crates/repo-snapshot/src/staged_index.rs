//! Staged Git index snapshot surface (#2583).
//!
//! PR1 skeleton: parity contracts only. Staged-byte semantics and negative fixtures
//! land in packet 2583-B.

/// Marker for the staged index module boundary during extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedIndexSurface;

impl StagedIndexSurface {
    pub const MODULE_ID: &'static str = "repo-snapshot::staged_index";
}
