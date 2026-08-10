use crate::{
    ClaimBoundaryV1, RepositorySnapshotKindV1, RepositorySnapshotV1, ResolvedRevisionV1,
    ResultClassV1, canonical_json_bytes, stable_digest_json,
};

#[test]
fn canonical_serialization_is_deterministic() -> Result<(), String> {
    let snapshot = sample_snapshot();
    let first = canonical_json_bytes(&snapshot).map_err(|err| err.message().to_string())?;
    let second = canonical_json_bytes(&snapshot).map_err(|err| err.message().to_string())?;
    if first != second {
        return Err("canonical bytes changed across identical encodings".to_string());
    }
    let digest = stable_digest_json(&snapshot).map_err(|err| err.message().to_string())?;
    if !digest.starts_with("sha256:v1:") {
        return Err(format!("unexpected digest prefix: {digest}"));
    }
    Ok(())
}

#[test]
fn provider_payload_roundtrip_preserves_semantics() -> Result<(), String> {
    let snapshot = sample_snapshot();
    let envelope = crate::AnalysisReceiptEnvelopeV1::new(
        "cargo-allow",
        snapshot,
        ResultClassV1::Findings,
        "cargo-allow.provider-result.v1",
        serde_json::json!({ "finding_count": 3 }),
        ClaimBoundaryV1::new("fixture parity receipt"),
    );
    let json = serde_json::to_string(&envelope)
        .map_err(|err| format!("serialize analysis receipt: {err}"))?;
    let decoded: crate::AnalysisReceiptEnvelopeV1 = serde_json::from_str(&json)
        .map_err(|err| format!("deserialize analysis receipt: {err}"))?;
    if decoded.result_class != ResultClassV1::Findings {
        return Err("result class did not round-trip".to_string());
    }
    if decoded.provider_payload.get("finding_count") != Some(&serde_json::json!(3)) {
        return Err("provider payload flattened during round-trip".to_string());
    }
    Ok(())
}

#[test]
fn source_anchor_relocation_uses_root_identity_not_absolute_path() -> Result<(), String> {
    let anchor = crate::SourceAnchorV1::from_selected_path(
        "sha256:v1:fixture-root",
        crate::SourceIdentityV1 {
            path: "src/lib.rs".to_string(),
            present: true,
            blob_oid: Some("deadbeef".to_string()),
        },
    );
    let json =
        serde_json::to_string(&anchor).map_err(|err| format!("serialize source anchor: {err}"))?;
    if json.contains("C:\\") || json.contains("/Users/") {
        return Err("source anchor leaked absolute checkout path".to_string());
    }
    if !json.contains("sha256:v1:fixture-root") {
        return Err("source anchor missing portable root identity".to_string());
    }
    Ok(())
}

#[test]
fn partial_result_class_denies_clean_completion() {
    assert!(ResultClassV1::InstrumentFailure.denies_clean_completion());
    assert!(!ResultClassV1::Completed.denies_clean_completion());
}

#[test]
fn source_view_contract_roundtrips_and_preserves_base_head_identity() -> Result<(), String> {
    let mut view = crate::RepositorySourceViewV1::new(
        crate::RepositorySourceViewKindV1::BaseHead,
        "sha256:v1:root",
        crate::CompletenessV1::Partial,
    );
    view.base_identity = Some("sha256:v1:base".to_string());
    view.head_identity = Some("sha256:v1:head".to_string());
    view.entries.push(crate::SourceEntryV1 {
        path: "src/lib.rs".to_string(),
        present: true,
        content_digest: Some(crate::SourceContentDigestV1 {
            algorithm: "sha256:v1".to_string(),
            value: "abc".to_string(),
        }),
        executable: false,
    });
    view.validate()?;
    let json =
        serde_json::to_string(&view).map_err(|err| format!("serialize source view: {err}"))?;
    let decoded: crate::RepositorySourceViewV1 =
        serde_json::from_str(&json).map_err(|err| format!("deserialize source view: {err}"))?;
    if decoded != view {
        return Err("source-view contract changed during round-trip".to_string());
    }
    Ok(())
}

#[test]
fn source_view_contract_rejects_flattened_base_head_identity() {
    let view = crate::RepositorySourceViewV1::new(
        crate::RepositorySourceViewKindV1::BaseHead,
        "sha256:v1:root",
        crate::CompletenessV1::Complete,
    );
    assert!(view.validate().is_err());
}

#[test]
fn allow_diff_repository_snapshot_field_parity_fixture() -> Result<(), String> {
    let snapshot = sample_snapshot();
    if snapshot.head.commit != "cccccccccccccccccccccccccccccccccccccccc" {
        return Err("fixture head commit drifted".to_string());
    }
    if snapshot.dirty_state != "not_probed" {
        return Err("fixture dirty_state should mirror allow-diff not_probed".to_string());
    }
    Ok(())
}

fn sample_snapshot() -> RepositorySnapshotV1 {
    RepositorySnapshotV1 {
        schema_id: crate::REPOSITORY_SNAPSHOT_SCHEMA_ID.to_string(),
        kind: RepositorySnapshotKindV1::CommittedHead,
        root_identity: "sha256:v1:fixture-root".to_string(),
        object_format: "sha1".to_string(),
        head: ResolvedRevisionV1 {
            requested: "HEAD".to_string(),
            commit: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
            tree: "tttttttttttttttttttttttttttttttttttttttt".to_string(),
        },
        base: None,
        merge_base: None,
        dirty_state: "not_probed".to_string(),
        selected_paths: Vec::new(),
        selected_source_closure: "sha256:v1:empty-closure".to_string(),
        limitations: Vec::new(),
    }
}
