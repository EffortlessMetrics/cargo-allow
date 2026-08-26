use allow_report::{
    CargoAllowFinalEvidenceEdgeV1, CargoAllowFinalEvidenceGraphV1, CargoAllowFinalEvidenceNodeV1,
    EvidenceEdgeKindV1, EvidenceNodeClassV1, EvidenceResultClassV1, FinalEvidenceGraphInitV1,
};
use std::io;

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if !condition {
        return Err(io::Error::other(message));
    }
    Ok(())
}

fn build_release_evidence_graph() -> CargoAllowFinalEvidenceGraphV1 {
    let nodes = vec![
        CargoAllowFinalEvidenceNodeV1 {
            node_id: "src:git-main".to_string(),
            node_class: EvidenceNodeClassV1::SourceAuthority,
            subject_version: "0.2.0".to_string(),
            producer_id: "git".to_string(),
            digest: "sha256:commit-sha-001".to_string(),
            result_class: EvidenceResultClassV1::Complete,
            claim_boundary: vec!["source_tree".to_string()],
        },
        CargoAllowFinalEvidenceNodeV1 {
            node_id: "pkg:cargo-allow".to_string(),
            node_class: EvidenceNodeClassV1::PackageArchive,
            subject_version: "0.2.0".to_string(),
            producer_id: "cargo-package".to_string(),
            digest: "sha256:pkg-sha-001".to_string(),
            result_class: EvidenceResultClassV1::Complete,
            claim_boundary: vec!["crate_bytes".to_string()],
        },
        CargoAllowFinalEvidenceNodeV1 {
            node_id: "rcpt:rehearsal".to_string(),
            node_class: EvidenceNodeClassV1::ReleaseRehearsal,
            subject_version: "0.2.0".to_string(),
            producer_id: "release-rehearsal".to_string(),
            digest: "sha256:rehearsal-sha-001".to_string(),
            result_class: EvidenceResultClassV1::Complete,
            claim_boundary: vec!["dry_run_evidence".to_string()],
        },
        CargoAllowFinalEvidenceNodeV1 {
            node_id: "auth:prerequisite".to_string(),
            node_class: EvidenceNodeClassV1::AuthorizationPrerequisite,
            subject_version: "0.2.0".to_string(),
            producer_id: "operator".to_string(),
            digest: "sha256:auth-sha-001".to_string(),
            result_class: EvidenceResultClassV1::Complete,
            claim_boundary: vec!["release_authority".to_string()],
        },
    ];

    let edges = vec![
        CargoAllowFinalEvidenceEdgeV1 {
            from_node: "src:git-main".to_string(),
            to_node: "pkg:cargo-allow".to_string(),
            edge_kind: EvidenceEdgeKindV1::ProducedFrom,
        },
        CargoAllowFinalEvidenceEdgeV1 {
            from_node: "pkg:cargo-allow".to_string(),
            to_node: "rcpt:rehearsal".to_string(),
            edge_kind: EvidenceEdgeKindV1::RequiresCurrent,
        },
        CargoAllowFinalEvidenceEdgeV1 {
            from_node: "rcpt:rehearsal".to_string(),
            to_node: "auth:prerequisite".to_string(),
            edge_kind: EvidenceEdgeKindV1::RequiresCurrent,
        },
    ];

    CargoAllowFinalEvidenceGraphV1::new(FinalEvidenceGraphInitV1 {
        graph_id: "final-graph-0.2.0".to_string(),
        release_version: "0.2.0".to_string(),
        nodes,
        edges,
        created_at_utc: "2026-08-26T10:00:00Z".to_string(),
    })
}

#[test]
fn test_final_evidence_graph_clean_authorization_chain() -> Result<(), io::Error> {
    let graph = build_release_evidence_graph();
    let eval = graph.evaluate();

    require(
        eval.overall_result == EvidenceResultClassV1::Complete,
        "clean graph must evaluate as Complete",
    )?;
    require(
        eval.required_reruns.is_empty(),
        "clean graph requires zero reruns",
    )?;
    Ok(())
}

#[test]
fn test_final_evidence_graph_transitive_invalidation_to_authorization() -> Result<(), io::Error> {
    let mut graph = build_release_evidence_graph();
    if let Some(pkg) = graph.nodes.get_mut(1) {
        pkg.result_class = EvidenceResultClassV1::Stale;
    }

    let eval = graph.evaluate();

    require(
        eval.overall_result == EvidenceResultClassV1::Stale,
        "stale package must stale graph",
    )?;
    require(
        eval.invalidated_descendants
            .contains(&"rcpt:rehearsal".to_string()),
        "rcpt:rehearsal must be invalidated",
    )?;
    require(
        eval.invalidated_descendants
            .contains(&"auth:prerequisite".to_string()),
        "auth:prerequisite must be transitively invalidated",
    )?;
    Ok(())
}
