//! Typed computational controls; authenticated production-path controls live
//! in test-qualify-release-freeze.py and run through the actual binary.

use super::*;
use allow_report::{
    ArtifactTransferFileV1, ArtifactTransferInitV1, MergeMethodV1, MergedStateV1,
    PostMergeQualificationInitV1, ReviewedContextV1,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> CargoAllowResult<(SubjectIdentity, ReadbackInput)> {
    let subject = super::super::tests::subject();
    let mut reviewed_subject = super::super::tests::subject();
    reviewed_subject.commit = "b".repeat(40);
    let selection = super::super::tests::selection();
    let reading = allow_report::ObservationReadingV1 {
        freshness: allow_report::ObservationFreshnessV1::ProviderUnavailable,
        detail: "explicit synthetic registry hold".to_string(),
    };
    let graph = super::super::build_evidence_graph(
        &reviewed_subject,
        &selection,
        &[],
        &[],
        None,
        (allow_report::FinalEvidenceNodeResultV1::NotProven, &reading),
    );
    let graph_digest = final_evidence_graph_digest(&graph)
        .map_err(|error| instrument(format!("fixture graph digest: {error}")))?;
    let qualification = CargoAllowPostMergeQualificationV1::new(PostMergeQualificationInitV1 {
        qualification_id: "synthetic-qualified-tree".to_string(),
        reviewed: ReviewedContextV1 {
            base_sha: "a".repeat(40),
            head_sha: reviewed_subject.commit.clone(),
            merge_base_sha: "a".repeat(40),
            tree_sha: subject.tree.clone(),
        },
        merged: MergedStateV1 {
            pr_number: 1,
            merge_commit_sha: subject.commit.clone(),
            merge_tree_sha: subject.tree.clone(),
            current_main_commit_sha: subject.commit.clone(),
            current_main_tree_sha: subject.tree.clone(),
            merge_method: MergeMethodV1::MergeCommit,
            merge_parents: vec!["a".repeat(40), reviewed_subject.commit],
        },
        changed_files: Vec::new(),
        semantic_owners: vec!["#2501".to_string()],
        premerge_evidence_digest: graph_digest,
        preserved_evidence_nodes: graph.required_node_ids.clone(),
        invalidated_evidence_nodes: Vec::new(),
        required_rerun_set: Vec::new(),
        created_at_utc: "2026-10-10T10:00:00Z".to_string(),
    });
    let identity = ProducerIdentityV1 {
        repository: REPOSITORY.to_string(),
        workflow_path: ".github/workflows/release.yml".to_string(),
        git_ref: "refs/heads/main".to_string(),
        run_id: 101,
        run_attempt: 1,
        job_id: "303".to_string(),
        commit_sha: subject.commit.clone(),
        tree_sha: subject.tree.clone(),
        release_version: subject.version.clone(),
        tool_name: "cargo-allow".to_string(),
        schema_id: "cargo-allow.final-evidence-graph.v1".to_string(),
        producer_generation: 1,
    };
    let members = ["first", "second"]
        .into_iter()
        .map(|id| MemberReadback {
            logical_id: id.to_string(),
            role: "EvidenceGraph".to_string(),
            path: format!("{id}.json"),
            bytes: b"same".to_vec(),
        })
        .collect::<Vec<_>>();
    let envelope = CargoAllowReleaseArtifactTransferV1::new(ArtifactTransferInitV1 {
        transfer_id: "original-505".to_string(),
        role: "EvidenceBundle".to_string(),
        stable_artifact_id: "505".to_string(),
        producer: identity.clone(),
        provider_id: "github-actions-artifact".to_string(),
        provider_artifact_name: "original".to_string(),
        files: members
            .iter()
            .map(|member| ArtifactTransferFileV1 {
                path: member.path.clone(),
                size_bytes: member.bytes.len() as u64,
                sha256: sha256_v1_bytes(&member.bytes).replacen("sha256:v1:", "sha256:", 1),
            })
            .collect(),
        semantic_payload_digest: None,
        trust_class: TrustClassV1::ManualDispatch,
        untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
        created_at_utc: "2026-10-10T10:00:00Z".to_string(),
    });
    Ok((
        subject,
        ReadbackInput {
            observed_at_utc: "2026-10-10T10:01:00Z".to_string(),
            authorization_window_end_utc: "2026-10-10T11:00:00Z".to_string(),
            qualification,
            reviewed_evidence_graph: graph,
            artifacts: vec![ArtifactReadback {
                transfer: envelope,
                expected_producer: identity,
                created_at_utc: "2026-10-10T10:00:00Z".to_string(),
                retention_expiry_utc: "2026-10-11T10:00:00Z".to_string(),
                members,
            }],
        },
    ))
}

#[test]
fn actual_typed_qualification_and_singleton_custody_keep_original_envelope() -> TestResult {
    let (subject, input) = fixture()?;
    input.validate(&subject)?;
    let original = input.artifacts.first().ok_or("fixture artifact absent")?;
    let before = serde_json::to_vec(&original.transfer)?;
    let (custody, retained) = input.custody(&subject, "selected-custody")?;
    if custody.evaluate_custody(&subject.commit, &subject.version, &input.observed_at_utc)
        != CustodyDispositionV1::Complete
        || retained.len() != 2
        || custody.items.len() != 2
        || custody.items.iter().any(|item| item.files.len() != 1)
        || custody
            .items
            .iter()
            .map(|item| &item.storage_locator)
            .collect::<BTreeSet<_>>()
            .len()
            != 2
        || before != serde_json::to_vec(&original.transfer)?
    {
        return Err(
            "member mapping changed original envelope identity or lost exact custody".into(),
        );
    }
    Ok(())
}

#[test]
fn second_member_and_context_controls_cannot_hide_behind_registry_hold() -> TestResult {
    for change in 0..11 {
        let (subject, mut input) = fixture()?;
        let artifact = input
            .artifacts
            .first_mut()
            .ok_or("fixture artifact absent")?;
        match change {
            0 => {
                artifact.members.pop();
            }
            1..=3 => {
                let second = artifact
                    .members
                    .get_mut(1)
                    .ok_or("fixture second member absent")?;
                match change {
                    1 => second.bytes.push(0),
                    2 => second.path = "first.json".to_string(),
                    _ => second.logical_id = "first".to_string(),
                }
            }
            4 => artifact.transfer.producer.job_id = "304".to_string(),
            5 => artifact.retention_expiry_utc = input.observed_at_utc.clone(),
            6 => input
                .qualification
                .required_rerun_set
                .push("release-rehearsal".to_string()),
            7 => input.qualification.preserved_evidence_nodes.clear(),
            8 => input.qualification.reviewed.tree_sha = "d".repeat(40),
            9 => input.qualification.merged.current_main_commit_sha = "d".repeat(40),
            _ => input.authorization_window_end_utc.clear(),
        }
        if input.validate(&subject).is_ok() {
            return Err(format!("qualification/custody control {change} was admitted").into());
        }
    }
    Ok(())
}

#[test]
fn unknown_observations_have_no_boolean_readiness_and_roundtrip_blocking_rows() -> TestResult {
    let subject = super::super::tests::subject();
    let result = super::super::readiness_decision_inputs_observed(
        &subject,
        &super::super::tests::selection(),
        &[],
        None,
        None,
    );
    let rows = result
        .err()
        .ok_or("missing observations produced full readiness")?;
    let bytes = serde_json::to_vec(&rows)?;
    let actual: Vec<allow_report::FinalReadinessRowV1> = serde_json::from_slice(&bytes)?;
    let ids = actual
        .iter()
        .filter_map(|row| row.evidence_id.as_deref())
        .collect::<BTreeSet<_>>();
    if rows != actual
        || ids
            != BTreeSet::from([
                "post-merge-qualification",
                "custody-readback",
                "evaluation-clock",
                "authorization-window",
            ])
        || actual
            .iter()
            .any(|row| row.message.contains("main moved") || row.message.contains("expired"))
    {
        return Err("unknown facts acquired a factual Boolean projection".into());
    }
    Ok(())
}

#[test]
fn clock_and_window_are_canonical_not_lexical_lookalikes() -> TestResult {
    for invalid in [
        "",
        "2026-02-30T00:00:00Z",
        "2026-10-10T24:00:00Z",
        "2026-10-10T00:00:60Z",
        "2026-10-10T00:00:00+00:00",
        "2026-10-10T00:00:00.0Z",
        "2026-10-10T0a:00:00Z",
    ] {
        if utc(invalid).is_ok() {
            return Err(format!("malformed UTC admitted: {invalid:?}").into());
        }
    }
    utc("2024-02-29T23:59:59Z")?;
    Ok(())
}
