use allow_report::{
    FINAL_EVIDENCE_EDGE_SCHEMA_ID, FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
    FINAL_EVIDENCE_GRAPH_SCHEMA_ID, FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
    FINAL_EVIDENCE_NODE_SCHEMA_ID, FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
    FinalEvidenceAuthorityScopeV1, FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1,
    FinalEvidenceEdgeV1, FinalEvidenceEvaluationResultV1, FinalEvidenceFindingKindV1,
    FinalEvidenceGraphModeV1, FinalEvidenceGraphV1, FinalEvidenceInvalidationDimensionV1,
    FinalEvidenceNodeClassV1, FinalEvidenceNodeResultV1, FinalEvidenceNodeV1,
    FinalEvidenceOriginV1, FinalEvidencePackageRoleV1, FinalEvidencePackageSubjectV1,
    FinalEvidenceProducerExpectationV1, FinalEvidenceProducerV1, FinalEvidenceReleaseIdentityV1,
    FinalEvidenceSelectedSubjectV1, FinalEvidenceSubjectBindingV1, evaluate_final_evidence_graph,
    render_final_evidence_evaluation_json, render_final_evidence_evaluation_markdown,
    render_final_evidence_graph_canonical_json,
};
use std::error::Error;
use std::io;

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const TREE: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn digest(seed: u64) -> String {
    format!("sha256:v1:{seed:064x}")
}

fn release_identity(version: &str, prerelease: bool) -> FinalEvidenceReleaseIdentityV1 {
    FinalEvidenceReleaseIdentityV1 {
        version: version.to_string(),
        tag: format!("v{version}"),
        github_prerelease: prerelease,
    }
}

fn package(
    logical_id: &str,
    package_name: &str,
    version: &str,
    role: FinalEvidencePackageRoleV1,
    seed: u64,
) -> FinalEvidencePackageSubjectV1 {
    let expected = digest(seed);
    FinalEvidencePackageSubjectV1 {
        logical_id: logical_id.to_string(),
        package_name: package_name.to_string(),
        version: version.to_string(),
        role,
        expected_digest: expected.clone(),
        observed_digest: Some(expected),
    }
}

fn package_rows() -> Vec<FinalEvidencePackageSubjectV1> {
    let mut rows = [
        "allow-core",
        "allow-policy",
        "allow-policy-legacy",
        "allow-inventory",
        "allow-files",
        "allow-rust",
        "allow-match",
        "allow-report",
        "allow-diff",
        "cargo-allow",
    ]
    .iter()
    .enumerate()
    .map(|(index, name)| {
        package(
            name,
            name,
            "0.2.0",
            FinalEvidencePackageRoleV1::UploadCandidate,
            100 + index as u64,
        )
    })
    .collect::<Vec<_>>();
    rows.extend([
        package(
            "repo-protocol",
            "effortless-repo-protocol",
            "0.1.0",
            FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            201,
        ),
        package(
            "repo-snapshot",
            "effortless-repo-snapshot",
            "0.1.0",
            FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            202,
        ),
        package(
            "repo-edit",
            "effortless-repo-edit",
            "0.1.0",
            FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            203,
        ),
    ]);
    rows
}

fn selected_subject() -> FinalEvidenceSelectedSubjectV1 {
    FinalEvidenceSelectedSubjectV1 {
        repository: REPOSITORY.to_string(),
        commit: COMMIT.to_string(),
        tree: TREE.to_string(),
        cargo_lock_digest: digest(1),
        topology_digest: digest(2),
        release_identity: release_identity("0.2.0", false),
        expected_upload_rows: 10,
        expected_shared_rows: 3,
        package_rows: package_rows(),
    }
}

fn final_binding(subject: &FinalEvidenceSelectedSubjectV1) -> FinalEvidenceSubjectBindingV1 {
    FinalEvidenceSubjectBindingV1 {
        repository: subject.repository.clone(),
        commit: Some(subject.commit.clone()),
        tree: Some(subject.tree.clone()),
        cargo_lock_digest: Some(subject.cargo_lock_digest.clone()),
        topology_digest: Some(subject.topology_digest.clone()),
        release_identity: Some(subject.release_identity.clone()),
        package_rows: subject.package_rows.clone(),
    }
}

