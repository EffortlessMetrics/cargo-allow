use super::super::release_identity_v1::{ReleaseChannelV1, ReleaseIdentityV1, ReleaseVersionV1};
use super::model::{
    FINAL_EVIDENCE_EDGE_SCHEMA_ID, FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
    FINAL_EVIDENCE_EVALUATION_SCHEMA_ID, FINAL_EVIDENCE_EVALUATION_SCHEMA_VERSION,
    FINAL_EVIDENCE_GRAPH_SCHEMA_ID, FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
    FINAL_EVIDENCE_NODE_SCHEMA_ID, FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
    FinalEvidenceAuthorityScopeV1, FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1,
    FinalEvidenceEdgeV1, FinalEvidenceEvaluationResultV1, FinalEvidenceFindingKindV1,
    FinalEvidenceFindingV1, FinalEvidenceGraphEvaluationV1, FinalEvidenceGraphModeV1,
    FinalEvidenceGraphV1, FinalEvidenceNodeClassV1, FinalEvidenceNodeDispositionV1,
    FinalEvidenceNodeResultV1, FinalEvidenceNodeV1, FinalEvidenceOriginV1,
    FinalEvidencePackageRoleV1, FinalEvidencePackageSubjectV1,
};
use super::render::final_evidence_graph_digest;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const CLAIM_BOUNDARY: &str = "This evaluation proves how the supplied exact release evidence composes, becomes non-current, and routes requalification. It does not produce the evidence, decide support, authorize release, or perform an external operation.";

