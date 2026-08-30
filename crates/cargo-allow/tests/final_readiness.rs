//! Integration controls for the #3929 final pre-freeze readiness aggregate.
//!
//! Each control adapts one required negative from the issue: exact evidence
//! results decide, issue/CI narration never substitutes, and every non-ready
//! verdict names an exact owner and next action.

use allow_report::{
    CargoAllowFinalReadinessV1, FINAL_EVIDENCE_EDGE_SCHEMA_ID, FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
    FINAL_EVIDENCE_GRAPH_SCHEMA_ID, FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
    FINAL_EVIDENCE_NODE_SCHEMA_ID, FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
    FinalEvidenceAuthorityScopeV1, FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1,
    FinalEvidenceEdgeV1, FinalEvidenceGraphModeV1, FinalEvidenceGraphV1,
    FinalEvidenceInvalidationDimensionV1, FinalEvidenceNodeClassV1, FinalEvidenceNodeResultV1,
    FinalEvidenceNodeV1, FinalEvidenceOriginV1, FinalEvidencePackageRoleV1,
    FinalEvidencePackageSubjectV1, FinalEvidenceProducerV1, FinalEvidenceReleaseIdentityV1,
    FinalEvidenceSelectedSubjectV1, FinalEvidenceSubjectBindingV1, FinalReadinessClaimNarrowingV1,
    FinalReadinessCustodyPostureV1, FinalReadinessDecisionInputsV1, FinalReadinessDecisionStateV1,
    FinalReadinessPostMergePostureV1, FinalReadinessQualificationPostureV1,
    FinalReadinessRootDecisionV1, FinalReadinessRowKindV1, FinalReadinessRowV1,
    FinalReadinessSupportedLimitationV1, FinalReadinessVerdictV1, aggregate_final_readiness,
    render_final_readiness_json, render_final_readiness_markdown,
};

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const TREE: &str = "fedcba9876543210fedcba9876543210fedcba98";
const OTHER_TREE: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c";

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
    FinalEvidencePackageSubjectV1 {
        logical_id: logical_id.to_string(),
        package_name: package_name.to_string(),
        version: version.to_string(),
        role,
        expected_digest: digest(seed),
        observed_digest: Some(digest(seed)),
    }
}