fn historical_binding() -> FinalEvidenceSubjectBindingV1 {
    FinalEvidenceSubjectBindingV1 {
        repository: REPOSITORY.to_string(),
        commit: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        tree: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
        cargo_lock_digest: Some(digest(901)),
        topology_digest: Some(digest(902)),
        release_identity: Some(release_identity("0.2.0-rc.1", true)),
        package_rows: Vec::new(),
    }
}

fn producer(id: &str, generation: u32, seed: u64) -> FinalEvidenceProducerV1 {
    FinalEvidenceProducerV1 {
        producer_id: id.to_string(),
        tool: "cargo-allow".to_string(),
        generation,
        identity_digest: digest(seed),
        workflow_path: Some(".github/workflows/release.yml".to_string()),
        workflow_run_id: Some(42),
        workflow_attempt: Some(1),
        job: Some(id.to_string()),
    }
}

fn node(
    evidence_id: &str,
    class: FinalEvidenceNodeClassV1,
    origin: FinalEvidenceOriginV1,
    authority_scope: FinalEvidenceAuthorityScopeV1,
    subject: FinalEvidenceSubjectBindingV1,
    seed: u64,
) -> FinalEvidenceNodeV1 {
    let producer = producer(evidence_id, 1, seed + 10_000);
    let semantic_digest = digest(seed);
    let artifact_digest = digest(seed + 1_000);
    FinalEvidenceNodeV1 {
        schema_id: FINAL_EVIDENCE_NODE_SCHEMA_ID.to_string(),
        schema_version: FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
        evidence_id: evidence_id.to_string(),
        class,
        origin,
        authority_scope,
        required: true,
        producer_expectation: Some(FinalEvidenceProducerExpectationV1 {
            producer_id: producer.producer_id.clone(),
            generation: producer.generation,
            identity_digest: Some(producer.identity_digest.clone()),
        }),
        producer,
        subject,
        semantic_digest: semantic_digest.clone(),
        expected_semantic_digest: Some(semantic_digest),
        artifact_digest: Some(artifact_digest.clone()),
        expected_artifact_digest: Some(artifact_digest),
        result: FinalEvidenceNodeResultV1::Complete,
        currentness: FinalEvidenceCurrentnessV1::Current,
        invalidation_dimensions: vec![
            FinalEvidenceInvalidationDimensionV1::Source,
            FinalEvidenceInvalidationDimensionV1::ProducerGeneration,
        ],
        rerun_owner: Some(format!("owner:{evidence_id}")),
        limitations: Vec::new(),
        claim_boundary: format!("Exact bounded evidence for {evidence_id}."),
    }
}

fn edge(from: &str, to: &str, kind: FinalEvidenceEdgeKindV1) -> FinalEvidenceEdgeV1 {
    FinalEvidenceEdgeV1 {
        schema_id: FINAL_EVIDENCE_EDGE_SCHEMA_ID.to_string(),
        schema_version: FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
        from: from.to_string(),
        to: to.to_string(),
        kind,
        claim_boundary: format!("{from} supplies only the selected {kind:?} relationship to {to}."),
    }
}

