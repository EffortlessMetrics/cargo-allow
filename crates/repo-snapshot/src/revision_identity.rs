//! Committed revision/tree identity surface (#2583).
//!
//! PR1 skeleton: parity contracts only. Implementation moves from `allow-diff` in
//! packet 2583-B/C.

/// Marker for the revision identity module boundary during extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionIdentitySurface;

impl RevisionIdentitySurface {
    pub const MODULE_ID: &'static str = "repo-snapshot::revision_identity";
}
