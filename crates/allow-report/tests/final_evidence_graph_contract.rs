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

fn sample_graph() -> CargoAllowFinalEvidenceGraphV1 {
    let nodes = vec![
        CargoAllowFinalEvidenceNodeV1 {
            node_id: "src:main".to_string(),
            node_class: EvidenceNodeClassV1::SourceAuthority,
            subject_version: "0.2.0".to_string(),
            producer_id: "git".to_string(),
            digest: "sha256:source-sha".to_string(),
            result_class: EvidenceResultClassV1::Complete,
            claim_boundary: vec!["source_tree".to_string()],
        },
        CargoAllowFinalEvidenceNodeV1 {
            node_id: "pkg:allow-core".to_string(),
            node_class: EvidenceNodeClassV1::PackageArchive,
            subject_version: "0.2.0".to_string(),
            producer_id: "cargo-package".to_string(),
            digest: "sha256:package-sha".to_string(),
            result_class: EvidenceResultClassV1::Complete,
            claim_boundary: vec!["package_bytes".to_string()],
        },
        CargoAllowFinalEvidenceNodeV1 {
            node_id: "rcpt:installed-journey".to_string(),
            node_class: EvidenceNodeClassV1::InstalledJourney,
            subject_version: "0.2.0".to_string(),
            producer_id: "cargo-install-smoke".to_string(),
            digest: "sha256:journey-sha".to_string(),
            result_class: EvidenceResultClassV1::Complete,
            claim_boundary: vec!["installed_journey".to_string()],
        },
    ];

    let edges = vec![
        CargoAllowFinalEvidenceEdgeV1 {
            from_node: "src:main".to_string(),
            to_node: "pkg:allow-core".to_string(),
            edge_kind: EvidenceEdgeKindV1::ProducedFrom,
        },
        CargoAllowFinalEvidenceEdgeV1 {
            from_node: "pkg:allow-core".to_string(),
            to_node: "rcpt:installed-journey".to_string(),
            edge_kind: EvidenceEdgeKindV1::RequiresCurrent,
        },
    ];

    CargoAllowFinalEvidenceGraphV1::new(FinalEvidenceGraphInitV1 {
        graph_id: "graph-001".to_string(),
        release_version: "0.2.0".to_string(),
        nodes,
        edges,
        created_at_utc: "2026-08-26T00:00:00Z".to_string(),
    })
}

#[test]
fn test_evidence_graph_complete_evaluation() -> Result<(), io::Error> {
    let graph = sample_graph();
    let eval = graph.evaluate();

    require(
        eval.overall_result == EvidenceResultClassV1::Complete,
        "clean graph must evaluate as Complete",
    )?;
    require(
        eval.stale_nodes.is_empty(),
        "clean graph must have no stale nodes",
    )?;
    require(
        eval.invalidated_descendants.is_empty(),
        "clean graph must have no invalidated descendants",
    )?;
    Ok(())
}

#[test]
fn test_evidence_graph_transitive_staleness_propagation() -> Result<(), io::Error> {
    let mut graph = sample_graph();
    if let Some(src) = graph.nodes.first_mut() {
        src.result_class = EvidenceResultClassV1::Stale;
    }

    let eval = graph.evaluate();

    require(
        eval.overall_result == EvidenceResultClassV1::Stale,
        "graph with stale root must evaluate as Stale",
    )?;
    require(
        eval.stale_nodes.contains(&"src:main".to_string()),
        "stale_nodes must contain src:main",
    )?;
    require(
        eval.invalidated_descendants
            .contains(&"pkg:allow-core".to_string()),
        "invalidated_descendants must contain direct child pkg:allow-core",
    )?;
    require(
        eval.invalidated_descendants
            .contains(&"rcpt:installed-journey".to_string()),
        "invalidated_descendants must contain transitive descendant rcpt:installed-journey",
    )?;
    Ok(())
}

#[test]
fn test_evidence_graph_rc_authority_exclusion() -> Result<(), io::Error> {
    let mut graph = sample_graph();
    if let Some(pkg) = graph.nodes.get_mut(1) {
        pkg.subject_version = "0.2.0-rc.1".to_string();
    }

    let eval = graph.evaluate();

    require(
        eval.overall_result == EvidenceResultClassV1::Conflict,
        "graph wiring RC.1 package as final authority must evaluate as Conflict",
    )?;
    Ok(())
}

#[test]
fn test_evidence_graph_cycle_detection() -> Result<(), io::Error> {
    let mut graph = sample_graph();
    graph.edges.push(CargoAllowFinalEvidenceEdgeV1 {
        from_node: "rcpt:installed-journey".to_string(),
        to_node: "src:main".to_string(),
        edge_kind: EvidenceEdgeKindV1::ProducedFrom,
    });

    let eval = graph.evaluate();

    require(
        eval.overall_result == EvidenceResultClassV1::Conflict,
        "cyclic graph must evaluate as Conflict",
    )?;
    Ok(())
}

#[test]
fn test_evidence_graph_serde_roundtrip() -> Result<(), io::Error> {
    let graph = sample_graph();
    let json = serde_json::to_string(&graph).map_err(io::Error::other)?;
    let parsed: CargoAllowFinalEvidenceGraphV1 =
        serde_json::from_str(&json).map_err(io::Error::other)?;

    require(parsed == graph, "deserialized graph must match original")?;
    Ok(())
}
