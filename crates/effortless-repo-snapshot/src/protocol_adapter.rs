use crate::revision_identity::{
    RepositorySnapshotIdentity, RepositorySnapshotKind, ResolvedRevisionIdentity,
    SelectedPathIdentity,
};
use effortless_repo_protocol::{
    RepositorySnapshotKindV1, RepositorySnapshotV1, ResolvedRevisionV1, SelectedPathIdentityV1,
};

pub fn repository_snapshot_v1(
    identity: &RepositorySnapshotIdentity,
) -> RepositorySnapshotV1 {
    RepositorySnapshotV1 {
        schema_id: effortless_repo_protocol::REPOSITORY_SNAPSHOT_SCHEMA_ID.to_string(),
        kind: match identity.kind {
            RepositorySnapshotKind::CommittedHead => RepositorySnapshotKindV1::CommittedHead,
            RepositorySnapshotKind::CommittedRange => RepositorySnapshotKindV1::CommittedRange,
        },
        root_identity: identity.root_identity.clone(),
        object_format: identity.object_format.as_str().to_string(),
        head: resolved_revision_v1(&identity.head),
        base: identity
            .base
            .as_ref()
            .map(resolved_revision_v1),
        merge_base: identity.merge_base.clone(),
        dirty_state: identity.dirty_state.as_str().to_string(),
        selected_paths: identity
            .selected_paths
            .iter()
            .map(selected_path_v1)
            .collect(),
        selected_source_closure: identity.selected_source_closure.clone(),
        limitations: identity.limitations.clone(),
    }
}

fn resolved_revision_v1(identity: &ResolvedRevisionIdentity) -> ResolvedRevisionV1 {
    ResolvedRevisionV1 {
        requested: identity.requested.clone(),
        commit: identity.commit.clone(),
        tree: identity.tree.clone(),
    }
}

fn selected_path_v1(identity: &SelectedPathIdentity) -> SelectedPathIdentityV1 {
    SelectedPathIdentityV1 {
        path: identity.path.clone(),
        present: identity.present,
        blob_oid: identity.blob_oid.clone(),
    }
}