/// Evaluate the supplied final-release evidence graph without performing I/O.
#[must_use]
pub fn evaluate_final_evidence_graph(
    graph: &FinalEvidenceGraphV1,
) -> FinalEvidenceGraphEvaluationV1 {
    let graph = graph.canonicalized();
    let graph_digest =
        final_evidence_graph_digest(&graph).unwrap_or_else(|error| format!("unavailable:{error}"));
    let mut findings = Vec::new();

    validate_graph_header(&graph, &mut findings);
    validate_selected_subject(&graph, &mut findings);

    let mut nodes = BTreeMap::<String, &FinalEvidenceNodeV1>::new();
    for node in &graph.nodes {
        if nodes.insert(node.evidence_id.clone(), node).is_some() {
            push_node_finding(
                &mut findings,
                FinalEvidenceFindingKindV1::DuplicateNode,
                node,
                "evidence ID is duplicated",
            );
        }
    }

    for node in &graph.nodes {
        validate_node(&graph, node, &mut findings);
    }

    let required = required_node_ids(&graph);
    validate_required_nodes(&required, &nodes, &mut findings);

    let valid_edges = validate_edges(&graph.edges, &nodes, &mut findings);
    validate_orphans(&required, &valid_edges, &mut findings);
    validate_cycles(&valid_edges, &nodes, &mut findings);

    let finding_non_current = findings
        .iter()
        .filter(|finding| finding_makes_node_non_current(finding.kind))
        .filter_map(|finding| finding.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    let direct_non_current = graph
        .nodes
        .iter()
        .filter(|node| node_is_non_current(node) || finding_non_current.contains(&node.evidence_id))
        .map(|node| node.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    let root_causes = propagate_non_current(&direct_non_current, &valid_edges);
    let rerun_roots = minimal_rerun_roots(&direct_non_current, &valid_edges);

    for root in &rerun_roots {
        let Some(node) = nodes.get(root).copied() else {
            continue;
        };
        if node.rerun_owner.as_deref().is_none_or(str::is_empty) {
            push_node_finding(
                &mut findings,
                FinalEvidenceFindingKindV1::MissingRerunOwner,
                node,
                "a root non-current evidence node has no rerun owner",
            );
        }
    }

    for node in &graph.nodes {
        if node_is_non_current(node) {
            push_node_finding(
                &mut findings,
                FinalEvidenceFindingKindV1::NonCurrentNode,
                node,
                &format!(
                    "node result `{}` and currentness `{}` are not clean",
                    node.result.label(),
                    node.currentness.label()
                ),
            );
        } else if root_causes
            .get(&node.evidence_id)
            .is_some_and(|roots| !roots.is_empty())
        {
            push_node_finding(
                &mut findings,
                FinalEvidenceFindingKindV1::TransitiveStaleness,
                node,
                "an upstream evidence dependency is non-current",
            );
        }
    }

    let mut node_dispositions = graph
        .nodes
        .iter()
        .map(|node| {
            let roots = root_causes
                .get(&node.evidence_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            FinalEvidenceNodeDispositionV1 {
                evidence_id: node.evidence_id.clone(),
                class: node.class,
                result: node.result,
                currentness: node.currentness,
                direct_non_current: direct_non_current.contains(&node.evidence_id),
                transitively_stale: !direct_non_current.contains(&node.evidence_id)
                    && !roots.is_empty(),
                root_causes: roots,
                rerun_owner: node.rerun_owner.clone(),
            }
        })
        .collect::<Vec<_>>();
    node_dispositions.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));

    findings.sort_by(|left, right| {
        (
            left.kind,
            left.evidence_id.as_deref(),
            left.edge.as_deref(),
            left.message.as_str(),
        )
            .cmp(&(
                right.kind,
                right.evidence_id.as_deref(),
                right.edge.as_deref(),
                right.message.as_str(),
            ))
    });
    findings.dedup_by(|left, right| {
        left.kind == right.kind
            && left.evidence_id == right.evidence_id
            && left.edge == right.edge
            && left.message == right.message
    });

    let rerun_owners = rerun_roots
        .iter()
        .filter_map(|root| nodes.get(root).and_then(|node| node.rerun_owner.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let result = aggregate_result(&graph, &findings, &node_dispositions);
    FinalEvidenceGraphEvaluationV1 {
        schema_id: FINAL_EVIDENCE_EVALUATION_SCHEMA_ID.to_string(),
        schema_version: FINAL_EVIDENCE_EVALUATION_SCHEMA_VERSION,
        graph_digest,
        result,
        findings,
        node_dispositions,
        rerun_roots: rerun_roots.into_iter().collect(),
        rerun_owners,
        limitations: graph.limitations.clone(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

fn validate_graph_header(graph: &FinalEvidenceGraphV1, findings: &mut Vec<FinalEvidenceFindingV1>) {
    if graph.schema_id != FINAL_EVIDENCE_GRAPH_SCHEMA_ID
        || graph.schema_version != FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION
    {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidSchema,
            "graph uses an unsupported schema generation",
        );
    }
    if graph.repository.trim().is_empty()
        || graph.claim_boundary.trim().is_empty()
        || graph.selected_subject.repository != graph.repository
    {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidSelectedSubject,
            "graph repository, selected repository, or claim boundary is missing or inconsistent",
        );
    }
}

fn validate_selected_subject(
    graph: &FinalEvidenceGraphV1,
    findings: &mut Vec<FinalEvidenceFindingV1>,
) {
    let subject = &graph.selected_subject;
    for (field, value) in [
        ("repository", subject.repository.as_str()),
        ("commit", subject.commit.as_str()),
        ("tree", subject.tree.as_str()),
    ] {
        if value.trim().is_empty() {
            push_graph_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidSelectedSubject,
                &format!("selected subject {field} is missing"),
            );
        }
    }
    for (field, digest) in [
        ("cargo_lock_digest", subject.cargo_lock_digest.as_str()),
        ("topology_digest", subject.topology_digest.as_str()),
    ] {
        if !is_sha256_digest(digest) {
            push_graph_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidDigest,
                &format!("selected subject {field} is not a SHA-256 digest"),
            );
        }
    }

    let identity = match ReleaseIdentityV1::parse(
        &subject.release_identity.version,
        &subject.release_identity.tag,
        subject.release_identity.github_prerelease,
    ) {
        Ok(identity) => Some(identity),
        Err(error) => {
            push_graph_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidSelectedSubject,
                &format!("selected release identity is invalid: {error}"),
            );
            None
        }
    };
    if graph.mode == FinalEvidenceGraphModeV1::Production
        && identity
            .as_ref()
            .is_some_and(|identity| identity.version().channel() != ReleaseChannelV1::Stable)
    {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidSelectedSubject,
            "production final evidence graph must select a stable release identity",
        );
    }

    let upload_rows = subject
        .package_rows
        .iter()
        .filter(|row| row.role == FinalEvidencePackageRoleV1::UploadCandidate)
        .count();
    let shared_rows = subject
        .package_rows
        .iter()
        .filter(|row| row.role == FinalEvidencePackageRoleV1::ExistingSharedPrerequisite)
        .count();
    if upload_rows != subject.expected_upload_rows as usize
        || shared_rows != subject.expected_shared_rows as usize
        || subject.package_rows.len() != upload_rows + shared_rows
    {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidPackageGraph,
            "selected package rows do not match the exact upload/shared denominator",
        );
    }

    let mut logical_ids = BTreeSet::new();
    let mut package_names = BTreeSet::new();
    for row in &subject.package_rows {
        validate_package_row(row, identity.as_ref(), findings, "selected subject");
        if !logical_ids.insert(row.logical_id.clone())
            || !package_names.insert(row.package_name.clone())
        {
            push_graph_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidPackageGraph,
                "selected package graph contains duplicate logical or Cargo package identity",
            );
        }
    }
}