fn package_rows() -> Vec<FinalEvidencePackageSubjectV1> {
    let upload_names = [
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
    ];
    let mut rows = upload_names
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
    rows.push(package(
        "repo-edit",
        "effortless-repo-edit",
        "0.1.0",
        FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
        201,
    ));
    rows.push(package(
        "repo-protocol",
        "effortless-repo-protocol",
        "0.1.0",
        FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
        202,
    ));
    rows.push(package(
        "repo-snapshot",
        "effortless-repo-snapshot",
        "0.1.0",
        FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
        203,
    ));
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
        package_rows: Vec::new(),
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

fn producer(evidence_id: &str) -> FinalEvidenceProducerV1 {
    FinalEvidenceProducerV1 {
        producer_id: format!("producer:{evidence_id}"),
        tool: "cargo-allow".to_string(),
        generation: 1,
        identity_digest: digest(9_000),
        workflow_path: Some(".github/workflows/release.yml".to_string()),
        workflow_run_id: Some(42),
        workflow_attempt: Some(1),
        job: Some(evidence_id.to_string()),
    }
}

/// Fixture parameters for one evidence node, bundled so the builder stays
/// under the argument-count lint without any lint-exception attribute.
struct NodeFixture {
    evidence_id: &'static str,
    class: FinalEvidenceNodeClassV1,
    origin: FinalEvidenceOriginV1,
    authority_scope: FinalEvidenceAuthorityScopeV1,
    subject: FinalEvidenceSubjectBindingV1,
    required: bool,
    seed: u64,
}

fn node(fixture: NodeFixture) -> FinalEvidenceNodeV1 {
    FinalEvidenceNodeV1 {
        schema_id: FINAL_EVIDENCE_NODE_SCHEMA_ID.to_string(),
        schema_version: FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
        evidence_id: fixture.evidence_id.to_string(),
        class: fixture.class,
        origin: fixture.origin,
        authority_scope: fixture.authority_scope,
        required: fixture.required,
        producer: producer(fixture.evidence_id),
        producer_expectation: None,
        subject: fixture.subject,
        semantic_digest: digest(fixture.seed),
        expected_semantic_digest: Some(digest(fixture.seed)),
        artifact_digest: Some(digest(fixture.seed + 1_000)),
        expected_artifact_digest: Some(digest(fixture.seed + 1_000)),
        result: FinalEvidenceNodeResultV1::Complete,
        currentness: FinalEvidenceCurrentnessV1::Current,
        invalidation_dimensions: vec![
            FinalEvidenceInvalidationDimensionV1::Source,
            FinalEvidenceInvalidationDimensionV1::ProducerGeneration,
        ],
        rerun_owner: Some(format!("owner:{}", fixture.evidence_id)),
        limitations: Vec::new(),
        claim_boundary: format!("Exact bounded evidence for {}.", fixture.evidence_id),
    }
}

fn edge(from: &str, to: &str, kind: FinalEvidenceEdgeKindV1) -> FinalEvidenceEdgeV1 {
    FinalEvidenceEdgeV1 {
        schema_id: FINAL_EVIDENCE_EDGE_SCHEMA_ID.to_string(),
        schema_version: FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
        from: from.to_string(),
        to: to.to_string(),
        kind,
        claim_boundary: format!("{from} supplies the selected {kind:?} relationship to {to}."),
    }
}

fn base_graph() -> FinalEvidenceGraphV1 {
    let subject = selected_subject();
    let binding = final_binding(&subject);
    let nodes = vec![
        node(NodeFixture {
            evidence_id: "package-archive",
            class: FinalEvidenceNodeClassV1::PackageArchive,
            origin: FinalEvidenceOriginV1::CandidateBytes,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: binding.clone(),
            required: true,
            seed: 11,
        }),
        node(NodeFixture {
            evidence_id: "installed-journey",
            class: FinalEvidenceNodeClassV1::InstalledJourney,
            origin: FinalEvidenceOriginV1::WorkflowArtifact,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: binding.clone(),
            required: true,
            seed: 12,
        }),
        node(NodeFixture {
            evidence_id: "package-docs",
            class: FinalEvidenceNodeClassV1::GeneratedProjection,
            origin: FinalEvidenceOriginV1::GeneratedProjection,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: binding.clone(),
            required: true,
            seed: 13,
        }),
        node(NodeFixture {
            evidence_id: "platform-receipt",
            class: FinalEvidenceNodeClassV1::PlatformReceipt,
            origin: FinalEvidenceOriginV1::WorkflowArtifact,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: binding.clone(),
            required: true,
            seed: 14,
        }),
        node(NodeFixture {
            evidence_id: "support-selection",
            class: FinalEvidenceNodeClassV1::SupportSelection,
            origin: FinalEvidenceOriginV1::SourceAuthority,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: binding.clone(),
            required: true,
            seed: 15,
        }),
        node(NodeFixture {
            evidence_id: "registry-observation",
            class: FinalEvidenceNodeClassV1::RegistryObservation,
            origin: FinalEvidenceOriginV1::ProviderObservation,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: binding.clone(),
            required: true,
            seed: 16,
        }),
        node(NodeFixture {
            evidence_id: "authorization-prerequisite",
            class: FinalEvidenceNodeClassV1::AuthorizationPrerequisite,
            origin: FinalEvidenceOriginV1::GeneratedProjection,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: binding,
            required: true,
            seed: 17,
        }),
        node(NodeFixture {
            evidence_id: "external-adoption",
            class: FinalEvidenceNodeClassV1::InstalledJourney,
            origin: FinalEvidenceOriginV1::WorkflowArtifact,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            subject: final_binding(&selected_subject()),
            required: false,
            seed: 18,
        }),
        node(NodeFixture {
            evidence_id: "rc1-incident",
            class: FinalEvidenceNodeClassV1::IncidentHandoff,
            origin: FinalEvidenceOriginV1::HistoricalObservation,
            authority_scope: FinalEvidenceAuthorityScopeV1::HistoricalIncident,
            subject: historical_binding(),
            required: false,
            seed: 19,
        }),
    ];
    let required_node_ids = nodes
        .iter()
        .filter(|node| node.required)
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
                "platform-receipt",
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
                "registry-observation",
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
                "platform-receipt",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "package-archive",
                "authorization-prerequisite",
                FinalEvidenceEdgeKindV1::RequiresCurrent,
            ),
            edge(
                "external-adoption",
                "installed-journey",
                FinalEvidenceEdgeKindV1::SupportsOnly,
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
        limitations: Vec::new(),
        claim_boundary: "Exact final-release evidence fixture for readiness controls.".to_string(),
    }
}