fn base_graph() -> FinalEvidenceGraphV1 {
    let subject = selected_subject();
    let binding = final_binding(&subject);
    let nodes = vec![
        node(
            "package-archive",
            FinalEvidenceNodeClassV1::PackageArchive,
            FinalEvidenceOriginV1::CandidateBytes,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding.clone(),
            11,
        ),
        node(
            "installed-journey",
            FinalEvidenceNodeClassV1::InstalledJourney,
            FinalEvidenceOriginV1::WorkflowArtifact,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding.clone(),
            12,
        ),
        node(
            "package-docs",
            FinalEvidenceNodeClassV1::GeneratedProjection,
            FinalEvidenceOriginV1::GeneratedProjection,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding.clone(),
            13,
        ),
        node(
            "support-selection",
            FinalEvidenceNodeClassV1::SupportSelection,
            FinalEvidenceOriginV1::SourceAuthority,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding.clone(),
            14,
        ),
        node(
            "workflow-inventory",
            FinalEvidenceNodeClassV1::SourceAuthority,
            FinalEvidenceOriginV1::SourceAuthority,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding.clone(),
            15,
        ),
        node(
            "release-rehearsal",
            FinalEvidenceNodeClassV1::ReleaseRehearsal,
            FinalEvidenceOriginV1::WorkflowArtifact,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding.clone(),
            16,
        ),
        node(
            "registry-observation",
            FinalEvidenceNodeClassV1::RegistryObservation,
            FinalEvidenceOriginV1::ProviderObservation,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding.clone(),
            17,
        ),
        node(
            "rc1-incident",
            FinalEvidenceNodeClassV1::IncidentHandoff,
            FinalEvidenceOriginV1::HistoricalObservation,
            FinalEvidenceAuthorityScopeV1::HistoricalIncident,
            historical_binding(),
            18,
        ),
        node(
            "authorization-prerequisite",
            FinalEvidenceNodeClassV1::AuthorizationPrerequisite,
            FinalEvidenceOriginV1::GeneratedProjection,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            binding,
            19,
        ),
    ];
    let required_node_ids = nodes
        .iter()
        .map(|node| node.evidence_id.clone())
        .collect::<Vec<_>>();
    FinalEvidenceGraphV1 {
        schema_id: FINAL_EVIDENCE_GRAPH_SCHEMA_ID.to_string(),
        schema_version: FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
        mode: FinalEvidenceGraphModeV1::Production,
        repository: REPOSITORY.to_string(),
        selected_subject: subject,
        required_node_ids,
        nodes,
        edges: vec![
            edge(
                "package-archive",
                "installed-journey",
                FinalEvidenceEdgeKindV1::ProducedFrom,
            ),
            edge(
                "package-archive",
                "package-docs",
                FinalEvidenceEdgeKindV1::ProducedFrom,
            ),
            edge(
                "package-archive",
                "release-rehearsal",
                FinalEvidenceEdgeKindV1::RequiresExactEquality,
            ),
            edge(
                "support-selection",
                "installed-journey",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "support-selection",
                "package-docs",
                FinalEvidenceEdgeKindV1::Projects,
            ),
            edge(
                "workflow-inventory",
                "release-rehearsal",
                FinalEvidenceEdgeKindV1::ProducedFrom,
            ),
            edge(
                "registry-observation",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "release-rehearsal",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "installed-journey",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "package-docs",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "support-selection",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "workflow-inventory",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "rc1-incident",
                "installed-journey",
                FinalEvidenceEdgeKindV1::SupportsOnly,
            ),
            edge(
                "rc1-incident",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::ExcludesAsAuthority,
            ),
        ],
        limitations: vec![
            "The fixture supplies retained evidence; it performs no release operation.".to_string(),
        ],
        claim_boundary: "Exact final-release evidence composition fixture.".to_string(),
    }
}

fn node_mut<'a>(
    graph: &'a mut FinalEvidenceGraphV1,
    evidence_id: &str,
) -> Result<&'a mut FinalEvidenceNodeV1, io::Error> {
    graph
        .nodes
        .iter_mut()
        .find(|node| node.evidence_id == evidence_id)
        .ok_or_else(|| io::Error::other(format!("missing fixture node {evidence_id}")))
}