fn validate_package_row(
    row: &FinalEvidencePackageSubjectV1,
    identity: Option<&ReleaseIdentityV1>,
    findings: &mut Vec<FinalEvidenceFindingV1>,
    owner: &str,
) {
    if row.logical_id.trim().is_empty() || row.package_name.trim().is_empty() {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidPackageGraph,
            &format!("{owner} has an empty package identity"),
        );
    }
    if !is_sha256_digest(&row.expected_digest)
        || row
            .observed_digest
            .as_deref()
            .is_some_and(|digest| !is_sha256_digest(digest))
    {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidDigest,
            &format!(
                "{owner} package `{}` has a malformed digest",
                row.package_name
            ),
        );
    }
    if row
        .observed_digest
        .as_deref()
        .is_some_and(|observed| observed != row.expected_digest)
    {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidPackageGraph,
            &format!(
                "{owner} package `{}` expected and observed digests conflict",
                row.package_name
            ),
        );
    }

    match row.role {
        FinalEvidencePackageRoleV1::UploadCandidate => {
            if identity.is_some_and(|identity| {
                identity
                    .validate_candidate_package_version(&row.package_name, &row.version)
                    .is_err()
            }) {
                push_graph_finding(
                    findings,
                    FinalEvidenceFindingKindV1::InvalidPackageGraph,
                    &format!(
                        "{owner} upload package `{}` is not on the exact selected release line",
                        row.package_name
                    ),
                );
            }
        }
        FinalEvidencePackageRoleV1::ExistingSharedPrerequisite => {
            let shared_line_is_invalid = match ReleaseVersionV1::parse(&row.version) {
                Ok(version) => version.channel() != ReleaseChannelV1::Stable,
                Err(_) => true,
            };
            if shared_line_is_invalid {
                push_graph_finding(
                    findings,
                    FinalEvidenceFindingKindV1::InvalidPackageGraph,
                    &format!(
                        "{owner} shared prerequisite `{}` is not on an exact stable line",
                        row.package_name
                    ),
                );
            }
        }
    }
}