fn inputs() -> FinalReadinessDecisionInputsV1 {
    FinalReadinessDecisionInputsV1 {
        graph_owner: "owner:release-campaign".to_string(),
        root_decisions: vec![
            FinalReadinessRootDecisionV1 {
                decision_id: "support:windows-tier".to_string(),
                owner: "owner:support".to_string(),
                state: FinalReadinessDecisionStateV1::Decided,
                required: true,
            },
            FinalReadinessRootDecisionV1 {
                decision_id: "release:denominator-10-3".to_string(),
                owner: "owner:release".to_string(),
                state: FinalReadinessDecisionStateV1::Decided,
                required: true,
            },
        ],
        supported_limitations: Vec::new(),
        permitted_claim_narrowings: Vec::new(),
        post_merge: FinalReadinessPostMergePostureV1 {
            merge_commit: "aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111".to_string(),
            merge_subject_current: true,
            qualification: FinalReadinessQualificationPostureV1::Current,
            owner: "owner:qualification".to_string(),
        },
        custody: FinalReadinessCustodyPostureV1 {
            replay_feasible: true,
            expires_before_authorization_window: false,
            owner: "owner:custody".to_string(),
        },
        remaining_reversible_work: vec!["candidate freeze (#2501)".to_string()],
        remaining_irreversible_operations: vec![
            "tag push".to_string(),
            "crates.io upload".to_string(),
        ],
    }
}

fn set_node(
    graph: &mut FinalEvidenceGraphV1,
    evidence_id: &str,
    mutate: impl FnOnce(&mut FinalEvidenceNodeV1),
) -> Result<(), String> {
    let node = graph
        .nodes
        .iter_mut()
        .find(|node| node.evidence_id == evidence_id)
        .ok_or_else(|| format!("missing fixture node {evidence_id}"))?;
    mutate(node);
    Ok(())
}

fn row_of_kind(
    readiness: &CargoAllowFinalReadinessV1,
    kind: FinalReadinessRowKindV1,
) -> Option<&FinalReadinessRowV1> {
    readiness.rows.iter().find(|row| row.kind == kind)
}

fn expect_verdict(
    readiness: &CargoAllowFinalReadinessV1,
    expected: FinalReadinessVerdictV1,
) -> Result<(), String> {
    if readiness.verdict != expected {
        return Err(format!(
            "expected verdict {expected:?}, got {:?} with rows {:#?}",
            readiness.verdict, readiness.rows
        ));
    }
    Ok(())
}

fn expect_all_rows_named(readiness: &CargoAllowFinalReadinessV1) -> Result<(), String> {
    for row in &readiness.rows {
        if row.owner.trim().is_empty() || row.next_action.trim().is_empty() {
            return Err(format!("row lacks an exact owner or next action: {row:?}"));
        }
    }
    Ok(())
}

