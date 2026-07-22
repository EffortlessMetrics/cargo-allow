//! Adapter from `allow-diff` repository snapshot identities to `repo-protocol`
//! transport envelopes (#2582 parity evidence).

use crate::revision_identity::{
    RepositorySnapshotIdentity, RepositorySnapshotKind, ResolvedRevisionIdentity,
    SelectedPathIdentity,
};
use repo_protocol::{
    RepositorySnapshotKindV1, RepositorySnapshotV1, ResolvedRevisionV1, SelectedPathIdentityV1,
};

pub fn repository_snapshot_v1_from_identity(
    identity: &RepositorySnapshotIdentity,
) -> RepositorySnapshotV1 {
    RepositorySnapshotV1 {
        schema_id: repo_protocol::REPOSITORY_SNAPSHOT_SCHEMA_ID.to_string(),
        kind: match identity.kind {
            RepositorySnapshotKind::CommittedHead => RepositorySnapshotKindV1::CommittedHead,
            RepositorySnapshotKind::CommittedRange => RepositorySnapshotKindV1::CommittedRange,
        },
        root_identity: identity.root_identity.clone(),
        object_format: identity.object_format.as_str().to_string(),
        head: resolved_revision_v1_from_identity(&identity.head),
        base: identity
            .base
            .as_ref()
            .map(resolved_revision_v1_from_identity),
        merge_base: identity.merge_base.clone(),
        dirty_state: identity.dirty_state.as_str().to_string(),
        selected_paths: identity
            .selected_paths
            .iter()
            .map(selected_path_v1_from_identity)
            .collect(),
        selected_source_closure: identity.selected_source_closure.clone(),
        limitations: identity.limitations.clone(),
    }
}

fn resolved_revision_v1_from_identity(identity: &ResolvedRevisionIdentity) -> ResolvedRevisionV1 {
    ResolvedRevisionV1 {
        requested: identity.requested.clone(),
        commit: identity.commit.clone(),
        tree: identity.tree.clone(),
    }
}

fn selected_path_v1_from_identity(identity: &SelectedPathIdentity) -> SelectedPathIdentityV1 {
    SelectedPathIdentityV1 {
        path: identity.path.clone(),
        present: identity.present,
        blob_oid: identity.blob_oid.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::revision_identity::{
        RepositoryDirtyState, RepositorySnapshotIdentity, RepositorySnapshotKind,
        ResolvedRevisionIdentity,
    };

    #[test]
    fn repository_snapshot_v1_preserves_semantic_fields() {
        let identity = RepositorySnapshotIdentity {
            schema: crate::revision_identity::REPOSITORY_SNAPSHOT_SCHEMA,
            kind: RepositorySnapshotKind::CommittedHead,
            root_identity: "sha256:v1:fixture".to_string(),
            object_format: RepositoryObjectFormat::Sha1,
            head: ResolvedRevisionIdentity {
                requested: "HEAD".to_string(),
                commit: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
                tree: "tttttttttttttttttttttttttttttttttttttttt".to_string(),
            },
            base: None,
            merge_base: None,
            dirty_state: RepositoryDirtyState::NotProbed,
            selected_paths: Vec::new(),
            selected_source_closure: "sha256:v1:empty".to_string(),
            limitations: vec!["fixture".to_string()],
        };
        let transport = repository_snapshot_v1_from_identity(&identity);
        assert_eq!(transport.root_identity, identity.root_identity);
        assert_eq!(transport.head.commit, identity.head.commit);
        assert_eq!(transport.dirty_state, "not_probed");
        assert_eq!(transport.limitations, identity.limitations);
    }
}