fn validate_node(
    graph: &FinalEvidenceGraphV1,
    node: &FinalEvidenceNodeV1,
    findings: &mut Vec<FinalEvidenceFindingV1>,
) {
    if node.schema_id != FINAL_EVIDENCE_NODE_SCHEMA_ID
        || node.schema_version != FINAL_EVIDENCE_NODE_SCHEMA_VERSION
    {
        push_node_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidSchema,
            node,
            "node uses an unsupported schema generation",
        );
    }
    for (field, value) in [
        ("evidence_id", node.evidence_id.as_str()),
        ("producer_id", node.producer.producer_id.as_str()),
        ("producer tool", node.producer.tool.as_str()),
        ("subject repository", node.subject.repository.as_str()),
        ("claim boundary", node.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            push_node_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidSchema,
                node,
                &format!("node {field} is missing"),
            );
        }
    }
    for (field, digest) in [
        (
            "producer identity",
            Some(node.producer.identity_digest.as_str()),
        ),
        ("semantic", Some(node.semantic_digest.as_str())),
        (
            "expected semantic",
            node.expected_semantic_digest.as_deref(),
        ),
        ("artifact", node.artifact_digest.as_deref()),
        (
            "expected artifact",
            node.expected_artifact_digest.as_deref(),
        ),
    ] {
        if digest.is_some_and(|digest| !is_sha256_digest(digest)) {
            push_node_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidDigest,
                node,
                &format!("node {field} digest is malformed"),
            );
        }
    }

    if node
        .expected_semantic_digest
        .as_ref()
        .is_some_and(|expected| expected != &node.semantic_digest)
    {
        push_node_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidSelectedSubject,
            node,
            "semantic digest differs from the selected expected digest",
        );
    }
    if node
        .expected_artifact_digest
        .as_ref()
        .is_some_and(|expected| node.artifact_digest.as_ref() != Some(expected))
    {
        push_node_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidSelectedSubject,
            node,
            "artifact digest differs from the selected expected digest",
        );
    }

    if let Some(expectation) = &node.producer_expectation {
        let mismatch = expectation.producer_id != node.producer.producer_id
            || expectation.generation != node.producer.generation
            || expectation
                .identity_digest
                .as_ref()
                .is_some_and(|digest| digest != &node.producer.identity_digest);
        if mismatch {
            push_node_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidProducer,
                node,
                "producer identity or generation differs from the selected expectation",
            );
        }
    }

    if !origin_allowed(node.class, node.origin) {
        push_node_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidNodeOrigin,
            node,
            "node origin cannot establish the claimed evidence class",
        );
    }
    if graph.mode == FinalEvidenceGraphModeV1::Production
        && (node.origin == FinalEvidenceOriginV1::TestFixture
            || node.authority_scope == FinalEvidenceAuthorityScopeV1::FixtureOnly)
    {
        push_node_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidAuthorityUse,
            node,
            "fixture evidence cannot enter a production final evidence graph",
        );
    }

    if node.authority_scope == FinalEvidenceAuthorityScopeV1::FinalExact {
        validate_final_subject_binding(graph, node, findings);
    }
    for row in &node.subject.package_rows {
        validate_package_row(row, None, findings, &node.evidence_id);
    }
}

fn validate_final_subject_binding(
    graph: &FinalEvidenceGraphV1,
    node: &FinalEvidenceNodeV1,
    findings: &mut Vec<FinalEvidenceFindingV1>,
) {
    let selected = &graph.selected_subject;
    let mismatch = node.subject.repository != selected.repository
        || node.subject.commit.as_deref() != Some(selected.commit.as_str())
        || node.subject.tree.as_deref() != Some(selected.tree.as_str())
        || node.subject.cargo_lock_digest.as_deref() != Some(selected.cargo_lock_digest.as_str())
        || node.subject.topology_digest.as_deref() != Some(selected.topology_digest.as_str())
        || node.subject.release_identity.as_ref() != Some(&selected.release_identity);
    if mismatch {
        push_node_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidSelectedSubject,
            node,
            "final-authority node does not bind the exact selected release subject",
        );
    }
    if !node.subject.package_rows.is_empty() && node.subject.package_rows != selected.package_rows {
        push_node_finding(
            findings,
            FinalEvidenceFindingKindV1::InvalidPackageGraph,
            node,
            "final-authority package rows differ from the selected exact package graph",
        );
    }
}

