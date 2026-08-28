use serde::{Deserialize, Serialize};

pub const FINAL_EVIDENCE_GRAPH_SCHEMA_ID: &str = "cargo-allow.final-evidence-graph.v1";
pub const FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION: u32 = 1;
pub const FINAL_EVIDENCE_NODE_SCHEMA_ID: &str = "cargo-allow.final-evidence-node.v1";
pub const FINAL_EVIDENCE_NODE_SCHEMA_VERSION: u32 = 1;
pub const FINAL_EVIDENCE_EDGE_SCHEMA_ID: &str = "cargo-allow.final-evidence-edge.v1";
pub const FINAL_EVIDENCE_EDGE_SCHEMA_VERSION: u32 = 1;
pub const FINAL_EVIDENCE_EVALUATION_SCHEMA_ID: &str = "cargo-allow.final-evidence-evaluation.v1";
pub const FINAL_EVIDENCE_EVALUATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceGraphModeV1 {
    Production,
    Fixture,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FinalEvidenceReleaseIdentityV1 {
    pub version: String,
    pub tag: String,
    pub github_prerelease: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidencePackageRoleV1 {
    UploadCandidate,
    ExistingSharedPrerequisite,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FinalEvidencePackageSubjectV1 {
    pub logical_id: String,
    pub package_name: String,
    pub version: String,
    pub role: FinalEvidencePackageRoleV1,
    pub expected_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceSelectedSubjectV1 {
    pub repository: String,
    pub commit: String,
    pub tree: String,
    pub cargo_lock_digest: String,
    pub topology_digest: String,
    pub release_identity: FinalEvidenceReleaseIdentityV1,
    pub expected_upload_rows: u32,
    pub expected_shared_rows: u32,
    pub package_rows: Vec<FinalEvidencePackageSubjectV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceSubjectBindingV1 {
    pub repository: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_lock_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_identity: Option<FinalEvidenceReleaseIdentityV1>,
    pub package_rows: Vec<FinalEvidencePackageSubjectV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceProducerV1 {
    pub producer_id: String,
    pub tool: String,
    pub generation: u32,
    pub identity_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_run_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_attempt: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceProducerExpectationV1 {
    pub producer_id: String,
    pub generation: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceNodeClassV1 {
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

impl FinalEvidenceNodeClassV1 {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::SourceAuthority => "source_authority",
            Self::GeneratedProjection => "generated_projection",
            Self::CandidateArtifact => "candidate_artifact",
            Self::PackageArchive => "package_archive",
            Self::InstalledJourney => "installed_journey",
            Self::PlatformReceipt => "platform_receipt",
            Self::UpgradeRollbackReceipt => "upgrade_rollback_receipt",
            Self::SupportSelection => "support_selection",
            Self::ChannelTruth => "channel_truth",
            Self::RegistryObservation => "registry_observation",
            Self::LiveControlObservation => "live_control_observation",
            Self::ReleaseRehearsal => "release_rehearsal",
            Self::ManifestResult => "manifest_result",
            Self::AssetResult => "asset_result",
            Self::IncidentHandoff => "incident_handoff",
            Self::ReviewDisposition => "review_disposition",
            Self::AuthorizationPrerequisite => "authorization_prerequisite",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceOriginV1 {
    SourceAuthority,
    GeneratedProjection,
    CandidateBytes,
    WorkflowArtifact,
    ProviderObservation,
    HistoricalObservation,
    HumanDecision,
    TestFixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceAuthorityScopeV1 {
    FinalExact,
    SupportOnly,
    HistoricalIncident,
    FixtureOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceNodeResultV1 {
    Complete,
    Incomplete,
    NotProven,
    Unsupported,
    ProviderUnavailable,
    InstrumentFailure,
    Conflict,
    Incident,
    Stale,
    Mismatch,
    Malformed,
}

impl FinalEvidenceNodeResultV1 {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
            Self::NotProven => "not_proven",
            Self::Unsupported => "unsupported",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InstrumentFailure => "instrument_failure",
            Self::Conflict => "conflict",
            Self::Incident => "incident",
            Self::Stale => "stale",
            Self::Mismatch => "mismatch",
            Self::Malformed => "malformed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceCurrentnessV1 {
    Current,
    Stale,
    Expired,
    Mismatch,
    ProviderUnavailable,
    InstrumentFailure,
}

impl FinalEvidenceCurrentnessV1 {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Expired => "expired",
            Self::Mismatch => "mismatch",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceInvalidationDimensionV1 {
    Source,
    PackageBytes,
    PackageManifest,
    CargoLock,
    Topology,
    SupportSelection,
    ChannelTruth,
    Workflow,
    ProducerGeneration,
    ProviderObservation,
    LiveControls,
    ReviewDisposition,
    AuthorizationPrerequisite,
    IncidentHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceNodeV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub evidence_id: String,
    pub class: FinalEvidenceNodeClassV1,
    pub origin: FinalEvidenceOriginV1,
    pub authority_scope: FinalEvidenceAuthorityScopeV1,
    pub required: bool,
    pub producer: FinalEvidenceProducerV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_expectation: Option<FinalEvidenceProducerExpectationV1>,
    pub subject: FinalEvidenceSubjectBindingV1,
    pub semantic_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_semantic_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_artifact_digest: Option<String>,
    pub result: FinalEvidenceNodeResultV1,
    pub currentness: FinalEvidenceCurrentnessV1,
    pub invalidation_dimensions: Vec<FinalEvidenceInvalidationDimensionV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerun_owner: Option<String>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceEdgeKindV1 {
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

impl FinalEvidenceEdgeKindV1 {
    pub(crate) const fn propagates_non_current(self) -> bool {
        matches!(
            self,
            Self::ProducedFrom
                | Self::RequiresCurrent
                | Self::RequiresExactEquality
                | Self::Projects
                | Self::InvalidatedBy
                | Self::Supersedes
                | Self::ConsumedBy
        )
    }

    pub(crate) const fn grants_positive_authority(self) -> bool {
        matches!(
            self,
            Self::ProducedFrom
                | Self::RequiresCurrent
                | Self::RequiresExactEquality
                | Self::Projects
                | Self::Supersedes
                | Self::ConsumedBy
        )
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::ProducedFrom => "produced_from",
            Self::RequiresCurrent => "requires_current",
            Self::RequiresExactEquality => "requires_exact_equality",
            Self::Projects => "projects",
            Self::ExcludesAsAuthority => "excludes_as_authority",
            Self::InvalidatedBy => "invalidated_by",
            Self::Supersedes => "supersedes",
            Self::SupportsOnly => "supports_only",
            Self::ConsumedBy => "consumed_by",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceEdgeV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub from: String,
    pub to: String,
    pub kind: FinalEvidenceEdgeKindV1,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceGraphV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub mode: FinalEvidenceGraphModeV1,
    pub repository: String,
    pub selected_subject: FinalEvidenceSelectedSubjectV1,
    pub required_node_ids: Vec<String>,
    pub nodes: Vec<FinalEvidenceNodeV1>,
    pub edges: Vec<FinalEvidenceEdgeV1>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

impl FinalEvidenceGraphV1 {
    /// Return a clone with all order-insensitive collections canonically ordered.
    #[must_use]
    pub fn canonicalized(&self) -> Self {
        let mut graph = self.clone();
        graph.selected_subject.package_rows.sort();
        graph.required_node_ids.sort();
        graph.required_node_ids.dedup();
        for node in &mut graph.nodes {
            node.subject.package_rows.sort();
            node.invalidation_dimensions.sort();
            node.invalidation_dimensions.dedup();
            node.limitations.sort();
            node.limitations.dedup();
        }
        graph
            .nodes
            .sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
        graph.edges.sort_by(|left, right| {
            (&left.from, &left.to, left.kind).cmp(&(&right.from, &right.to, right.kind))
        });
        graph.limitations.sort();
        graph.limitations.dedup();
        graph
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceEvaluationResultV1 {
    Complete,
    Incomplete,
    Stale,
    Mismatch,
    Conflict,
    MalformedGraph,
    ProviderUnavailable,
    InstrumentFailure,
    Incident,
}

impl FinalEvidenceEvaluationResultV1 {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
            Self::Stale => "stale",
            Self::Mismatch => "mismatch",
            Self::Conflict => "conflict",
            Self::MalformedGraph => "malformed_graph",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InstrumentFailure => "instrument_failure",
            Self::Incident => "incident",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalEvidenceFindingKindV1 {
    InvalidSchema,
    InvalidDigest,
    InvalidSelectedSubject,
    InvalidPackageGraph,
    DuplicateNode,
    DuplicateEdge,
    MissingRequiredNode,
    UnknownEdgeEndpoint,
    OrphanRequiredNode,
    DependencyCycle,
    InvalidProducer,
    InvalidNodeOrigin,
    InvalidAuthorityUse,
    ContradictoryEdge,
    NonCurrentNode,
    TransitiveStaleness,
    MissingRerunOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceFindingV1 {
    pub kind: FinalEvidenceFindingKindV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerun_owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceNodeDispositionV1 {
    pub evidence_id: String,
    pub class: FinalEvidenceNodeClassV1,
    pub result: FinalEvidenceNodeResultV1,
    pub currentness: FinalEvidenceCurrentnessV1,
    pub direct_non_current: bool,
    pub transitively_stale: bool,
    pub root_causes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerun_owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalEvidenceGraphEvaluationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub graph_digest: String,
    pub result: FinalEvidenceEvaluationResultV1,
    pub findings: Vec<FinalEvidenceFindingV1>,
    pub node_dispositions: Vec<FinalEvidenceNodeDispositionV1>,
    pub rerun_roots: Vec<String>,
    pub rerun_owners: Vec<String>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}