#[test]
fn final_evidence_graph_is_complete_and_canonical() -> Result<(), Box<dyn Error>> {
    let graph = base_graph();
    let evaluation = evaluate_final_evidence_graph(&graph);
    assert_eq!(evaluation.result, FinalEvidenceEvaluationResultV1::Complete);
    assert!(evaluation.findings.is_empty(), "{evaluation:#?}");
    assert!(evaluation.rerun_roots.is_empty());

    let mut reordered = graph.clone();
    reordered.nodes.reverse();
    reordered.edges.reverse();
    reordered.selected_subject.package_rows.reverse();
    reordered.required_node_ids.reverse();

    let reordered_evaluation = evaluate_final_evidence_graph(&reordered);
    assert_eq!(evaluation.graph_digest, reordered_evaluation.graph_digest);
    assert_eq!(
        render_final_evidence_graph_canonical_json(&graph)?,
        render_final_evidence_graph_canonical_json(&reordered)?
    );
    assert!(render_final_evidence_evaluation_json(&evaluation)?.contains("graph_digest"));
    assert!(render_final_evidence_evaluation_markdown(&evaluation).contains("No findings"));
    Ok(())
}

#[test]
fn final_evidence_graph_invalidation_propagates_and_names_the_smallest_root()
-> Result<(), Box<dyn Error>> {
    let mut graph = base_graph();
    let package = node_mut(&mut graph, "package-archive")?;
    package.semantic_digest = digest(88_888);

    let evaluation = evaluate_final_evidence_graph(&graph);
    assert_eq!(evaluation.result, FinalEvidenceEvaluationResultV1::Conflict);
    assert_eq!(evaluation.rerun_roots, vec!["package-archive"]);
    assert_eq!(evaluation.rerun_owners, vec!["owner:package-archive"]);

    for dependent in [
        "installed-journey",
        "package-docs",
        "release-rehearsal",
        "authorization-prerequisite",
    ] {
        let row = evaluation
            .node_dispositions
            .iter()
            .find(|row| row.evidence_id == dependent)
            .ok_or_else(|| io::Error::other(format!("missing disposition {dependent}")))?;
        assert!(row.transitively_stale, "{dependent}: {row:#?}");
        assert!(row.root_causes.contains(&"package-archive".to_string()));
    }
    Ok(())
}

#[test]
fn final_evidence_graph_preserves_producer_provider_and_instrument_failures()
-> Result<(), Box<dyn Error>> {
    let mut producer_mismatch = base_graph();
    node_mut(&mut producer_mismatch, "package-docs")?
        .producer
        .generation = 2;
    let evaluation = evaluate_final_evidence_graph(&producer_mismatch);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::MalformedGraph
    );
    assert!(evaluation.findings.iter().any(|finding| {
        finding.kind == FinalEvidenceFindingKindV1::InvalidProducer
            && finding.evidence_id.as_deref() == Some("package-docs")
    }));

    let mut provider_unavailable = base_graph();
    let registry = node_mut(&mut provider_unavailable, "registry-observation")?;
    registry.result = FinalEvidenceNodeResultV1::ProviderUnavailable;
    registry.currentness = FinalEvidenceCurrentnessV1::ProviderUnavailable;
    let evaluation = evaluate_final_evidence_graph(&provider_unavailable);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::ProviderUnavailable
    );

    let mut instrument_failure = base_graph();
    let rehearsal = node_mut(&mut instrument_failure, "release-rehearsal")?;
    rehearsal.result = FinalEvidenceNodeResultV1::InstrumentFailure;
    rehearsal.currentness = FinalEvidenceCurrentnessV1::InstrumentFailure;
    let evaluation = evaluate_final_evidence_graph(&instrument_failure);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::InstrumentFailure
    );
    Ok(())
}

#[test]
fn final_evidence_graph_rejects_rc1_as_final_authority() -> Result<(), Box<dyn Error>> {
    let graph = base_graph();
    assert_eq!(
        evaluate_final_evidence_graph(&graph).result,
        FinalEvidenceEvaluationResultV1::Complete
    );

    let mut hostile = graph;
    let exclusion = hostile
        .edges
        .iter_mut()
        .find(|edge| edge.from == "rc1-incident" && edge.to == "authorization-prerequisite")
        .ok_or_else(|| io::Error::other("missing RC exclusion fixture edge"))?;
    exclusion.kind = FinalEvidenceEdgeKindV1::RequiresExactEquality;

    let evaluation = evaluate_final_evidence_graph(&hostile);
    assert_eq!(evaluation.result, FinalEvidenceEvaluationResultV1::Conflict);
    assert!(evaluation.findings.iter().any(|finding| {
        finding.kind == FinalEvidenceFindingKindV1::InvalidAuthorityUse
            && finding
                .edge
                .as_deref()
                .is_some_and(|edge| edge.contains("rc1-incident"))
    }));
    Ok(())
}

