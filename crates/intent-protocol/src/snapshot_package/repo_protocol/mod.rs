//! Re-export of shared repo-protocol types from effortless-repo-protocol (#3308).
//!
//! These types were previously copied into intent-protocol. They are now
//! consumed directly from the product-neutral shared protocol crate so there
//! is a single source of truth for repository snapshot/result-class vocabulary.

pub use effortless_repo_protocol::{
    REPOSITORY_SNAPSHOT_SCHEMA_ID, RepositorySnapshotKindV1, RepositorySnapshotV1,
    ResolvedRevisionV1, ResultClassV1, SelectedPathIdentityV1,
};
