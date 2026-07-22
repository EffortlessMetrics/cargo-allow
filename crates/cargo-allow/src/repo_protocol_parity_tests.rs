use crate::repo_protocol_adapter::repository_snapshot_v1_from_identity;
use allow_diff::{
    RepositoryDirtyState, RepositoryObjectFormat, RepositorySnapshotIdentity,
    RepositorySnapshotKind, ResolvedRevisionIdentity,
};
use repo_protocol::stable_digest_json;
use std::path::PathBuf;

#[test]
fn repo_protocol_snapshot_parity_with_allow_diff_identity() -> Result<(), String> {
    let identity = RepositorySnapshotIdentity {
        schema: allow_diff::REPOSITORY_SNAPSHOT_SCHEMA,
        kind: RepositorySnapshotKind::CommittedHead,
        root_identity: "sha256:v1:parity-fixture".to_string(),
        object_format: RepositoryObjectFormat::Sha1,
        head: ResolvedRevisionIdentity {
            requested: "HEAD".to_string(),
            commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            tree: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        },
        base: None,
        merge_base: None,
        dirty_state: RepositoryDirtyState::NotProbed,
        selected_paths: Vec::new(),
        selected_source_closure: "sha256:v1:empty-closure".to_string(),
        limitations: vec!["parity-fixture".to_string()],
    };

    let transport = repository_snapshot_v1_from_identity(&identity);
    if transport.root_identity != identity.root_identity {
        return Err("root identity drifted during adapter migration".to_string());
    }
    if transport.head.commit != identity.head.commit {
        return Err("head commit drifted during adapter migration".to_string());
    }

    let digest = stable_digest_json(&transport).map_err(|err| err.message().to_string())?;
    if !digest.starts_with("sha256:v1:") {
        return Err(format!("transport digest missing prefix: {digest}"));
    }

    let doc = repo_root().join("docs/architecture/repo-protocol.md");
    let doc_text = std::fs::read_to_string(&doc)
        .map_err(|err| format!("repo-protocol doc readable: {err}"))?;
    if !doc_text.contains("RepositorySnapshotV1") {
        return Err("human projection missing migrated envelope name".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