#[test]
fn final_evidence_graph_rejects_copied_registry_truth_and_graph_corruption()
-> Result<(), Box<dyn Error>> {
    let mut copied_registry = base_graph();
    node_mut(&mut copied_registry, "registry-observation")?.origin =
        FinalEvidenceOriginV1::CandidateBytes;
    let evaluation = evaluate_final_evidence_graph(&copied_registry);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::MalformedGraph
    );
    assert!(evaluation.findings.iter().any(|finding| {
        finding.kind == FinalEvidenceFindingKindV1::InvalidNodeOrigin
            && finding.evidence_id.as_deref() == Some("registry-observation")
    }));

    let mut cycle = base_graph();
    cycle.edges.push(edge(
        "authorization-prerequisite",
        "package-archive",
        FinalEvidenceEdgeKindV1::RequiresCurrent,
    ));
    let evaluation = evaluate_final_evidence_graph(&cycle);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::MalformedGraph
    );
    assert!(
        evaluation
            .findings
            .iter()
            .any(|finding| finding.kind == FinalEvidenceFindingKindV1::DependencyCycle)
    );

    let mut missing = base_graph();
    missing
        .nodes
        .retain(|node| node.evidence_id != "package-docs");
    let evaluation = evaluate_final_evidence_graph(&missing);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::MalformedGraph
    );
    assert!(evaluation.findings.iter().any(|finding| {
        finding.kind == FinalEvidenceFindingKindV1::MissingRequiredNode
            && finding.evidence_id.as_deref() == Some("package-docs")
    }));
    Ok(())
}

#[test]
fn final_evidence_graph_requires_a_root_requalification_owner() -> Result<(), Box<dyn Error>> {
    let mut graph = base_graph();
    let package = node_mut(&mut graph, "package-archive")?;
    package.currentness = FinalEvidenceCurrentnessV1::Stale;
    package.rerun_owner = None;

    let evaluation = evaluate_final_evidence_graph(&graph);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::MalformedGraph
    );
    assert!(evaluation.findings.iter().any(|finding| {
        finding.kind == FinalEvidenceFindingKindV1::MissingRerunOwner
            && finding.evidence_id.as_deref() == Some("package-archive")
    }));
    Ok(())
}

#[test]
fn final_evidence_graph_ignores_malformed_edges_for_dependency_analysis() {
    let mut graph = base_graph();
    graph.edges.push(FinalEvidenceEdgeV1 {
        schema_id: FINAL_EVIDENCE_EDGE_SCHEMA_ID.to_string(),
        schema_version: FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
        from: String::new(),
        to: "package-archive".to_string(),
        kind: FinalEvidenceEdgeKindV1::RequiresCurrent,
        claim_boundary: "malformed edge must not become a dependency".to_string(),
    });

    let evaluation = evaluate_final_evidence_graph(&graph);
    assert_eq!(
        evaluation.result,
        FinalEvidenceEvaluationResultV1::MalformedGraph
    );
    assert!(
        evaluation
            .findings
            .iter()
            .any(|finding| finding.kind == FinalEvidenceFindingKindV1::InvalidSchema)
    );
    assert!(evaluation.rerun_roots.is_empty());
}

#[test]
fn final_evidence_graph_markdown_escapes_untrusted_values() -> Result<(), Box<dyn Error>> {
    let mut graph = base_graph();
    node_mut(&mut graph, "package-archive")?.evidence_id = "archive|`<x>".to_string();
    let evaluation = evaluate_final_evidence_graph(&graph);
    let markdown = render_final_evidence_evaluation_markdown(&evaluation);

    assert!(markdown.contains("archive\\|\\`<x>"));
    assert!(!markdown.contains("| `archive|`"));
    Ok(())
}