fn origin_allowed(class: FinalEvidenceNodeClassV1, origin: FinalEvidenceOriginV1) -> bool {
    match class {
        FinalEvidenceNodeClassV1::SourceAuthority
        | FinalEvidenceNodeClassV1::SupportSelection
        | FinalEvidenceNodeClassV1::ChannelTruth => {
            origin == FinalEvidenceOriginV1::SourceAuthority
        }
        FinalEvidenceNodeClassV1::GeneratedProjection
        | FinalEvidenceNodeClassV1::AuthorizationPrerequisite => {
            matches!(
                origin,
                FinalEvidenceOriginV1::GeneratedProjection
                    | FinalEvidenceOriginV1::SourceAuthority
                    | FinalEvidenceOriginV1::TestFixture
            )
        }
        FinalEvidenceNodeClassV1::CandidateArtifact | FinalEvidenceNodeClassV1::PackageArchive => {
            matches!(
                origin,
                FinalEvidenceOriginV1::CandidateBytes
                    | FinalEvidenceOriginV1::WorkflowArtifact
                    | FinalEvidenceOriginV1::TestFixture
            )
        }
        FinalEvidenceNodeClassV1::InstalledJourney
        | FinalEvidenceNodeClassV1::PlatformReceipt
        | FinalEvidenceNodeClassV1::UpgradeRollbackReceipt
        | FinalEvidenceNodeClassV1::ReleaseRehearsal
        | FinalEvidenceNodeClassV1::ManifestResult
        | FinalEvidenceNodeClassV1::AssetResult => matches!(
            origin,
            FinalEvidenceOriginV1::WorkflowArtifact | FinalEvidenceOriginV1::TestFixture
        ),
        FinalEvidenceNodeClassV1::RegistryObservation
        | FinalEvidenceNodeClassV1::LiveControlObservation => matches!(
            origin,
            FinalEvidenceOriginV1::ProviderObservation | FinalEvidenceOriginV1::TestFixture
        ),
        FinalEvidenceNodeClassV1::IncidentHandoff => matches!(
            origin,
            FinalEvidenceOriginV1::HistoricalObservation | FinalEvidenceOriginV1::TestFixture
        ),
        FinalEvidenceNodeClassV1::ReviewDisposition => matches!(
            origin,
            FinalEvidenceOriginV1::HumanDecision
                | FinalEvidenceOriginV1::WorkflowArtifact
                | FinalEvidenceOriginV1::TestFixture
        ),
    }
}

fn required_node_ids(graph: &FinalEvidenceGraphV1) -> BTreeSet<String> {
    graph
        .required_node_ids
        .iter()
        .cloned()
        .chain(
            graph
                .nodes
                .iter()
                .filter(|node| node.required)
                .map(|node| node.evidence_id.clone()),
        )
        .collect()
}

fn validate_required_nodes(
    required: &BTreeSet<String>,
    nodes: &BTreeMap<String, &FinalEvidenceNodeV1>,
    findings: &mut Vec<FinalEvidenceFindingV1>,
) {
    for required_id in required {
        if !nodes.contains_key(required_id) {
            findings.push(FinalEvidenceFindingV1 {
                kind: FinalEvidenceFindingKindV1::MissingRequiredNode,
                evidence_id: Some(required_id.clone()),
                edge: None,
                message: "required evidence node is missing".to_string(),
                rerun_owner: None,
            });
        }
    }
}

