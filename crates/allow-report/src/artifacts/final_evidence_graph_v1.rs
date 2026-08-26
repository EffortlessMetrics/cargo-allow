//! Final evidence dependency graph and transitive staleness propagation.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceNodeClassV1 {
    SourceAuthority,
    GeneratedProjection,
    CandidateArtifact,
    PackageArchive,
    InstalledJourney,
    PlatformReceipt,
    UpgradeRollbackReceipt,
    SupportSelection,
    ChannelTruth,
    RegistryObservation,
    LiveControlObservation,
    ReleaseRehearsal,
    ManifestResult,
    AssetResult,
    IncidentHandoff,
    ReviewDisposition,
    AuthorizationPrerequisite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceEdgeKindV1 {
    ProducedFrom,
    RequiresCurrent,
    RequiresExactEquality,
    Projects,
    ExcludesAsAuthority,
    InvalidatedBy,
    Supersedes,
    SupportsOnly,
    ConsumedBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceResultClassV1 {
    Complete,
    Stale,
    NotProven,
    Unsupported,
    ProviderUnavailable,
    InstrumentFailure,
    Conflict,
    Incident,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalEvidenceNodeV1 {
    pub node_id: String,
    pub node_class: EvidenceNodeClassV1,
    pub subject_version: String,
    pub producer_id: String,
    pub digest: String,
    pub result_class: EvidenceResultClassV1,
    pub claim_boundary: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalEvidenceEdgeV1 {
    pub from_node: String,
    pub to_node: String,
    pub edge_kind: EvidenceEdgeKindV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalEvidenceEvaluationV1 {
    pub overall_result: EvidenceResultClassV1,
    pub stale_nodes: Vec<String>,
    pub invalidated_descendants: Vec<String>,
    pub required_reruns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalEvidenceGraphInitV1 {
    pub graph_id: String,
    pub release_version: String,
    pub nodes: Vec<CargoAllowFinalEvidenceNodeV1>,
    pub edges: Vec<CargoAllowFinalEvidenceEdgeV1>,
    pub created_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalEvidenceGraphV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub graph_id: String,
    pub release_version: String,
    pub nodes: Vec<CargoAllowFinalEvidenceNodeV1>,
    pub edges: Vec<CargoAllowFinalEvidenceEdgeV1>,
    pub created_at_utc: String,
    pub claim_boundary: Vec<String>,
    pub limitations: Vec<String>,
}

impl CargoAllowFinalEvidenceGraphV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = "cargo-allow.final-evidence-graph.v1";
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    pub fn new(init: FinalEvidenceGraphInitV1) -> Self {
        Self {
            schema_id: Self::CURRENT_SCHEMA_ID.to_string(),
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            graph_id: init.graph_id,
            release_version: init.release_version,
            nodes: init.nodes,
            edges: init.edges,
            created_at_utc: init.created_at_utc,
            claim_boundary: vec![
                "typed_evidence_dependency_graph".to_string(),
                "transitive_staleness_propagation".to_string(),
                "rc_artifact_authority_exclusion".to_string(),
                "deterministic_rerun_set_identification".to_string(),
            ],
            limitations: vec![
                "does_not_execute_reruns_directly".to_string(),
                "does_not_mutate_remote_storage".to_string(),
            ],
        }
    }

    pub fn evaluate(&self) -> CargoAllowFinalEvidenceEvaluationV1 {
        // Schema and structural integrity
        if self.schema_id != Self::CURRENT_SCHEMA_ID
            || self.schema_version != Self::CURRENT_SCHEMA_VERSION
            || self.graph_id.is_empty()
            || self.nodes.is_empty()
        {
            return CargoAllowFinalEvidenceEvaluationV1 {
                overall_result: EvidenceResultClassV1::InstrumentFailure,
                stale_nodes: vec![],
                invalidated_descendants: vec![],
                required_reruns: vec![],
            };
        }

        let node_map: HashMap<&str, &CargoAllowFinalEvidenceNodeV1> =
            self.nodes.iter().map(|n| (n.node_id.as_str(), n)).collect();

        // Edge target validity
        for edge in &self.edges {
            if !node_map.contains_key(edge.from_node.as_str())
                || !node_map.contains_key(edge.to_node.as_str())
            {
                return CargoAllowFinalEvidenceEvaluationV1 {
                    overall_result: EvidenceResultClassV1::InstrumentFailure,
                    stale_nodes: vec![],
                    invalidated_descendants: vec![],
                    required_reruns: vec![],
                };
            }
        }

        // Cycle check via Kahn's algorithm or DFS
        if has_dependency_cycles(&self.nodes, &self.edges) {
            return CargoAllowFinalEvidenceEvaluationV1 {
                overall_result: EvidenceResultClassV1::Conflict,
                stale_nodes: vec![],
                invalidated_descendants: vec![],
                required_reruns: vec![],
            };
        }

        // Track directly non-complete nodes
        let mut stale_set = HashSet::new();
        let mut conflicting = false;
        let mut provider_unavailable = false;
        let mut incident = false;

        for node in &self.nodes {
            match node.result_class {
                EvidenceResultClassV1::Complete => {}
                EvidenceResultClassV1::Stale => {
                    stale_set.insert(node.node_id.as_str());
                }
                EvidenceResultClassV1::Conflict => {
                    conflicting = true;
                    stale_set.insert(node.node_id.as_str());
                }
                EvidenceResultClassV1::ProviderUnavailable => {
                    provider_unavailable = true;
                    stale_set.insert(node.node_id.as_str());
                }
                EvidenceResultClassV1::Incident => {
                    incident = true;
                    stale_set.insert(node.node_id.as_str());
                }
                EvidenceResultClassV1::NotProven
                | EvidenceResultClassV1::Unsupported
                | EvidenceResultClassV1::InstrumentFailure => {
                    stale_set.insert(node.node_id.as_str());
                }
            }

            // RC.1 exclusion rule: if an RC.1 package archive is incorrectly used as authority for 0.2.0 final release
            if node.subject_version.contains("-rc.")
                && node.node_class == EvidenceNodeClassV1::PackageArchive
                && self.release_version == "0.2.0"
            {
                for edge in &self.edges {
                    if edge.from_node == node.node_id
                        && (edge.edge_kind == EvidenceEdgeKindV1::RequiresExactEquality
                            || edge.edge_kind == EvidenceEdgeKindV1::RequiresCurrent)
                    {
                        conflicting = true;
                        stale_set.insert(node.node_id.as_str());
                    }
                }
            }
        }

        // Transitive staleness propagation
        // If node A is stale, any node B that depends on A via ProducedFrom, RequiresCurrent, RequiresExactEquality is invalidated
        let mut forward_adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            if edge.edge_kind == EvidenceEdgeKindV1::ProducedFrom
                || edge.edge_kind == EvidenceEdgeKindV1::RequiresCurrent
                || edge.edge_kind == EvidenceEdgeKindV1::RequiresExactEquality
            {
                // from_node is the prerequisite, to_node depends on from_node
                forward_adj
                    .entry(edge.from_node.as_str())
                    .or_default()
                    .push(edge.to_node.as_str());
            }
        }

        let mut queue: VecDeque<&str> = stale_set.iter().copied().collect();
        let mut invalidated_set = HashSet::new();

        while let Some(current) = queue.pop_front() {
            if let Some(dependents) = forward_adj.get(current) {
                for &dep in dependents {
                    if !stale_set.contains(dep) && invalidated_set.insert(dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }

        let mut stale_nodes: Vec<String> = stale_set.into_iter().map(String::from).collect();
        stale_nodes.sort();

        let mut invalidated_descendants: Vec<String> =
            invalidated_set.into_iter().map(String::from).collect();
        invalidated_descendants.sort();

        let mut required_reruns = stale_nodes.clone();
        for inv in &invalidated_descendants {
            if !required_reruns.contains(inv) {
                required_reruns.push(inv.clone());
            }
        }
        required_reruns.sort();

        let overall_result = if conflicting {
            EvidenceResultClassV1::Conflict
        } else if incident {
            EvidenceResultClassV1::Incident
        } else if provider_unavailable {
            EvidenceResultClassV1::ProviderUnavailable
        } else if !stale_nodes.is_empty() || !invalidated_descendants.is_empty() {
            EvidenceResultClassV1::Stale
        } else {
            EvidenceResultClassV1::Complete
        };

        CargoAllowFinalEvidenceEvaluationV1 {
            overall_result,
            stale_nodes,
            invalidated_descendants,
            required_reruns,
        }
    }
}

fn has_dependency_cycles(
    nodes: &[CargoAllowFinalEvidenceNodeV1],
    edges: &[CargoAllowFinalEvidenceEdgeV1],
) -> bool {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for node in nodes {
        in_degree.insert(node.node_id.as_str(), 0);
    }

    for edge in edges {
        *in_degree.entry(edge.to_node.as_str()).or_default() += 1;
        adj.entry(edge.from_node.as_str())
            .or_default()
            .push(edge.to_node.as_str());
    }

    let mut queue = VecDeque::new();
    for (node, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(*node);
        }
    }

    let mut visited_count = 0;
    while let Some(curr) = queue.pop_front() {
        visited_count += 1;
        if let Some(neighbors) = adj.get(curr) {
            for &next in neighbors {
                if let Some(deg) = in_degree.get_mut(next) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    visited_count != nodes.len()
}