#[test]
fn final_readiness_ready_for_freeze_positive_path() -> Result<(), String> {
    let readiness = aggregate_final_readiness(&base_graph(), &inputs());
    expect_verdict(&readiness, FinalReadinessVerdictV1::ReadyForFreeze)?;
    expect_all_rows_named(&readiness)?;
    if readiness.selected_upload_rows != 10 || readiness.selected_shared_rows != 3 {
        return Err("selected 10 + 3 denominator was not retained".to_string());
    }
    if readiness.required_evidence.len() != 7 {
        return Err(format!(
            "expected 7 required evidence rows, got {}",
            readiness.required_evidence.len()
        ));
    }
    if readiness.support_only_evidence_ids != vec!["rc1-incident".to_string()] {
        return Err("RC.1 support-only evidence was not retained".to_string());
    }
    if !readiness
        .remaining_irreversible_operations
        .contains(&"tag push".to_string())
    {
        return Err("remaining irreversible operations were not retained".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_false_complete_missing_receipt_blocks_despite_closed_issues()
-> Result<(), String> {
    let mut graph = base_graph();
    let required_id = "installed-journey".to_string();
    graph.nodes.retain(|node| node.evidence_id != required_id);
    let readiness = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&readiness, FinalReadinessVerdictV1::Incomplete)?;
    let row = row_of_kind(&readiness, FinalReadinessRowKindV1::MissingEvidence)
        .ok_or_else(|| "missing-evidence row is absent".to_string())?;
    if row.owner.trim().is_empty() {
        return Err("missing receipt row lacks an exact owner".to_string());
    }
    expect_all_rows_named(&readiness)
}

#[test]
fn final_readiness_false_complete_skipped_platform_row_blocks_ready() -> Result<(), String> {
    let mut graph = base_graph();
    set_node(&mut graph, "platform-receipt", |node| {
        node.result = FinalEvidenceNodeResultV1::Incomplete;
    })?;
    let readiness = aggregate_final_readiness(&graph, &inputs());
    if readiness.verdict == FinalReadinessVerdictV1::ReadyForFreeze {
        return Err("a skipped selected platform row cannot stay ready".to_string());
    }
    let row = readiness
        .rows
        .iter()
        .find(|row| row.evidence_id.as_deref() == Some("platform-receipt"))
        .ok_or_else(|| "platform row is absent from the rows".to_string())?;
    if row.kind != FinalReadinessRowKindV1::MissingEvidence || row.owner != "owner:platform-receipt"
    {
        return Err("platform row lost its kind or exact owner".to_string());
    }
    expect_all_rows_named(&readiness)
}

#[test]
fn final_readiness_claim_narrowing_never_becomes_proof() -> Result<(), String> {
    let mut graph = base_graph();
    set_node(&mut graph, "external-adoption", |node| {
        node.result = FinalEvidenceNodeResultV1::NotProven;
    })?;

    let unpermitted = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&unpermitted, FinalReadinessVerdictV1::NotProven)?;

    let mut permitted_inputs = inputs();
    permitted_inputs.permitted_claim_narrowings = vec![FinalReadinessClaimNarrowingV1 {
        evidence_id: "external-adoption".to_string(),
        permitted_by_decision: "support:external-adoption".to_string(),
        owner: "owner:support".to_string(),
    }];
    let permitted = aggregate_final_readiness(&graph, &permitted_inputs);
    expect_verdict(&permitted, FinalReadinessVerdictV1::ReadyForFreeze)?;
    let row = row_of_kind(&permitted, FinalReadinessRowKindV1::ClaimNarrowed)
        .ok_or_else(|| "claim-narrowed row is absent".to_string())?;
    if row.owner != "owner:support"
        || !row.next_action.contains("out of proof narration")
        || !row.message.contains("never becomes proof")
    {
        return Err("narrowed claim leaked into proof narration".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_rc1_receipt_cannot_satisfy_the_final_package_row() -> Result<(), String> {
    let mut graph = base_graph();
    graph.nodes.push(node(NodeFixture {
        evidence_id: "rc1-package-receipt",
        class: FinalEvidenceNodeClassV1::PackageArchive,
        origin: FinalEvidenceOriginV1::CandidateBytes,
        authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
        subject: historical_binding(),
        required: true,
        seed: 21,
    }));
    graph.edges.push(edge(
        "rc1-package-receipt",
        "authorization-prerequisite",
        FinalEvidenceEdgeKindV1::RequiresCurrent,
    ));
    let readiness = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&readiness, FinalReadinessVerdictV1::Mismatch)?;
    let row = row_of_kind(&readiness, FinalReadinessRowKindV1::Mismatch)
        .ok_or_else(|| "mismatch row is absent".to_string())?;
    if row.owner.trim().is_empty() {
        return Err("RC.1 authority row lacks an exact owner".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_receipt_from_another_tree_is_a_mismatch() -> Result<(), String> {
    let mut graph = base_graph();
    set_node(&mut graph, "package-docs", |node| {
        node.subject.tree = Some(OTHER_TREE.to_string());
    })?;
    let readiness = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&readiness, FinalReadinessVerdictV1::Mismatch)?;
    expect_all_rows_named(&readiness)
}

#[test]
fn final_readiness_expired_registry_observation_forces_stale() -> Result<(), String> {
    let mut graph = base_graph();
    set_node(&mut graph, "registry-observation", |node| {
        node.currentness = FinalEvidenceCurrentnessV1::Expired;
    })?;
    let readiness = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&readiness, FinalReadinessVerdictV1::Stale)?;
    if !readiness
        .rows
        .iter()
        .any(|row| row.owner == "owner:registry-observation")
    {
        return Err("stale registry row did not name its rerun owner".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_provider_unavailable_stays_distinct_from_ready() -> Result<(), String> {
    let mut graph = base_graph();
    graph.nodes.push(node(NodeFixture {
        evidence_id: "live-control",
        class: FinalEvidenceNodeClassV1::LiveControlObservation,
        origin: FinalEvidenceOriginV1::ProviderObservation,
        authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
        subject: final_binding(&selected_subject()),
        required: true,
        seed: 22,
    }));
    graph.edges.push(edge(
        "live-control",
        "authorization-prerequisite",
        FinalEvidenceEdgeKindV1::RequiresCurrent,
    ));
    set_node(&mut graph, "live-control", |node| {
        node.result = FinalEvidenceNodeResultV1::ProviderUnavailable;
    })?;
    let readiness = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&readiness, FinalReadinessVerdictV1::ProviderUnavailable)?;
    let row = row_of_kind(&readiness, FinalReadinessRowKindV1::ProviderUnavailable)
        .ok_or_else(|| "provider row is absent".to_string())?;
    if row.owner.trim().is_empty() {
        return Err("provider row lacks an exact owner".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_inferred_root_decision_is_forbidden() -> Result<(), String> {
    let mut decision_inputs = inputs();
    for decision in &mut decision_inputs.root_decisions {
        if decision.decision_id == "support:windows-tier" {
            decision.state = FinalReadinessDecisionStateV1::Missing;
        }
    }
    let readiness = aggregate_final_readiness(&base_graph(), &decision_inputs);
    expect_verdict(&readiness, FinalReadinessVerdictV1::NeedsDecision)?;
    let row = row_of_kind(&readiness, FinalReadinessRowKindV1::DecisionRequired)
        .ok_or_else(|| "decision row is absent".to_string())?;
    if row.owner != "owner:support" || !row.next_action.contains("cannot be inferred") {
        return Err("decision row lost its owner or inference ban".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_supported_limitation_requires_projection_and_owner() -> Result<(), String> {
    let mut decision_inputs = inputs();
    decision_inputs.supported_limitations = vec![FinalReadinessSupportedLimitationV1 {
        limitation_id: "limitation:windows-symlink".to_string(),
        user_facing_projection: None,
        owner: None,
    }];
    let readiness = aggregate_final_readiness(&base_graph(), &decision_inputs);
    expect_verdict(&readiness, FinalReadinessVerdictV1::Unsupported)?;
    if !readiness.supported_limitation_ids.is_empty() {
        return Err("invalid limitation was admitted as supported".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_qualification_rerun_rejects_the_old_graph() -> Result<(), String> {
    let mut decision_inputs = inputs();
    decision_inputs.post_merge.qualification = FinalReadinessQualificationPostureV1::RequiresRerun;
    let readiness = aggregate_final_readiness(&base_graph(), &decision_inputs);
    expect_verdict(&readiness, FinalReadinessVerdictV1::Stale)?;
    if readiness.post_merge_qualification != FinalReadinessQualificationPostureV1::RequiresRerun {
        return Err("qualification posture was not retained".to_string());
    }
    expect_all_rows_named(&readiness)
}

#[test]
fn final_readiness_custody_expiring_before_window_is_named() -> Result<(), String> {
    let mut decision_inputs = inputs();
    decision_inputs.custody.expires_before_authorization_window = true;
    let readiness = aggregate_final_readiness(&base_graph(), &decision_inputs);
    expect_verdict(&readiness, FinalReadinessVerdictV1::Stale)?;
    let row = row_of_kind(&readiness, FinalReadinessRowKindV1::CustodyExpiring)
        .ok_or_else(|| "custody-expiring row is absent".to_string())?;
    if row.owner != "owner:custody" {
        return Err("custody row lost its owner".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_aggregate_creates_no_release_state() -> Result<(), String> {
    let graph = base_graph();
    let first = aggregate_final_readiness(&graph, &inputs());
    let second = aggregate_final_readiness(&graph, &inputs());
    if first != second {
        return Err("aggregate is not deterministic".to_string());
    }
    let boundary = first.claim_boundary.to_lowercase();
    for forbidden in [
        "does not generate package bytes",
        "tag",
        "upload",
        "publish",
    ] {
        if !boundary.contains(forbidden) {
            return Err(format!("claim boundary omits `{forbidden}`"));
        }
    }
    let markdown = render_final_readiness_markdown(&first);
    if !markdown.contains("ready_for_freeze") {
        return Err("markdown projection lost the verdict".to_string());
    }
    let json = render_final_readiness_json(&first).map_err(|error| error.to_string())?;
    if !json.contains("\"verdict\"") {
        return Err("json projection lost the verdict".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_required_not_proven_rejects_a_narrowing() -> Result<(), String> {
    let mut graph = base_graph();
    set_node(&mut graph, "installed-journey", |node| {
        node.result = FinalEvidenceNodeResultV1::NotProven;
    })?;
    let mut decision_inputs = inputs();
    decision_inputs.permitted_claim_narrowings = vec![FinalReadinessClaimNarrowingV1 {
        evidence_id: "installed-journey".to_string(),
        permitted_by_decision: "support:journey-tier".to_string(),
        owner: "owner:support".to_string(),
    }];
    let readiness = aggregate_final_readiness(&graph, &decision_inputs);
    if readiness.verdict == FinalReadinessVerdictV1::ReadyForFreeze {
        return Err("a required NotProven row cannot be narrowed into ready".to_string());
    }
    let row = readiness
        .rows
        .iter()
        .find(|row| {
            row.kind == FinalReadinessRowKindV1::NotProven
                && row.evidence_id.as_deref() == Some("installed-journey")
        })
        .ok_or_else(|| "required not-proven row is absent".to_string())?;
    if !row.message.contains("inapplicable") || row.owner != "owner:installed-journey" {
        return Err("required row lost the rejected-narrowing note or owner".to_string());
    }
    Ok(())
}

#[test]
fn final_readiness_fixture_mode_graph_is_never_ready_for_freeze() -> Result<(), String> {
    let mut graph = base_graph();
    graph.mode = FinalEvidenceGraphModeV1::Fixture;
    let readiness = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&readiness, FinalReadinessVerdictV1::Mismatch)?;
    let row = row_of_kind(&readiness, FinalReadinessRowKindV1::Mismatch)
        .ok_or_else(|| "fixture-mode mismatch row is absent".to_string())?;
    if !row.message.contains("fixture") {
        return Err("mismatch row did not name the graph mode".to_string());
    }
    if row.owner != "owner:release-campaign" || !row.next_action.contains("production mode") {
        return Err("fixture-mode row lost its owner or next action".to_string());
    }
    expect_all_rows_named(&readiness)
}

#[test]
fn final_readiness_declared_limitations_require_supported_inputs() -> Result<(), String> {
    let mut graph = base_graph();
    graph.limitations = vec!["limitation:windows-symlink".to_string()];
    set_node(&mut graph, "platform-receipt", |node| {
        node.limitations = vec!["limitation:long-path-support".to_string()];
    })?;

    let blocked = aggregate_final_readiness(&graph, &inputs());
    expect_verdict(&blocked, FinalReadinessVerdictV1::Unsupported)?;
    for limitation in ["limitation:windows-symlink", "limitation:long-path-support"] {
        if !blocked.rows.iter().any(|row| {
            row.kind == FinalReadinessRowKindV1::Unsupported && row.message.contains(limitation)
        }) {
            return Err(format!("unsupported row did not name `{limitation}`"));
        }
    }
    if !blocked.supported_limitation_ids.is_empty() {
        return Err("undeclared-support limitations leaked into supported ids".to_string());
    }

    let mut supported_inputs = inputs();
    supported_inputs.supported_limitations = vec![
        FinalReadinessSupportedLimitationV1 {
            limitation_id: "limitation:windows-symlink".to_string(),
            user_facing_projection: Some("windows: symlinks require developer mode".to_string()),
            owner: Some("owner:support".to_string()),
        },
        FinalReadinessSupportedLimitationV1 {
            limitation_id: "limitation:long-path-support".to_string(),
            user_facing_projection: Some("windows: long paths require registry opt-in".to_string()),
            owner: Some("owner:support".to_string()),
        },
    ];
    let supported = aggregate_final_readiness(&graph, &supported_inputs);
    expect_verdict(&supported, FinalReadinessVerdictV1::ReadyForFreeze)?;
    if supported.supported_limitation_ids.len() != 2 {
        return Err("supported limitation ids were not retained".to_string());
    }
    Ok(())
}