fn validate_edges<'a>(
    edges: &'a [FinalEvidenceEdgeV1],
    nodes: &BTreeMap<String, &FinalEvidenceNodeV1>,
    findings: &mut Vec<FinalEvidenceFindingV1>,
) -> Vec<&'a FinalEvidenceEdgeV1> {
    let mut identities = BTreeSet::new();
    let mut pair_kinds = BTreeMap::<(String, String), BTreeSet<FinalEvidenceEdgeKindV1>>::new();
    let mut valid = Vec::new();

    for edge in edges {
        let edge_name = edge_name(edge);
        let has_valid_shape = !edge.from.trim().is_empty()
            && !edge.to.trim().is_empty()
            && edge.schema_id == FINAL_EVIDENCE_EDGE_SCHEMA_ID
            && edge.schema_version == FINAL_EVIDENCE_EDGE_SCHEMA_VERSION
            && !edge.claim_boundary.trim().is_empty();
        if !has_valid_shape {
            push_edge_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidSchema,
                edge,
                "edge uses an unsupported schema generation or lacks a claim boundary",
            );
        }
        if !identities.insert((edge.from.clone(), edge.to.clone(), edge.kind)) {
            push_edge_finding(
                findings,
                FinalEvidenceFindingKindV1::DuplicateEdge,
                edge,
                "edge identity is duplicated",
            );
        }
        let (Some(source), Some(_target)) = (nodes.get(&edge.from), nodes.get(&edge.to)) else {
            push_edge_finding(
                findings,
                FinalEvidenceFindingKindV1::UnknownEdgeEndpoint,
                edge,
                "edge references an unknown evidence node",
            );
            continue;
        };
        if edge.from == edge.to {
            push_edge_finding(
                findings,
                FinalEvidenceFindingKindV1::DependencyCycle,
                edge,
                "self-referential evidence dependency is not allowed",
            );
        }
        if source.authority_scope != FinalEvidenceAuthorityScopeV1::FinalExact
            && edge.kind.grants_positive_authority()
        {
            push_edge_finding(
                findings,
                FinalEvidenceFindingKindV1::InvalidAuthorityUse,
                edge,
                "support-only, historical, or fixture evidence cannot grant final authority",
            );
        }
        if !has_valid_shape || edge.from == edge.to {
            continue;
        }
        pair_kinds
            .entry((edge.from.clone(), edge.to.clone()))
            .or_default()
            .insert(edge.kind);
        valid.push(edge);
    }

    for ((from, to), kinds) in pair_kinds {
        let excludes = kinds.contains(&FinalEvidenceEdgeKindV1::ExcludesAsAuthority);
        let grants = kinds.iter().any(|kind| kind.grants_positive_authority());
        if excludes && grants {
            findings.push(FinalEvidenceFindingV1 {
                kind: FinalEvidenceFindingKindV1::ContradictoryEdge,
                evidence_id: None,
                edge: Some(format!("{from}->{to}")),
                message: "the same evidence pair both grants and excludes authority".to_string(),
                rerun_owner: None,
            });
        }
    }
    valid
}

fn validate_orphans(
    required: &BTreeSet<String>,
    edges: &[&FinalEvidenceEdgeV1],
    findings: &mut Vec<FinalEvidenceFindingV1>,
) {
    if required.len() <= 1 {
        return;
    }
    let connected = edges
        .iter()
        .flat_map(|edge| [edge.from.clone(), edge.to.clone()])
        .collect::<BTreeSet<_>>();
    for required_id in required {
        if !connected.contains(required_id) {
            findings.push(FinalEvidenceFindingV1 {
                kind: FinalEvidenceFindingKindV1::OrphanRequiredNode,
                evidence_id: Some(required_id.clone()),
                edge: None,
                message: "required evidence node is disconnected from the graph".to_string(),
                rerun_owner: None,
            });
        }
    }
}

fn validate_cycles(
    edges: &[&FinalEvidenceEdgeV1],
    nodes: &BTreeMap<String, &FinalEvidenceNodeV1>,
    findings: &mut Vec<FinalEvidenceFindingV1>,
) {
    let adjacency = dependency_adjacency(edges);
    let mut indegree = nodes
        .keys()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    for targets in adjacency.values() {
        for target in targets {
            if let Some(count) = indegree.get_mut(target) {
                *count += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| id.clone())
        .collect::<VecDeque<_>>();
    let mut visited = 0usize;
    while let Some(node) = ready.pop_front() {
        visited += 1;
        if let Some(targets) = adjacency.get(&node) {
            for target in targets {
                if let Some(count) = indegree.get_mut(target) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        ready.push_back(target.clone());
                    }
                }
            }
        }
    }
    if visited != nodes.len() {
        push_graph_finding(
            findings,
            FinalEvidenceFindingKindV1::DependencyCycle,
            "evidence dependency graph contains a cycle",
        );
    }
}

