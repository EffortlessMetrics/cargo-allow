//! Exact repository source views for the cargo-allow three-product extraction (#2583).
//!
//! This crate owns committed tree, staged index, and working-tree source views for
//! source-tree snapshot consumers. It does not invoke Cargo, compile code, or execute
//! repository policy.
//!
//! ## Product-neutrality contract (#3146)
//!
//! This is a shared substrate crate: its runtime dependencies are neutral
//! (serde/sha2/toml only), its public surface carries no product-domain
//! vocabulary, and no product crate (allow-*, intent-*, proof-*,
//! cargo-allow/intent/proof) may appear in its dependency graph. The
//! parity cases and cutover machinery live with their consumers. Registry
//! publication posture is decided by #3386, not here.

mod error;
mod git;
mod inventory;
mod parity;
mod revision_identity;
mod source_view;
mod staged_index;
mod util;

#[cfg(test)]
mod protocol_adapter;

pub use error::{SnapshotDiagnostic, SnapshotError, SnapshotErrorKind, SnapshotResult};
pub use inventory::{SourceInventory, SourceInventoryCompleteness, SourceInventorySource};
#[doc(hidden)]
pub use parity::{ParityContract, load_parity_contract, parity_contract_paths};
pub use revision_identity::RevisionIdentitySurface;
pub use revision_identity::{
    REPOSITORY_SNAPSHOT_SCHEMA, RepositoryDirtyState, RepositoryObjectFormat,
    RepositorySnapshotIdentity, RepositorySnapshotKind, RepositorySnapshotRequest,
    ResolvedRevisionCapability, ResolvedRevisionIdentity, SelectedPathIdentity,
    repository_object_format, repository_snapshot, repository_snapshot_from_capability,
    resolve_dirty_state, resolve_revision_capability, resolve_revision_identity,
};
pub use source_view::RepositorySourceView;
pub use staged_index::StagedIndexSurface;
pub use staged_index::{
    STAGED_GIT_CAPABILITY_GENERATION, StagedEntryKind, StagedGitCapabilities, StagedIndexEntry,
    StagedPathChange, StagedPathRead, StagedPathStatus, StagedRepositorySnapshot,
    StagedSnapshotCompleteness, StagedSnapshotIdentity, probe_staged_git_capabilities,
    read_staged_path, read_staged_raw_path, staged_repository_snapshot,
};

#[cfg(test)]
mod source_view_tests;

#[cfg(test)]
mod tests;