fn finding_makes_node_non_current(kind: FinalEvidenceFindingKindV1) -> bool {
    matches!(
        kind,
        FinalEvidenceFindingKindV1::InvalidDigest
            | FinalEvidenceFindingKindV1::InvalidSelectedSubject
            | FinalEvidenceFindingKindV1::InvalidPackageGraph
            | FinalEvidenceFindingKindV1::InvalidProducer
            | FinalEvidenceFindingKindV1::InvalidNodeOrigin
            | FinalEvidenceFindingKindV1::InvalidAuthorityUse
    )
}

fn node_is_non_current(node: &FinalEvidenceNodeV1) -> bool {
    node.result != FinalEvidenceNodeResultV1::Complete
        || node.currentness != FinalEvidenceCurrentnessV1::Current
}

fn dependency_adjacency(edges: &[&FinalEvidenceEdgeV1]) -> BTreeMap<String, BTreeSet<String>> {
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in edges {
        if edge.kind.propagates_non_current() {
            adjacency
                .entry(edge.from.clone())
                .or_default()
                .insert(edge.to.clone());
        }
    }
    adjacency
}

fn propagate_non_current(
    roots: &BTreeSet<String>,
    edges: &[&FinalEvidenceEdgeV1],
) -> BTreeMap<String, BTreeSet<String>> {
    let adjacency = dependency_adjacency(edges);
    let mut causes = BTreeMap::<String, BTreeSet<String>>::new();
    for root in roots {
        causes.entry(root.clone()).or_default().insert(root.clone());
        let mut pending = VecDeque::from([root.clone()]);
        let mut seen = BTreeSet::from([root.clone()]);
        while let Some(current) = pending.pop_front() {
            let Some(targets) = adjacency.get(&current) else {
                continue;
            };
            for target in targets {
                causes
                    .entry(target.clone())
                    .or_default()
                    .insert(root.clone());
                if seen.insert(target.clone()) {
                    pending.push_back(target.clone());
                }
            }
        }
    }
    causes
}

fn minimal_rerun_roots(
    direct: &BTreeSet<String>,
    edges: &[&FinalEvidenceEdgeV1],
) -> BTreeSet<String> {
    let causes = propagate_non_current(direct, edges);
    direct
        .iter()
        .filter(|candidate| {
            !causes.get(*candidate).is_some_and(|roots| {
                roots
                    .iter()
                    .any(|root| root != *candidate && direct.contains(root))
            })
        })
        .cloned()
        .collect()
}

fn aggregate_result(
    graph: &FinalEvidenceGraphV1,
    findings: &[FinalEvidenceFindingV1],
    dispositions: &[FinalEvidenceNodeDispositionV1],
) -> FinalEvidenceEvaluationResultV1 {
    let malformed = findings.iter().any(|finding| {
        matches!(
            finding.kind,
            FinalEvidenceFindingKindV1::InvalidSchema
                | FinalEvidenceFindingKindV1::InvalidDigest
                | FinalEvidenceFindingKindV1::DuplicateNode
                | FinalEvidenceFindingKindV1::DuplicateEdge
                | FinalEvidenceFindingKindV1::MissingRequiredNode
                | FinalEvidenceFindingKindV1::UnknownEdgeEndpoint
                | FinalEvidenceFindingKindV1::OrphanRequiredNode
                | FinalEvidenceFindingKindV1::DependencyCycle
                | FinalEvidenceFindingKindV1::InvalidProducer
                | FinalEvidenceFindingKindV1::InvalidNodeOrigin
                | FinalEvidenceFindingKindV1::MissingRerunOwner
        )
    });
    if malformed || graph_digest_failed(graph) {
        return FinalEvidenceEvaluationResultV1::MalformedGraph;
    }
    if findings.iter().any(|finding| {
        matches!(
            finding.kind,
            FinalEvidenceFindingKindV1::InvalidSelectedSubject
                | FinalEvidenceFindingKindV1::InvalidPackageGraph
                | FinalEvidenceFindingKindV1::InvalidAuthorityUse
                | FinalEvidenceFindingKindV1::ContradictoryEdge
        )
    }) || dispositions
        .iter()
        .any(|node| node.result == FinalEvidenceNodeResultV1::Conflict)
    {
        return FinalEvidenceEvaluationResultV1::Conflict;
    }
    if dispositions
        .iter()
        .any(|node| node.result == FinalEvidenceNodeResultV1::Incident)
    {
        return FinalEvidenceEvaluationResultV1::Incident;
    }
    if dispositions.iter().any(|node| {
        node.result == FinalEvidenceNodeResultV1::ProviderUnavailable
            || node.currentness == FinalEvidenceCurrentnessV1::ProviderUnavailable
    }) {
        return FinalEvidenceEvaluationResultV1::ProviderUnavailable;
    }
    if dispositions.iter().any(|node| {
        node.result == FinalEvidenceNodeResultV1::InstrumentFailure
            || node.currentness == FinalEvidenceCurrentnessV1::InstrumentFailure
    }) {
        return FinalEvidenceEvaluationResultV1::InstrumentFailure;
    }
    if dispositions.iter().any(|node| {
        node.result == FinalEvidenceNodeResultV1::Mismatch
            || node.currentness == FinalEvidenceCurrentnessV1::Mismatch
    }) {
        return FinalEvidenceEvaluationResultV1::Mismatch;
    }
    if dispositions.iter().any(|node| {
        node.result == FinalEvidenceNodeResultV1::Stale
            || matches!(
                node.currentness,
                FinalEvidenceCurrentnessV1::Stale | FinalEvidenceCurrentnessV1::Expired
            )
            || node.transitively_stale
    }) {
        return FinalEvidenceEvaluationResultV1::Stale;
    }
    if dispositions.iter().any(|node| {
        matches!(
            node.result,
            FinalEvidenceNodeResultV1::Incomplete
                | FinalEvidenceNodeResultV1::NotProven
                | FinalEvidenceNodeResultV1::Unsupported
                | FinalEvidenceNodeResultV1::Malformed
        )
    }) {
        return FinalEvidenceEvaluationResultV1::Incomplete;
    }
    FinalEvidenceEvaluationResultV1::Complete
}

fn graph_digest_failed(graph: &FinalEvidenceGraphV1) -> bool {
    final_evidence_graph_digest(graph).is_err()
}

fn is_sha256_digest(value: &str) -> bool {
    let hex = value
        .strip_prefix("sha256:v1:")
        .or_else(|| value.strip_prefix("sha256:"));
    hex.is_some_and(|hex| {
        hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
    })
}

fn edge_name(edge: &FinalEvidenceEdgeV1) -> String {
    format!("{}-{}->{}", edge.from, edge.kind.label(), edge.to)
}

fn push_graph_finding(
    findings: &mut Vec<FinalEvidenceFindingV1>,
    kind: FinalEvidenceFindingKindV1,
    message: &str,
) {
    findings.push(FinalEvidenceFindingV1 {
        kind,
        evidence_id: None,
        edge: None,
        message: message.to_string(),
        rerun_owner: None,
    });
}

fn push_node_finding(
    findings: &mut Vec<FinalEvidenceFindingV1>,
    kind: FinalEvidenceFindingKindV1,
    node: &FinalEvidenceNodeV1,
    message: &str,
) {
    findings.push(FinalEvidenceFindingV1 {
        kind,
        evidence_id: Some(node.evidence_id.clone()),
        edge: None,
        message: message.to_string(),
        rerun_owner: node.rerun_owner.clone(),
    });
}

fn push_edge_finding(
    findings: &mut Vec<FinalEvidenceFindingV1>,
    kind: FinalEvidenceFindingKindV1,
    edge: &FinalEvidenceEdgeV1,
    message: &str,
) {
    findings.push(FinalEvidenceFindingV1 {
        kind,
        evidence_id: None,
        edge: Some(edge_name(edge)),
        message: message.to_string(),
        rerun_owner: None,
    });
}
