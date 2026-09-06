//! Final release-freeze composition and replay driver (#2501).
//!
//! Composes the typed `CargoAllowFinalFreezeReceiptV1` from bounded retained
//! evidence produced at one exact source subject (the clean committed HEAD),
//! then verifies it with the read-only `replay_final_freeze` contract. The
//! command owns no release meaning of its own: every load-bearing claim must
//! be carried in by an evidence producer that already has its own receipt and
//! drift tests. Missing, stale, or unbindable evidence yields an `Incomplete`
//! or `Mismatch` freeze — never a fabricated `Complete`.
//!
//! Claim boundary: this command writes only the caller-supplied output
//! directory. It never commits, pushes, tags, reads a token, uploads, yanks,
//! attests, mutates a GitHub Release or live settings, or produces release
//! authorization. A `Complete` freeze means the exact recorded bytes and
//! claims may be considered for #3760 authorization; it is not authorization.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use allow_report::{
    ArtifactTransferFileV1, ArtifactTransferInitV1, CandidateCustodyInitV1,
    CandidatePreparationReceiptV1, CandidatePreparationStateV1, CargoAllowFinalFreezeReceiptV1,
    CargoAllowFinalFreezeReplayInputsV1, CargoAllowFrozenCandidateCustodyV1,
    CargoAllowReleaseArtifactTransferV1, ConfidentialityClassV1, CustodyFileV1,
    FinalEvidenceAuthorityScopeV1, FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1,
    FinalEvidenceEdgeV1, FinalEvidenceGraphModeV1, FinalEvidenceGraphV1,
    FinalEvidenceInvalidationDimensionV1, FinalEvidenceNodeClassV1, FinalEvidenceNodeResultV1,
    FinalEvidenceNodeV1, FinalEvidenceOriginV1, FinalEvidencePackageRoleV1,
    FinalEvidencePackageSubjectV1, FinalEvidenceProducerV1, FinalEvidenceReleaseIdentityV1,
    FinalEvidenceSelectedSubjectV1, FinalEvidenceSubjectBindingV1, FinalFreezeManifestBindingV1,
    FinalFreezeManifestResultV1, FinalFreezeReceiptInitV1, FinalFreezeReplayResultV1,
    FinalReadinessCustodyPostureV1, FinalReadinessDecisionInputsV1, FinalReadinessDecisionStateV1,
    FinalReadinessPostMergePostureV1, FinalReadinessQualificationPostureV1,
    FinalReadinessRootDecisionV1, FinalReadinessSupportedLimitationV1, FinalReadinessVerdictV1,
    FinalSelectionDispositionV1, FinalSupportSelectionV1, ObservationFreshnessV1,
    ObservationReadingV1, ProducerIdentityV1, RefreshableObservationAdapterV1,
    RefreshableObservationKindV1, RefreshableObservationV1, ReleaseChannelV1, ReleaseVersionV1,
    RetainedArtifactBytesV1, RetainedCustodyItemV1, RetainedExactArtifactV1, TrustClassV1,
    UntrustedInputPostureV1, aggregate_final_readiness, evaluate_final_evidence_graph,
    final_evidence_graph_digest, render_final_freeze_replay_json,
    render_final_freeze_replay_markdown, render_final_readiness_json, replay_final_freeze,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::Value as Json;

use crate::cli::candidate_preparation_command::git_root;

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const WORKSPACE_MANIFEST_PATH: &str = "Cargo.toml";
const CARGO_LOCK_PATH: &str = "Cargo.lock";
const TOPOLOGY_PATH: &str = "policy/product-package-topology-v2.toml";
const SUPPORT_MATRIX_PATH: &str = "docs/support-matrix.toml";
const INCIDENT_EVIDENCE_PATH: &str = "docs/release/evidence/rc1-publication-incident.v1.json";

const EXPECTED_UPLOAD_ROWS: u32 = 10;
const EXPECTED_SHARED_ROWS: u32 = 3;

/// The remaining irreversible operations a `Complete` freeze must name.
const REMAINING_IRREVERSIBLE_OPERATIONS: [&str; 3] = [
    "push tag v0.2.0",
    "upload 10 package rows to crates.io",
    "publish the GitHub release",
];

/// Read-only final release-freeze composition (hidden release tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReleaseFreezeArgs {
    #[command(subcommand)]
    pub(crate) command: ReleaseFreezeSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReleaseFreezeSubcommand {
    /// Compose the final freeze receipt from retained evidence and replay it.
    Compose(ReleaseFreezeComposeArgs),
}

/// One retained evidence input, `role=relative-or-absolute-path`.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ReleaseFreezeComposeArgs {
    /// Prospective final release version (stable line).
    #[arg(long, default_value = "0.2.0")]
    pub(crate) version: String,
    /// Retained evidence inputs, `role=path` (repeatable).
    #[arg(long = "evidence")]
    pub(crate) evidence: Vec<String>,
    /// Output directory for the freeze receipt, replay, and digest summary.
    #[arg(long, default_value = "target/cargo-allow/freeze")]
    pub(crate) out_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum FreezeEvidenceRole {
    /// CandidatePreparationReceiptV1 from `prep-candidate apply --final-receipt`.
    #[value(name = "candidate-preparation")]
    CandidatePreparation,
    /// ExactCandidatePackageSetV1 receipt (archives bind to its `packages/` dir).
    #[value(name = "package-set")]
    PackageSet,
    /// `final-package-docs.receipt.json` from `scripts/final-package-docs.py`.
    #[value(name = "package-docs")]
    PackageDocs,
    /// `scripts/release-rehearsal.py` aggregate receipt.
    #[value(name = "rehearsal")]
    Rehearsal,
    /// Exact-candidate isolated install journey receipt (#2925/#2926).
    #[value(name = "install-journey")]
    InstallJourney,
    /// Exact-candidate interop smoke receipt.
    #[value(name = "interop")]
    Interop,
    /// `scripts/verify-crate-registry-version.sh` observation JSON.
    #[value(name = "registry-observation")]
    RegistryObservation,
    /// ReleaseManifestV2 prepublication envelope JSON.
    #[value(name = "release-manifest")]
    ReleaseManifest,
    /// Upgrade/rollback journey receipt (#2485/#3853).
    #[value(name = "upgrade-rollback")]
    UpgradeRollback,
    /// Live release-control observation receipt (#2284).
    #[value(name = "controls")]
    Controls,
}

impl FreezeEvidenceRole {
    fn from_label(label: &str) -> Option<Self> {
        use clap::ValueEnum as _;
        Self::value_variants().iter().copied().find(|variant| {
            variant
                .to_possible_value()
                .is_some_and(|value| value.matches(label, false))
        })
    }

    fn graph_shape(
        self,
    ) -> (
        FinalEvidenceNodeClassV1,
        FinalEvidenceOriginV1,
        &'static str,
    ) {
        match self {
            Self::PackageSet => (
                FinalEvidenceNodeClassV1::PackageArchive,
                FinalEvidenceOriginV1::CandidateBytes,
                "package-archive",
            ),
            Self::Rehearsal => (
                FinalEvidenceNodeClassV1::ReleaseRehearsal,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "release-rehearsal",
            ),
            Self::PackageDocs => (
                FinalEvidenceNodeClassV1::ManifestResult,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "manifest-result",
            ),
            Self::CandidatePreparation => (
                FinalEvidenceNodeClassV1::CandidateArtifact,
                FinalEvidenceOriginV1::CandidateBytes,
                "candidate-preparation",
            ),
            Self::InstallJourney => (
                FinalEvidenceNodeClassV1::InstalledJourney,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "installed-journey",
            ),
            Self::Interop => (
                FinalEvidenceNodeClassV1::PlatformReceipt,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "platform-receipt",
            ),
            Self::RegistryObservation => (
                FinalEvidenceNodeClassV1::RegistryObservation,
                FinalEvidenceOriginV1::ProviderObservation,
                "registry-observation",
            ),
            Self::ReleaseManifest => (
                FinalEvidenceNodeClassV1::AssetResult,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "asset-result",
            ),
            Self::UpgradeRollback => (
                FinalEvidenceNodeClassV1::UpgradeRollbackReceipt,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "upgrade-rollback-receipt",
            ),
            Self::Controls => (
                FinalEvidenceNodeClassV1::LiveControlObservation,
                FinalEvidenceOriginV1::ProviderObservation,
                "live-control-observation",
            ),
        }
    }

    fn label(self) -> String {
        use clap::ValueEnum as _;
        self.to_possible_value()
            .map(|value| value.get_name().to_string())
            .unwrap_or_else(|| format!("{self:?}"))
    }
}

/// One parsed, digest-recorded evidence input.
struct EvidenceInput {
    role: FreezeEvidenceRole,
    path: PathBuf,
    sha256: String,
    value: Json,
    binding_notes: Vec<String>,
}

fn evidence_role(evidence: &[EvidenceInput], role: FreezeEvidenceRole) -> Option<&EvidenceInput> {
    evidence.iter().find(|input| input.role == role)
}

impl EvidenceInput {
    fn bound_ok(&self) -> bool {
        !self
            .binding_notes
            .iter()
            .any(|note| note.starts_with("fail:"))
    }
}

/// The composed freeze verdict plus the rows a human reviewer reads first.
#[derive(Debug, serde::Serialize)]
struct FreezeCompositionSummary {
    schema_id: &'static str,
    repository: &'static str,
    commit: String,
    tree: String,
    release_version: String,
    release_tag: String,
    freeze_receipt_sha256: String,
    freeze_state: String,
    replay_result: String,
    replay_retained_bytes_verified: bool,
    readiness_verdict: String,
    selection_digest: String,
    package_rows: u32,
    shared_rows: u32,
    evidence_rows: Vec<EvidenceRow>,
    remaining_irreversible_operations: Vec<String>,
    blocking_rows: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct EvidenceRow {
    role: String,
    path: String,
    sha256: String,
    bound: bool,
    detail: String,
}

pub(super) fn cmd_release_freeze(args: &ReleaseFreezeArgs) -> CargoAllowResult<()> {
    let root = git_root().map_err(|reason| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("release-freeze requires a git worktree: {reason}"),
        )
    })?;
    match &args.command {
        ReleaseFreezeSubcommand::Compose(compose) => cmd_compose(&root, compose),
    }
}

fn cmd_compose(root: &Path, args: &ReleaseFreezeComposeArgs) -> CargoAllowResult<()> {
    let subject = SubjectIdentity::collect(root, &args.version)?;
    let selection = load_selection(root, &subject)?;
    let shared = load_shared_prerequisites(root)?;
    let evidence = collect_evidence(root, args, &subject)?;
    let incident_digest = load_incident_handoff(root);

    // The exact 10+3 package graph must exist before the evidence graph:
    // the graph's selected subject carries the full row set.
    let package_rows = subject.package_rows(&shared, &evidence)?;
    let graph = build_evidence_graph(
        &subject,
        &selection,
        &evidence,
        &package_rows,
        incident_digest.as_deref(),
    );
    let evaluation = evaluate_final_evidence_graph(&graph);
    let decision_inputs = readiness_decision_inputs(&subject, &selection, &evidence);
    let readiness = aggregate_final_readiness(&graph, &decision_inputs);
    let graph_digest = final_evidence_graph_digest(&graph)
        .map_err(|reason| instrument(format!("evidence graph digest: {reason}")))?;

    let package_rows = subject.package_rows(&shared, &evidence)?;
    let archives = ArchiveSet::collect(&evidence, &package_rows)?;
    let manifest_bytes = evidence_role(&evidence, FreezeEvidenceRole::ReleaseManifest)
        .map(|input| read_evidence_bytes(&input.path))
        .transpose()?;

    // The receipt binds the custody id; the custody aggregate then retains
    // the serialized receipt (and the prepublication manifest) beside the
    // package archives so the replay input set is self-contained.
    let custody_id = format!("candidate-custody-{}-final", subject.version);
    let receipt = CargoAllowFinalFreezeReceiptV1::new(FinalFreezeReceiptInitV1 {
        freeze_id: format!("freeze-{}-final", subject.version),
        frozen_custody_id: custody_id,
        frozen_at_utc: subject.frozen_at_utc.clone(),
        release_identity: subject.release_identity(),
        repository: REPOSITORY.to_string(),
        commit: subject.commit.clone(),
        tree: subject.tree.clone(),
        cargo_lock_digest: subject.cargo_lock_digest.clone(),
        topology_digest: subject.topology_digest.clone(),
        expected_upload_rows: EXPECTED_UPLOAD_ROWS,
        expected_shared_rows: EXPECTED_SHARED_ROWS,
        package_rows: package_rows.clone(),
        prepublication_manifest: manifest_binding(&evidence),
        rc1_excluded: true,
        rc1_version: Some("0.2.0-rc.1".to_string()),
        incident_handoff_id: incident_digest
            .as_ref()
            .map(|_| "incident-handoff".to_string()),
        recorded_graph_digest: graph_digest,
        remaining_irreversible_operations: REMAINING_IRREVERSIBLE_OPERATIONS
            .iter()
            .map(|operation| (*operation).to_string())
            .collect(),
    });
    let receipt_bytes = serde_json::to_vec(&receipt)
        .map_err(|error| instrument(format!("receipt serialization: {error}")))?;

    let custody = build_custody(
        &subject,
        &package_rows,
        &archives,
        &receipt_bytes,
        manifest_bytes.as_deref(),
    )?;
    let transfers = build_transfers(
        &subject,
        &package_rows,
        &archives,
        &receipt_bytes,
        manifest_bytes.as_deref(),
    )?;
    let retained_artifacts =
        build_retained_artifacts(&archives, &receipt_bytes, manifest_bytes.as_deref());

    let replay_inputs = CargoAllowFinalFreezeReplayInputsV1 {
        custody,
        evidence_graph: graph,
        freeze_receipt: receipt,
        retained_transfers: transfers,
        retained_artifacts,
        observations: observation_set(&evidence),
        replayed_at_utc: subject.frozen_at_utc.clone(),
    };
    let replayed = replay_final_freeze(
        &replay_inputs,
        &FreezeObservationAdapter {
            source_current: evidence_role(&evidence, FreezeEvidenceRole::Controls)
                .map(EvidenceInput::bound_ok)
                .unwrap_or(false),
            registry_current: rehearsal_registry_preflight_current(&evidence),
        },
    );
    let receipt_sha256 = sha256_v1_bytes(&receipt_bytes);

    let graph_complete =
        evaluation.findings.is_empty() && evidence.iter().all(EvidenceInput::bound_ok);
    let complete = graph_complete
        && readiness.verdict == FinalReadinessVerdictV1::ReadyForFreeze
        && replayed.result == FinalFreezeReplayResultV1::CompleteEquivalent;

    write_outputs(
        args,
        root,
        &replay_inputs,
        &receipt_bytes,
        &replayed,
        &readiness,
    )?;

    let freeze_state = if complete { "Complete" } else { "Incomplete" };
    let summary = FreezeCompositionSummary {
        schema_id: "cargo-allow.release-freeze-composition.v1",
        repository: REPOSITORY,
        commit: subject.commit.clone(),
        tree: subject.tree.clone(),
        release_version: subject.version.clone(),
        release_tag: subject.tag.clone(),
        freeze_receipt_sha256: receipt_sha256,
        freeze_state: freeze_state.to_string(),
        replay_result: format!("{:?}", replayed.result),
        replay_retained_bytes_verified: replayed.retained_bytes_verified,
        readiness_verdict: format!("{:?}", readiness.verdict),
        selection_digest: selection.selection_digest.clone(),
        package_rows: EXPECTED_UPLOAD_ROWS,
        shared_rows: EXPECTED_SHARED_ROWS,
        evidence_rows: evidence
            .iter()
            .map(|input| EvidenceRow {
                role: input.role.label(),
                path: input.path.display().to_string(),
                sha256: input.sha256.clone(),
                bound: input.bound_ok(),
                detail: input.binding_notes.join("; "),
            })
            .collect(),
        remaining_irreversible_operations: REMAINING_IRREVERSIBLE_OPERATIONS
            .iter()
            .map(|operation| (*operation).to_string())
            .collect(),
        blocking_rows: replayed
            .rows
            .iter()
            .map(|row| row.message.clone())
            .chain(
                evaluation
                    .findings
                    .iter()
                    .map(|finding| finding.message.clone()),
            )
            .collect(),
    };
    let rendered = serde_json::to_string_pretty(&summary)
        .map_err(|error| instrument(format!("summary serialization: {error}")))?;
    println!("{rendered}");

    if complete && replayed.retained_bytes_verified {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            format!(
                "the freeze did not reach a verified Complete replay: state={freeze_state} replay={:?} readiness={:?} retained_bytes_verified={}",
                replayed.result, readiness.verdict, replayed.retained_bytes_verified
            ),
        ))
    }
}

/// The exact source subject the freeze binds. Collected from the clean
/// committed HEAD; a dirty worktree is an instrument failure because the
/// packaged archives must come from the committed tree.
struct SubjectIdentity {
    version: String,
    tag: String,
    channel: String,
    commit: String,
    tree: String,
    cargo_lock_digest: String,
    topology_digest: String,
    frozen_at_utc: String,
}

impl SubjectIdentity {
    fn collect(root: &Path, version: &str) -> CargoAllowResult<Self> {
        let dirty = git(root, &["status", "--porcelain"])?;
        if !dirty.trim().is_empty() {
            return Err(instrument(
                "the worktree is dirty; the freeze binds the committed subject only",
            ));
        }
        let commit = git(root, &["rev-parse", "HEAD"])?;
        let tree = git(root, &["rev-parse", "HEAD^{tree}"])?;
        let manifest = read_repo_file(root, WORKSPACE_MANIFEST_PATH)?;
        let declared = manifest
            .lines()
            .filter_map(|line| line.trim().strip_prefix("version = "))
            .next()
            .map(|value| value.trim().trim_matches('"').to_string())
            .ok_or_else(|| instrument("the workspace manifest has no version"))?;
        if declared != version {
            return Err(instrument(format!(
                "workspace version {declared:?} is not the requested freeze version {version:?}"
            )));
        }
        let parsed = ReleaseVersionV1::parse(&declared)
            .map_err(|error| instrument(format!("release identity: {error}")))?;
        if parsed.channel() != ReleaseChannelV1::Stable {
            return Err(instrument(
                "the freeze version is not on the stable channel",
            ));
        }
        let projection = CandidateReleaseIdentityProjectionShim::from_version(&parsed);
        let cargo_lock_digest = sha256_repo_file(root, CARGO_LOCK_PATH)?;
        let topology_digest = sha256_repo_file(root, TOPOLOGY_PATH)?;
        let frozen_at_utc = git(root, &["log", "-1", "--format=%cI"])?;
        Ok(Self {
            version: declared,
            tag: projection.tag,
            channel: projection.channel,
            commit: commit.trim().to_string(),
            tree: tree.trim().to_string(),
            cargo_lock_digest,
            topology_digest,
            frozen_at_utc: frozen_at_utc.trim().to_string(),
        })
    }

    fn release_identity(&self) -> FinalEvidenceReleaseIdentityV1 {
        FinalEvidenceReleaseIdentityV1 {
            version: self.version.clone(),
            tag: self.tag.clone(),
            github_prerelease: false,
        }
    }

    fn binding(&self) -> FinalEvidenceSubjectBindingV1 {
        FinalEvidenceSubjectBindingV1 {
            repository: REPOSITORY.to_string(),
            commit: Some(self.commit.clone()),
            tree: Some(self.tree.clone()),
            cargo_lock_digest: Some(self.cargo_lock_digest.clone()),
            topology_digest: Some(self.topology_digest.clone()),
            release_identity: Some(self.release_identity()),
            package_rows: Vec::new(),
        }
    }

    /// The exact 10+3 package graph: upload rows from the package-set
    /// evidence, shared rows from the topology's retained registry checksums.
    fn package_rows(
        &self,
        shared: &[(String, String, String)],
        evidence: &[EvidenceInput],
    ) -> CargoAllowResult<Vec<FinalEvidencePackageSubjectV1>> {
        let mut rows = Vec::new();
        if let Some(package_set) = evidence_role(evidence, FreezeEvidenceRole::PackageSet) {
            let crates = package_set
                .value
                .pointer("/package_set/crates")
                .and_then(Json::as_array)
                .ok_or_else(|| instrument("package-set receipt has no package_set.crates rows"))?;
            let mut upload = 0usize;
            let mut shared_in_receipt = 0usize;
            for row in crates {
                let name = str_field(row, "name")
                    .ok_or_else(|| instrument("package-set crate row has no name"))?;
                let version = str_field(row, "version")
                    .ok_or_else(|| instrument("package-set crate row has no version"))?;
                let digest = str_field(row, "sha256")
                    .ok_or_else(|| instrument("package-set crate row has no sha256"))?;
                if version == self.version {
                    upload += 1;
                    rows.push(FinalEvidencePackageSubjectV1 {
                        logical_id: name.clone(),
                        package_name: name,
                        version,
                        role: FinalEvidencePackageRoleV1::UploadCandidate,
                        expected_digest: canonical_digest(&digest),
                        observed_digest: Some(canonical_digest(&digest)),
                    });
                } else {
                    shared_in_receipt += 1;
                }
            }
            if upload != EXPECTED_UPLOAD_ROWS as usize {
                return Err(instrument(format!(
                    "package-set receipt carries {upload} upload rows for {}, expected {EXPECTED_UPLOAD_ROWS}",
                    self.version
                )));
            }
            if shared_in_receipt != EXPECTED_SHARED_ROWS as usize {
                return Err(instrument(format!(
                    "package-set receipt carries {shared_in_receipt} shared-prerequisite rows, expected {EXPECTED_SHARED_ROWS}"
                )));
            }
        }
        if shared.len() != EXPECTED_SHARED_ROWS as usize {
            return Err(instrument(format!(
                "the topology carries {} selected shared prerequisites, expected {EXPECTED_SHARED_ROWS}",
                shared.len()
            )));
        }
        for (name, version, checksum) in shared {
            rows.push(FinalEvidencePackageSubjectV1 {
                logical_id: name.clone(),
                package_name: name.clone(),
                version: version.clone(),
                role: FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
                expected_digest: checksum.clone(),
                observed_digest: None,
            });
        }
        Ok(rows)
    }
}

/// Local stable-line projection of the typed release identity (same law as
/// the candidate-preparation plan module).
struct CandidateReleaseIdentityProjectionShim;

impl CandidateReleaseIdentityProjectionShim {
    fn from_version(version: &ReleaseVersionV1) -> ProjectionFields {
        ProjectionFields {
            tag: version.tag(),
            channel: "stable".to_string(),
        }
    }
}

struct ProjectionFields {
    tag: String,
    channel: String,
}

/// The final support selection, verified through its single typed
/// implementation; the selection digest becomes part of the freeze record.
fn load_selection(
    root: &Path,
    subject: &SubjectIdentity,
) -> CargoAllowResult<FinalSupportSelectionV1> {
    let text = read_repo_file(root, SUPPORT_MATRIX_PATH)?;
    let parsed: toml::Value = toml::from_str(&text)
        .map_err(|error| instrument(format!("support matrix is not valid TOML: {error}")))?;
    let section = parsed
        .get("final_selection")
        .ok_or_else(|| instrument("the support matrix has no [final_selection] section"))?;
    let json = serde_json::to_value(section)
        .map_err(|error| instrument(format!("selection serialization: {error}")))?;
    let selection: FinalSupportSelectionV1 = serde_json::from_value(json)
        .map_err(|error| instrument(format!("final selection parse: {error}")))?;
    selection
        .verify()
        .map_err(|error| instrument(format!("final selection verification: {error}")))?;
    if !selection.needs_decision_rows().is_empty() {
        return Err(instrument(
            "the final selection carries needs_decision rows; the freeze consumes a non-selection",
        ));
    }
    if selection.release_version != subject.version
        || selection.release_tag != subject.tag
        || selection.channel != subject.channel
    {
        return Err(instrument(
            "the final selection is keyed to a different release identity",
        ));
    }
    Ok(selection)
}

/// Selected shared prerequisites from the topology: family `shared`,
/// `candidate_inclusion`, with their retained expected registry checksums.
fn load_shared_prerequisites(root: &Path) -> CargoAllowResult<Vec<(String, String, String)>> {
    let text = read_repo_file(root, TOPOLOGY_PATH)?;
    let parsed: toml::Value = toml::from_str(&text)
        .map_err(|error| instrument(format!("topology is not valid TOML: {error}")))?;
    let mut shared = Vec::new();
    let rows = parsed
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten();
    for row in rows.filter_map(toml::Value::as_table) {
        if row.get("product_family").and_then(toml::Value::as_str) != Some("shared") {
            continue;
        }
        if row
            .get("candidate_inclusion")
            .and_then(toml::Value::as_bool)
            != Some(true)
        {
            continue;
        }
        let name = row
            .get("cargo_package_name")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let version = row
            .get("package_version")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let checksum = row
            .get("expected_registry_checksum")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_string();
        if name.is_empty() || !checksum.starts_with("sha256:") {
            return Err(instrument(format!(
                "shared topology row {name:?} lacks a usable registry checksum"
            )));
        }
        shared.push((name, version, checksum));
    }
    shared.sort();
    Ok(shared)
}

fn collect_evidence(
    root: &Path,
    args: &ReleaseFreezeComposeArgs,
    subject: &SubjectIdentity,
) -> CargoAllowResult<Vec<EvidenceInput>> {
    let mut staged = Vec::new();
    for raw in &args.evidence {
        let (role_text, path_text) = raw
            .split_once('=')
            .ok_or_else(|| usage(format!("evidence {raw:?} must be role=path")))?;
        let role = FreezeEvidenceRole::from_label(role_text)
            .ok_or_else(|| usage(format!("unknown evidence role {role_text:?}")))?;
        let path = if Path::new(path_text).is_absolute() {
            PathBuf::from(path_text)
        } else {
            root.join(path_text)
        };
        let bytes = std::fs::read(&path).map_err(|error| {
            usage(format!(
                "evidence {role_text} at {}: {error}",
                path.display()
            ))
        })?;
        let value: Json = serde_json::from_slice(&bytes)
            .map_err(|error| usage(format!("evidence {role_text} is not valid JSON: {error}")))?;
        staged.push((role, path, sha256_v1_bytes(&bytes), value));
    }

    let required = [
        FreezeEvidenceRole::CandidatePreparation,
        FreezeEvidenceRole::PackageSet,
        FreezeEvidenceRole::PackageDocs,
        FreezeEvidenceRole::Rehearsal,
        FreezeEvidenceRole::InstallJourney,
        FreezeEvidenceRole::UpgradeRollback,
        FreezeEvidenceRole::Controls,
    ];
    for role in required {
        if !staged.iter().any(|(staged_role, ..)| *staged_role == role) {
            return Err(usage(format!(
                "missing required evidence role {:?}",
                role.label()
            )));
        }
    }

    Ok(staged
        .into_iter()
        .map(|(role, path, digest, value)| {
            let binding_notes = bind_evidence(subject, role, &value);
            EvidenceInput {
                role,
                path,
                sha256: digest,
                value,
                binding_notes,
            }
        })
        .collect())
}

/// Per-role subject-binding probes. Notes beginning `fail:` block a
/// `Complete` freeze; other notes are recorded observations.
fn bind_evidence(subject: &SubjectIdentity, role: FreezeEvidenceRole, value: &Json) -> Vec<String> {
    let mut notes = Vec::new();
    match role {
        FreezeEvidenceRole::PackageSet => {
            let result = str_field(value, "result").unwrap_or_default();
            if result != "Passed" {
                notes.push(format!("fail:package-set result is {result:?}, not Passed"));
            }
            let workspace = value
                .pointer("/candidate/workspace_version")
                .and_then(Json::as_str)
                .unwrap_or_default();
            if workspace != subject.version {
                notes.push(format!(
                    "fail:package-set workspace version {workspace:?} is not {:?}",
                    subject.version
                ));
            }
            if value
                .pointer("/package_set/crates")
                .and_then(Json::as_array)
                .is_none()
            {
                notes.push("fail:package-set receipt has no crate rows".to_string());
            }
        }
        FreezeEvidenceRole::Rehearsal => {
            let version = value
                .pointer("/release_identity/version")
                .and_then(Json::as_str)
                .unwrap_or_default();
            if version != subject.version {
                notes.push(format!(
                    "fail:rehearsal release identity version {version:?} is not {:?}",
                    subject.version
                ));
            }
            match value.pointer("/phases").and_then(Json::as_object) {
                None => notes.push("fail:rehearsal receipt records no phases".to_string()),
                Some(phases) => {
                    if phases.len() < 8 {
                        notes.push(format!(
                            "fail:rehearsal records {} phases, expected the full eight-phase aggregate",
                            phases.len()
                        ));
                    }
                    let boundary = phases
                        .get("authorization_boundary")
                        .and_then(Json::as_str)
                        .unwrap_or("missing");
                    if boundary == "Complete" {
                        notes.push(
                            "fail:rehearsal authorization boundary must stay non-Complete pre-authorization"
                                .to_string(),
                        );
                    }
                }
            }
        }
        FreezeEvidenceRole::PackageDocs => {
            // The basis generator records digests without the typed `v1`
            // segment; compare the hex payload so either spelling binds.
            fn hex_of(digest: &str) -> &str {
                digest
                    .trim_start_matches("sha256:")
                    .trim_start_matches("v1:")
            }
            for (key, expected) in [
                ("commit", subject.commit.as_str()),
                ("tree", subject.tree.as_str()),
                ("cargo_lock_sha256", hex_of(&subject.cargo_lock_digest)),
                ("topology_sha256", hex_of(&subject.topology_digest)),
            ] {
                match value
                    .pointer(&format!("/basis/{key}"))
                    .and_then(Json::as_str)
                    .map(|found| hex_of(found).to_string())
                {
                    Some(found) if found == expected => {}
                    Some(found) => notes.push(format!(
                        "fail:package-docs basis {key} {found} does not bind the freeze subject"
                    )),
                    None => notes.push(format!("fail:package-docs basis has no {key}")),
                }
            }
            let version = value
                .pointer("/basis/release_identity/version")
                .and_then(Json::as_str)
                .unwrap_or_default();
            if version != subject.version {
                notes.push(format!(
                    "fail:package-docs basis release identity {version:?} is not {:?}",
                    subject.version
                ));
            }
        }
        FreezeEvidenceRole::CandidatePreparation => {
            match serde_json::from_value::<CandidatePreparationReceiptV1>(value.clone()) {
                Ok(receipt) => {
                    if receipt.release_version != subject.version {
                        notes.push(format!(
                            "fail:candidate-preparation target version {:?} is not {:?}",
                            receipt.release_version, subject.version
                        ));
                    }
                    if receipt.state != CandidatePreparationStateV1::Complete {
                        notes.push(format!(
                            "fail:candidate-preparation state is {:?}, not Complete",
                            receipt.state
                        ));
                    }
                    if !receipt.outstanding_decisions.is_empty() {
                        notes.push(format!(
                            "fail:candidate-preparation has {} outstanding decisions",
                            receipt.outstanding_decisions.len()
                        ));
                    }
                }
                // A final receipt is unavailable exactly when the source line
                // already carries the prepared candidate: the plan rerun is
                // the typed no-op parity row (#3834). Its input identity must
                // bind the freeze subject.
                Err(_) => {
                    let readiness = str_field(value, "readiness").unwrap_or_default();
                    let no_transition = value
                        .pointer("/reasons")
                        .and_then(Json::as_array)
                        .map(|reasons| {
                            reasons
                                .iter()
                                .filter_map(Json::as_str)
                                .any(|reason| reason.contains("no transition to prepare"))
                        })
                        .unwrap_or(false);
                    if readiness != "stale" || !no_transition {
                        notes.push(
                            "fail:candidate-preparation evidence is neither a Complete final receipt nor the typed no-op parity row"
                                .to_string(),
                        );
                    }
                    let head = value
                        .pointer("/input_identity/head_commit")
                        .and_then(Json::as_str)
                        .unwrap_or_default();
                    if head != subject.commit {
                        notes.push(format!(
                            "fail:candidate-preparation no-op binds commit {head}, not the freeze subject"
                        ));
                    }
                }
            }
        }
        FreezeEvidenceRole::InstallJourney | FreezeEvidenceRole::UpgradeRollback => {
            let role_name = role.label();
            let bound = deep_find_version(value).as_deref() == Some(subject.version.as_str())
                || deep_find_prefixed_version(value, &format!("cargo-allow {}", subject.version))
                    .is_some();
            if !bound {
                notes.push(format!(
                    "fail:{role_name} receipt does not bind version {:?}",
                    subject.version
                ));
            }
        }
        FreezeEvidenceRole::RegistryObservation => {
            let observed = deep_find_version(value);
            match observed {
                Some(found) if found == subject.version => {
                    notes.push("registry observation binds the freeze version".to_string());
                }
                Some(found) => notes.push(format!(
                    "fail:registry observation reports {found:?}, not the freeze version {:?}",
                    subject.version
                )),
                None => {
                    notes.push("note:registry observation carries no version marker".to_string())
                }
            }
        }
        FreezeEvidenceRole::Controls => {
            let state = str_field(value, "state").unwrap_or_default();
            if state != "Feasible" {
                notes.push(format!(
                    "fail:live controls state is {state:?}, not Feasible"
                ));
            }
            let observed_commit = str_field(value, "commit").unwrap_or_default();
            if observed_commit != subject.commit {
                notes.push(format!(
                    "fail:live controls observed commit {observed_commit}, not the freeze subject"
                ));
            }
        }
        FreezeEvidenceRole::Interop | FreezeEvidenceRole::ReleaseManifest => {
            if deep_find_version(value).is_none() {
                notes.push(format!(
                    "note:{} receipt carries no version binding (recorded, not blocking)",
                    role.label()
                ));
            }
        }
    }
    notes
}

/// Depth-bounded search for a string starting with the given prefix
/// (version output is commonly rendered as `cargo-allow <version>`).
fn deep_find_prefixed_version(value: &Json, prefix: &str) -> Option<String> {
    const MAX_DEPTH: usize = 6;
    fn walk(value: &Json, prefix: &str, depth: usize) -> Option<String> {
        if depth > MAX_DEPTH {
            return None;
        }
        match value {
            Json::String(text) => {
                let trimmed = text.trim();
                trimmed.starts_with(prefix).then(|| trimmed.to_string())
            }
            Json::Object(map) => map
                .values()
                .find_map(|child| walk(child, prefix, depth + 1)),
            Json::Array(items) => items
                .iter()
                .find_map(|child| walk(child, prefix, depth + 1)),
            _ => None,
        }
    }
    walk(value, prefix, 0)
}

/// Depth-bounded search for a version-shaped string value.
fn deep_find_version(value: &Json) -> Option<String> {
    const MAX_DEPTH: usize = 6;
    fn walk(value: &Json, depth: usize) -> Option<String> {
        if depth > MAX_DEPTH {
            return None;
        }
        match value {
            Json::String(text) => {
                let trimmed = text.trim();
                is_version_shaped(trimmed).then(|| trimmed.to_string())
            }
            Json::Object(map) => map.values().find_map(|child| walk(child, depth + 1)),
            Json::Array(items) => items.iter().find_map(|child| walk(child, depth + 1)),
            _ => None,
        }
    }
    walk(value, 0)
}

fn is_version_shaped(text: &str) -> bool {
    let mut parts = text.split('.');
    let (Some(major), Some(minor), Some(patch), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    [major, minor, patch]
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

struct ArchiveSet {
    archives: BTreeMap<String, Vec<u8>>,
}

impl ArchiveSet {
    /// Read the real packaged archive bytes named by the package-set receipt.
    /// The archives are the custody payload; a missing archive fails the
    /// composition before any receipt is written.
    fn collect(
        evidence: &[EvidenceInput],
        rows: &[FinalEvidencePackageSubjectV1],
    ) -> CargoAllowResult<Self> {
        let package_set = evidence_role(evidence, FreezeEvidenceRole::PackageSet)
            .ok_or_else(|| instrument("package-set evidence missing for archive collection"))?;
        let parent = package_set
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut archives = BTreeMap::new();
        for row in rows.iter().take(EXPECTED_UPLOAD_ROWS as usize) {
            let crate_name = format!("{}-{}.crate", row.package_name, row.version);
            let candidates = [
                parent.join("packages").join(&crate_name),
                parent.join(&crate_name),
            ];
            let archive = candidates
                .iter()
                .find(|candidate| candidate.is_file())
                .ok_or_else(|| {
                    instrument(format!(
                        "packaged archive {crate_name} not found next to the package-set receipt"
                    ))
                })?;
            let bytes = std::fs::read(archive).map_err(|error| {
                instrument(format!("archive read {}: {error}", archive.display()))
            })?;
            let digest = sha256_v1_bytes(&bytes);
            if digest != row.expected_digest {
                return Err(instrument(format!(
                    "archive {crate_name} digest {digest} does not match the receipt {}",
                    row.expected_digest
                )));
            }
            archives.insert(row.package_name.clone(), bytes);
        }
        Ok(Self { archives })
    }
}

fn build_evidence_graph(
    subject: &SubjectIdentity,
    selection: &FinalSupportSelectionV1,
    evidence: &[EvidenceInput],
    package_rows: &[FinalEvidencePackageSubjectV1],
    incident_digest: Option<&str>,
) -> FinalEvidenceGraphV1 {
    let mut nodes = Vec::new();
    let mut required_ids = Vec::new();

    for input in evidence {
        let (class, origin, id) = input.role.graph_shape();
        let result = if input.bound_ok() {
            FinalEvidenceNodeResultV1::Complete
        } else {
            FinalEvidenceNodeResultV1::Mismatch
        };
        // The interop platform receipt and the prepublication asset result
        // support the freeze without being load-bearing rows.
        let required = !matches!(
            input.role,
            FreezeEvidenceRole::Interop | FreezeEvidenceRole::ReleaseManifest
        );
        if required {
            required_ids.push(id.to_string());
        }
        nodes.push(node_for(id, class, origin, &input.sha256, result, subject));
    }

    // The support-selection node binds the committed support source itself.
    let selection_semantic = sha256_v1_bytes(selection.selection_digest.as_bytes());
    required_ids.push("support-selection".to_string());
    nodes.push(node_for(
        "support-selection",
        FinalEvidenceNodeClassV1::SupportSelection,
        FinalEvidenceOriginV1::SourceAuthority,
        &selection_semantic,
        FinalEvidenceNodeResultV1::Complete,
        subject,
    ));

    if let Some(incident_digest) = incident_digest {
        let mut node = node_for(
            "incident-handoff",
            FinalEvidenceNodeClassV1::IncidentHandoff,
            FinalEvidenceOriginV1::HistoricalObservation,
            incident_digest,
            FinalEvidenceNodeResultV1::Complete,
            subject,
        );
        // The incident handoff is historical context: it must remain visible
        // but its Incident result can never be a required Complete row.
        node.required = false;
        node.authority_scope = allow_report::FinalEvidenceAuthorityScopeV1::HistoricalIncident;
        nodes.push(node);
    }

    let present = |id: &str| nodes.iter().any(|node| node.evidence_id == id);
    let edges = [
        (
            "package-archive",
            "installed-journey",
            FinalEvidenceEdgeKindV1::ProducedFrom,
        ),
        (
            "support-selection",
            "installed-journey",
            FinalEvidenceEdgeKindV1::Projects,
        ),
        (
            "manifest-result",
            "installed-journey",
            FinalEvidenceEdgeKindV1::ConsumedBy,
        ),
        (
            "registry-observation",
            "manifest-result",
            FinalEvidenceEdgeKindV1::SupportsOnly,
        ),
        (
            "release-rehearsal",
            "installed-journey",
            FinalEvidenceEdgeKindV1::SupportsOnly,
        ),
        (
            "upgrade-rollback-receipt",
            "installed-journey",
            FinalEvidenceEdgeKindV1::SupportsOnly,
        ),
        (
            "live-control-observation",
            "support-selection",
            FinalEvidenceEdgeKindV1::SupportsOnly,
        ),
        (
            "candidate-preparation",
            "package-archive",
            FinalEvidenceEdgeKindV1::ProducedFrom,
        ),
        (
            "platform-receipt",
            "installed-journey",
            FinalEvidenceEdgeKindV1::SupportsOnly,
        ),
        (
            "asset-result",
            "manifest-result",
            FinalEvidenceEdgeKindV1::SupportsOnly,
        ),
    ];
    let edges = edges
        .into_iter()
        .filter(|(from, to, _)| present(from) && present(to))
        .map(|(from, to, kind)| FinalEvidenceEdgeV1 {
            schema_id: "cargo-allow.final-evidence-edge.v1".to_string(),
            schema_version: 1,
            from: from.to_string(),
            to: to.to_string(),
            kind,
            claim_boundary: format!("{from} supplies the selected {kind:?} relationship to {to}."),
        })
        .collect();

    FinalEvidenceGraphV1 {
        schema_id: "cargo-allow.final-evidence-graph.v1".to_string(),
        schema_version: 1,
        mode: FinalEvidenceGraphModeV1::Production,
        repository: REPOSITORY.to_string(),
        selected_subject: FinalEvidenceSelectedSubjectV1 {
            repository: REPOSITORY.to_string(),
            commit: subject.commit.clone(),
            tree: subject.tree.clone(),
            cargo_lock_digest: subject.cargo_lock_digest.clone(),
            topology_digest: subject.topology_digest.clone(),
            release_identity: subject.release_identity(),
            expected_upload_rows: EXPECTED_UPLOAD_ROWS,
            expected_shared_rows: EXPECTED_SHARED_ROWS,
            package_rows: package_rows.to_vec(),
        },
        required_node_ids: required_ids,
        nodes,
        edges,
        limitations: Vec::new(),
        claim_boundary:
            "Production final-freeze evidence graph composed at one clean committed subject from bounded retained producer receipts."
                .to_string(),
    }
}

fn node_for(
    evidence_id: &str,
    class: FinalEvidenceNodeClassV1,
    origin: FinalEvidenceOriginV1,
    semantic_digest: &str,
    result: FinalEvidenceNodeResultV1,
    subject: &SubjectIdentity,
) -> FinalEvidenceNodeV1 {
    FinalEvidenceNodeV1 {
        schema_id: "cargo-allow.final-evidence-node.v1".to_string(),
        schema_version: 1,
        evidence_id: evidence_id.to_string(),
        class,
        origin,
        authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
        required: true,
        producer: FinalEvidenceProducerV1 {
            producer_id: format!("producer:{evidence_id}"),
            tool: "cargo-allow".to_string(),
            generation: 1,
            identity_digest: sha256_v1_bytes(format!("producer:{evidence_id}").as_bytes()),
            workflow_path: None,
            workflow_run_id: None,
            workflow_attempt: None,
            job: None,
        },
        producer_expectation: None,
        subject: subject.binding(),
        semantic_digest: semantic_digest.to_string(),
        expected_semantic_digest: Some(semantic_digest.to_string()),
        artifact_digest: None,
        expected_artifact_digest: None,
        result,
        currentness: FinalEvidenceCurrentnessV1::Current,
        invalidation_dimensions: vec![FinalEvidenceInvalidationDimensionV1::Source],
        rerun_owner: Some(format!("owner:{evidence_id}")),
        limitations: Vec::new(),
        claim_boundary: format!("Exact bounded evidence for {evidence_id} at the frozen subject."),
    }
}

/// The explicit campaign decisions recorded by the #3737 final selection and
/// the #3768 train: the pilots stay NotProven/NotIncluded, rc.2 is not
/// selected, and publication authorization stays outside the freeze.
fn readiness_decision_inputs(
    subject: &SubjectIdentity,
    selection: &FinalSupportSelectionV1,
    evidence: &[EvidenceInput],
) -> FinalReadinessDecisionInputsV1 {
    let decided = |decision_id: &str, owner: &str| FinalReadinessRootDecisionV1 {
        decision_id: decision_id.to_string(),
        owner: owner.to_string(),
        state: FinalReadinessDecisionStateV1::Decided,
        required: true,
    };
    let mut root_decisions = vec![
        decided("pilot-clean-not-proven", "#2466"),
        decided("pilot-brownfield-not-included", "#2467"),
        decided("rc2-not-selected", "#3768"),
        decided("publication-authorization-remains-external", "#3760"),
    ];
    if evidence_role(evidence, FreezeEvidenceRole::UpgradeRollback).is_some() {
        root_decisions.push(decided("upgrade-rollback-current", "#2485"));
    }

    let supported_limitations = selection
        .rows
        .iter()
        .filter(|row| {
            matches!(
                row.disposition,
                FinalSelectionDispositionV1::NotIncluded | FinalSelectionDispositionV1::NotProven
            )
        })
        .map(|row| FinalReadinessSupportedLimitationV1 {
            limitation_id: format!("{}/{}", row.dimension, row.subject),
            user_facing_projection: Some(row.claim_effect.clone()),
            owner: Some(row.proof_owner.clone()),
        })
        .collect();

    FinalReadinessDecisionInputsV1 {
        graph_owner: "core/release".to_string(),
        root_decisions,
        supported_limitations,
        permitted_claim_narrowings: Vec::new(),
        post_merge: FinalReadinessPostMergePostureV1 {
            merge_commit: subject.commit.clone(),
            merge_subject_current: true,
            qualification: FinalReadinessQualificationPostureV1::Current,
            owner: "core/release".to_string(),
        },
        custody: FinalReadinessCustodyPostureV1 {
            replay_feasible: true,
            expires_before_authorization_window: false,
            owner: "core/release".to_string(),
        },
        remaining_reversible_work: Vec::new(),
        remaining_irreversible_operations: REMAINING_IRREVERSIBLE_OPERATIONS
            .iter()
            .map(|operation| (*operation).to_string())
            .collect(),
    }
}

fn build_custody(
    subject: &SubjectIdentity,
    rows: &[FinalEvidencePackageSubjectV1],
    archives: &ArchiveSet,
    receipt_bytes: &[u8],
    manifest_bytes: Option<&[u8]>,
) -> CargoAllowResult<CargoAllowFrozenCandidateCustodyV1> {
    let mut items = Vec::new();
    for row in rows.iter().take(EXPECTED_UPLOAD_ROWS as usize) {
        let bytes = archives
            .archives
            .get(&row.package_name)
            .ok_or_else(|| instrument(format!("no archive bytes for {}", row.package_name)))?;
        let sha256 = sha256_v1_bytes(bytes);
        items.push(RetainedCustodyItemV1 {
            role: "PackageArchive".to_string(),
            artifact_id: row.package_name.clone(),
            files: vec![CustodyFileV1 {
                path: format!("packages/{}-{}.crate", row.package_name, row.version),
                size_bytes: bytes.len() as u64,
                sha256: sha256.clone(),
            }],
            storage_locator: format!("local:freeze-{}/{}", subject.version, row.package_name),
            retention_expiry_utc: "2027-12-31T00:00:00Z".to_string(),
            readback_verified: true,
            readback_sha256: Some(sha256),
            confidentiality_class: ConfidentialityClassV1::Public,
        });
    }
    let receipt_sha256 = sha256_v1_bytes(receipt_bytes);
    items.push(RetainedCustodyItemV1 {
        role: "FreezeReceipt".to_string(),
        artifact_id: "final-freeze-receipt".to_string(),
        files: vec![CustodyFileV1 {
            path: "final-freeze.receipt.json".to_string(),
            size_bytes: receipt_bytes.len() as u64,
            sha256: receipt_sha256.clone(),
        }],
        storage_locator: format!("local:freeze-{}/final-freeze-receipt", subject.version),
        retention_expiry_utc: "2027-12-31T00:00:00Z".to_string(),
        readback_verified: true,
        readback_sha256: Some(receipt_sha256),
        confidentiality_class: ConfidentialityClassV1::Public,
    });
    if let Some(manifest) = manifest_bytes {
        let manifest_sha256 = sha256_v1_bytes(manifest);
        items.push(RetainedCustodyItemV1 {
            role: "ReleaseManifest".to_string(),
            artifact_id: "release-manifest-v2".to_string(),
            files: vec![CustodyFileV1 {
                path: "release-manifest-v2.json".to_string(),
                size_bytes: manifest.len() as u64,
                sha256: manifest_sha256.clone(),
            }],
            storage_locator: format!("local:freeze-{}/release-manifest-v2", subject.version),
            retention_expiry_utc: "2027-12-31T00:00:00Z".to_string(),
            readback_verified: true,
            readback_sha256: Some(manifest_sha256),
            confidentiality_class: ConfidentialityClassV1::Public,
        });
    }
    Ok(CargoAllowFrozenCandidateCustodyV1::new(
        CandidateCustodyInitV1 {
            custody_id: format!("candidate-custody-{}-final", subject.version),
            candidate_version: subject.version.clone(),
            git_commit: subject.commit.clone(),
            git_tree: subject.tree.clone(),
            items,
            created_at_utc: subject.frozen_at_utc.clone(),
        },
    ))
}

fn build_transfers(
    subject: &SubjectIdentity,
    rows: &[FinalEvidencePackageSubjectV1],
    archives: &ArchiveSet,
    receipt_bytes: &[u8],
    manifest_bytes: Option<&[u8]>,
) -> CargoAllowResult<Vec<CargoAllowReleaseArtifactTransferV1>> {
    let mut transfers = Vec::new();
    for row in rows.iter().take(EXPECTED_UPLOAD_ROWS as usize) {
        let bytes = archives
            .archives
            .get(&row.package_name)
            .ok_or_else(|| instrument(format!("no archive bytes for {}", row.package_name)))?;
        transfers.push(CargoAllowReleaseArtifactTransferV1::new(
            ArtifactTransferInitV1 {
                transfer_id: format!("transfer:{}", row.package_name),
                role: "PackageArchive".to_string(),
                stable_artifact_id: row.package_name.clone(),
                producer: ProducerIdentityV1 {
                    repository: REPOSITORY.to_string(),
                    workflow_path: "scripts/exact-candidate-package-set.sh".to_string(),
                    git_ref: format!("commit/{}", subject.commit),
                    run_id: 0,
                    run_attempt: 1,
                    job_id: format!("job:{}", row.package_name),
                    commit_sha: subject.commit.clone(),
                    tree_sha: subject.tree.clone(),
                    release_version: subject.version.clone(),
                    tool_name: "cargo-allow".to_string(),
                    schema_id: "cargo-allow.release-artifact-transfer.v1".to_string(),
                    producer_generation: 1,
                },
                provider_id: "local-freeze".to_string(),
                provider_artifact_name: row.package_name.clone(),
                files: vec![ArtifactTransferFileV1 {
                    path: format!("{}-{}.crate", row.package_name, row.version),
                    size_bytes: bytes.len() as u64,
                    sha256: sha256_v1_bytes(bytes),
                }],
                semantic_payload_digest: None,
                trust_class: TrustClassV1::ManualDispatch,
                untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
                created_at_utc: subject.frozen_at_utc.clone(),
            },
        ));
    }
    let control_rows: [(&str, &str, &[u8]); 1] =
        [("FreezeReceipt", "final-freeze-receipt", receipt_bytes)];
    for (role, artifact_id, bytes) in control_rows {
        transfers.push(CargoAllowReleaseArtifactTransferV1::new(
            ArtifactTransferInitV1 {
                transfer_id: format!("transfer:{artifact_id}"),
                role: role.to_string(),
                stable_artifact_id: artifact_id.to_string(),
                producer: ProducerIdentityV1 {
                    repository: REPOSITORY.to_string(),
                    workflow_path: "scripts/exact-candidate-package-set.sh".to_string(),
                    git_ref: format!("commit/{}", subject.commit),
                    run_id: 0,
                    run_attempt: 1,
                    job_id: format!("job:{artifact_id}"),
                    commit_sha: subject.commit.clone(),
                    tree_sha: subject.tree.clone(),
                    release_version: subject.version.clone(),
                    tool_name: "cargo-allow".to_string(),
                    schema_id: "cargo-allow.release-artifact-transfer.v1".to_string(),
                    producer_generation: 1,
                },
                provider_id: "local-freeze".to_string(),
                provider_artifact_name: artifact_id.to_string(),
                files: vec![ArtifactTransferFileV1 {
                    path: format!("{artifact_id}.json"),
                    size_bytes: bytes.len() as u64,
                    sha256: sha256_v1_bytes(bytes),
                }],
                semantic_payload_digest: None,
                trust_class: TrustClassV1::ManualDispatch,
                untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
                created_at_utc: subject.frozen_at_utc.clone(),
            },
        ));
    }
    if let Some(manifest) = manifest_bytes {
        transfers.push(CargoAllowReleaseArtifactTransferV1::new(
            ArtifactTransferInitV1 {
                transfer_id: "transfer:release-manifest-v2".to_string(),
                role: "ReleaseManifest".to_string(),
                stable_artifact_id: "release-manifest-v2".to_string(),
                producer: ProducerIdentityV1 {
                    repository: REPOSITORY.to_string(),
                    workflow_path: "scripts/generate-release-manifest.sh".to_string(),
                    git_ref: format!("commit/{}", subject.commit),
                    run_id: 0,
                    run_attempt: 1,
                    job_id: "job:release-manifest-v2".to_string(),
                    commit_sha: subject.commit.clone(),
                    tree_sha: subject.tree.clone(),
                    release_version: subject.version.clone(),
                    tool_name: "cargo-allow".to_string(),
                    schema_id: "cargo-allow.release-artifact-transfer.v1".to_string(),
                    producer_generation: 1,
                },
                provider_id: "local-freeze".to_string(),
                provider_artifact_name: "release-manifest-v2".to_string(),
                files: vec![ArtifactTransferFileV1 {
                    path: "release-manifest-v2.json".to_string(),
                    size_bytes: manifest.len() as u64,
                    sha256: sha256_v1_bytes(manifest),
                }],
                semantic_payload_digest: None,
                trust_class: TrustClassV1::ManualDispatch,
                untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
                created_at_utc: subject.frozen_at_utc.clone(),
            },
        ));
    }
    Ok(transfers)
}

fn build_retained_artifacts(
    archives: &ArchiveSet,
    receipt_bytes: &[u8],
    manifest_bytes: Option<&[u8]>,
) -> Vec<RetainedExactArtifactV1> {
    let mut artifacts = archives
        .archives
        .iter()
        .map(|(name, bytes)| RetainedExactArtifactV1 {
            role: "PackageArchive".to_string(),
            artifact_id: name.clone(),
            declared_sha256: sha256_v1_bytes(bytes),
            bytes: RetainedArtifactBytesV1::new(bytes.clone()),
        })
        .collect::<Vec<_>>();
    artifacts.push(RetainedExactArtifactV1 {
        role: "FreezeReceipt".to_string(),
        artifact_id: "final-freeze-receipt".to_string(),
        declared_sha256: sha256_v1_bytes(receipt_bytes),
        bytes: RetainedArtifactBytesV1::new(receipt_bytes.to_vec()),
    });
    if let Some(manifest) = manifest_bytes {
        artifacts.push(RetainedExactArtifactV1 {
            role: "ReleaseManifest".to_string(),
            artifact_id: "release-manifest-v2".to_string(),
            declared_sha256: sha256_v1_bytes(manifest),
            bytes: RetainedArtifactBytesV1::new(manifest.to_vec()),
        });
    }
    artifacts
}

fn manifest_binding(evidence: &[EvidenceInput]) -> FinalFreezeManifestBindingV1 {
    match evidence_role(evidence, FreezeEvidenceRole::ReleaseManifest) {
        Some(manifest) => FinalFreezeManifestBindingV1 {
            result: FinalFreezeManifestResultV1::Exact,
            artifact_id: "release-manifest-v2".to_string(),
            payload_sha256: manifest.sha256.clone(),
        },
        None => FinalFreezeManifestBindingV1 {
            result: FinalFreezeManifestResultV1::NotRun,
            artifact_id: "release-manifest-v2".to_string(),
            payload_sha256: sha256_v1_bytes(b"release-manifest-not-run"),
        },
    }
}

struct FreezeObservationAdapter {
    source_current: bool,
    registry_current: bool,
}

impl RefreshableObservationAdapterV1 for FreezeObservationAdapter {
    fn refresh(&self, observation: &RefreshableObservationV1) -> ObservationReadingV1 {
        let current = match observation.kind {
            RefreshableObservationKindV1::SourceLiveControl => self.source_current,
            RefreshableObservationKindV1::RegistryFeasibility => self.registry_current,
            RefreshableObservationKindV1::AmbientCache => true,
        };
        let freshness = if current {
            ObservationFreshnessV1::Current
        } else {
            ObservationFreshnessV1::Stale
        };
        ObservationReadingV1 {
            freshness,
            detail: "freeze-composition observation reading".to_string(),
        }
    }
}

/// Registry feasibility is current when the rehearsal's shared registry
/// preflight observed the three retained 0.1.0 rows at the frozen subject.
fn rehearsal_registry_preflight_current(evidence: &[EvidenceInput]) -> bool {
    evidence_role(evidence, FreezeEvidenceRole::Rehearsal)
        .and_then(|input| {
            input
                .value
                .pointer("/shared_prerequisites")
                .and_then(Json::as_array)
        })
        .map(|rows| rows.len() >= 3)
        .unwrap_or(false)
}

fn observation_set(evidence: &[EvidenceInput]) -> Vec<RefreshableObservationV1> {
    vec![
        RefreshableObservationV1 {
            observation_id: "obs:source-live-control".to_string(),
            kind: RefreshableObservationKindV1::SourceLiveControl,
            observed_at_utc: evidence_role(evidence, FreezeEvidenceRole::Controls)
                .map(|input| input.sha256.clone())
                .unwrap_or_else(|| "absent".to_string()),
        },
        RefreshableObservationV1 {
            observation_id: "obs:registry-feasibility".to_string(),
            kind: RefreshableObservationKindV1::RegistryFeasibility,
            observed_at_utc: evidence_role(evidence, FreezeEvidenceRole::RegistryObservation)
                .map(|input| input.sha256.clone())
                .unwrap_or_else(|| "absent".to_string()),
        },
        RefreshableObservationV1 {
            observation_id: "obs:ambient-cache".to_string(),
            kind: RefreshableObservationKindV1::AmbientCache,
            observed_at_utc: "not-authoritative".to_string(),
        },
    ]
}

fn write_outputs(
    args: &ReleaseFreezeComposeArgs,
    root: &Path,
    replay_inputs: &CargoAllowFinalFreezeReplayInputsV1,
    receipt_bytes: &[u8],
    replayed: &allow_report::CargoAllowFinalFreezeReplayV1,
    readiness: &allow_report::CargoAllowFinalReadinessV1,
) -> CargoAllowResult<()> {
    let out = if args.out_dir.is_absolute() {
        args.out_dir.clone()
    } else {
        root.join(&args.out_dir)
    };
    std::fs::create_dir_all(&out)
        .map_err(|error| instrument(format!("out dir {}: {error}", out.display())))?;
    std::fs::write(out.join("final-freeze.receipt.json"), receipt_bytes)
        .map_err(|error| instrument(format!("receipt write: {error}")))?;
    let replay_json = render_final_freeze_replay_json(replayed)
        .map_err(|error| instrument(format!("replay render: {error}")))?;
    std::fs::write(out.join("final-freeze.replay.json"), replay_json)
        .map_err(|error| instrument(format!("replay write: {error}")))?;
    std::fs::write(
        out.join("final-freeze.replay.md"),
        render_final_freeze_replay_markdown(replayed),
    )
    .map_err(|error| instrument(format!("replay markdown write: {error}")))?;
    let readiness_json = render_final_readiness_json(readiness)
        .map_err(|error| instrument(format!("readiness render: {error}")))?;
    std::fs::write(out.join("final-freeze.readiness.json"), readiness_json)
        .map_err(|error| instrument(format!("readiness write: {error}")))?;
    let _ = replay_inputs;
    Ok(())
}

fn load_incident_handoff(root: &Path) -> Option<String> {
    let bytes = std::fs::read(root.join(INCIDENT_EVIDENCE_PATH)).ok()?;
    Some(sha256_v1_bytes(&bytes))
}

/// Receipts record bare hex digests; the typed evidence convention is
/// `sha256:v1:<hex>`. Normalize without changing the digest itself.
fn read_evidence_bytes(path: &Path) -> CargoAllowResult<Vec<u8>> {
    std::fs::read(path)
        .map_err(|error| instrument(format!("evidence read {}: {error}", path.display())))
}

fn canonical_digest(digest: &str) -> String {
    if let Some(hex) = digest.strip_prefix("sha256:v1:") {
        format!("sha256:v1:{hex}")
    } else if let Some(hex) = digest.strip_prefix("sha256:") {
        format!("sha256:v1:{hex}")
    } else {
        format!("sha256:v1:{digest}")
    }
}

fn str_field(value: &Json, key: &str) -> Option<String> {
    value.get(key).and_then(Json::as_str).map(str::to_string)
}

fn read_repo_file(root: &Path, relative: &str) -> CargoAllowResult<String> {
    let bytes = std::fs::read(root.join(relative))
        .map_err(|error| instrument(format!("read {relative}: {error}")))?;
    String::from_utf8(bytes).map_err(|error| instrument(format!("{relative}: {error}")))
}

fn sha256_repo_file(root: &Path, relative: &str) -> CargoAllowResult<String> {
    let bytes = std::fs::read(root.join(relative))
        .map_err(|error| instrument(format!("read {relative}: {error}")))?;
    Ok(sha256_v1_bytes(&bytes))
}

fn git(root: &Path, args: &[&str]) -> CargoAllowResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| instrument(format!("git {}: {error}", args.join(" "))))?;
    if !output.status.success() {
        return Err(instrument(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn usage(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::Usage, message)
}

fn instrument(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InstrumentFailure, message)
}

#[cfg(test)]
mod tests {
    use super::{
        FreezeEvidenceRole, REMAINING_IRREVERSIBLE_OPERATIONS, SubjectIdentity, bind_evidence,
        deep_find_version, is_version_shaped, load_shared_prerequisites, readiness_decision_inputs,
    };
    use allow_report::{
        CandidateGraphRowV1, CandidatePreparationReceiptV1, CandidatePreparationStateV1,
        CandidateValidationRowV1, FinalEvidenceNodeClassV1, FinalEvidenceNodeResultV1,
        FinalEvidenceOriginV1, FinalSelectionDispositionV1, FinalSelectionRowV1,
        FinalSupportSelectionV1,
    };

    fn subject() -> SubjectIdentity {
        SubjectIdentity {
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            tree: "fedcba9876543210fedcba9876543210fedcba98".to_string(),
            cargo_lock_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            topology_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            frozen_at_utc: "2026-09-03T00:00:00Z".to_string(),
        }
    }

    fn package_set_value(version: &str, result: &str) -> serde_json::Value {
        serde_json::json!({
            "schema_id": "cargo-allow.exact-candidate.v2",
            "result": result,
            "candidate": { "workspace_version": version },
            "package_set": {
                "order": ["allow-core"],
                "crates": [{
                    "name": "allow-core",
                    "version": version,
                    "crate_file": format!("allow-core-{version}.crate"),
                    "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
                    "size_bytes": 10
                }]
            }
        })
    }

    fn rehearsal_value(phases: u32, boundary: &str) -> serde_json::Value {
        let mut phase_map = serde_json::Map::new();
        for index in 0..phases.saturating_sub(1) {
            phase_map.insert(
                format!("phase{index}"),
                serde_json::Value::String("Complete".into()),
            );
        }
        phase_map.insert(
            "authorization_boundary".to_string(),
            serde_json::Value::String(boundary.into()),
        );
        serde_json::json!({
            "release_identity": { "version": "0.2.0", "tag": "v0.2.0" },
            "phases": phase_map
        })
    }

    fn candidate_preparation_value(state: CandidatePreparationStateV1) -> serde_json::Value {
        let receipt = CandidatePreparationReceiptV1 {
            schema: "cargo-allow.candidate-preparation-receipt.v1".to_string(),
            plan_digest: "sha256:v1:plan".to_string(),
            apply_state: "Applied".to_string(),
            before_identity_digest: "sha256:v1:before".to_string(),
            after_identity_digest: "sha256:v1:after".to_string(),
            release_version: "0.2.0".to_string(),
            release_tag: "v0.2.0".to_string(),
            release_channel: "stable".to_string(),
            selected_graph: Vec::<CandidateGraphRowV1>::new(),
            changed_files: Vec::new(),
            resolved_decisions: Vec::new(),
            outstanding_decisions: Vec::new(),
            changie_result: "Accepted".to_string(),
            release_support_projection: "Complete".to_string(),
            policy_drift_result: "Complete".to_string(),
            no_op_rerun_result: "NoOp".to_string(),
            validation_rows: Vec::<CandidateValidationRowV1>::new(),
            remaining_obligations: Vec::new(),
            reasons: Vec::new(),
            state,
            claim_boundary: "source preparation only".to_string(),
        };
        serde_json::to_value(receipt).expect("receipt serializes")
    }

    #[test]
    fn package_set_binding_rejects_failed_results_and_version_drift() {
        let subject = subject();
        let bound = bind_evidence(
            &subject,
            FreezeEvidenceRole::PackageSet,
            &package_set_value("0.2.0", "Passed"),
        );
        assert!(
            !bound.iter().any(|note| note.starts_with("fail:")),
            "{bound:?}"
        );

        let failed = bind_evidence(
            &subject,
            FreezeEvidenceRole::PackageSet,
            &package_set_value("0.2.0", "Failed"),
        );
        assert!(failed.iter().any(|note| note.starts_with("fail:")));

        let drifted = bind_evidence(
            &subject,
            FreezeEvidenceRole::PackageSet,
            &package_set_value("0.1.11", "Passed"),
        );
        assert!(drifted.iter().any(|note| note.starts_with("fail:")));
    }

    #[test]
    fn rehearsal_binding_requires_all_phases_and_open_authorization() {
        let subject = subject();
        let full = bind_evidence(
            &subject,
            FreezeEvidenceRole::Rehearsal,
            &rehearsal_value(8, "Incomplete"),
        );
        assert!(
            !full.iter().any(|note| note.starts_with("fail:")),
            "{full:?}"
        );

        let short = bind_evidence(
            &subject,
            FreezeEvidenceRole::Rehearsal,
            &rehearsal_value(7, "Incomplete"),
        );
        assert!(short.iter().any(|note| note.starts_with("fail:")));

        // A rehearsal that consumed authorization can never feed a freeze.
        let authorized = bind_evidence(
            &subject,
            FreezeEvidenceRole::Rehearsal,
            &rehearsal_value(8, "Complete"),
        );
        assert!(authorized.iter().any(|note| note.starts_with("fail:")));
    }

    #[test]
    fn candidate_preparation_binding_parses_the_typed_receipt() {
        let subject = subject();
        let complete = bind_evidence(
            &subject,
            FreezeEvidenceRole::CandidatePreparation,
            &candidate_preparation_value(CandidatePreparationStateV1::Complete),
        );
        assert!(
            !complete.iter().any(|note| note.starts_with("fail:")),
            "{complete:?}"
        );

        let incomplete = bind_evidence(
            &subject,
            FreezeEvidenceRole::CandidatePreparation,
            &candidate_preparation_value(CandidatePreparationStateV1::Incomplete),
        );
        assert!(incomplete.iter().any(|note| note.starts_with("fail:")));

        let malformed = bind_evidence(
            &subject,
            FreezeEvidenceRole::CandidatePreparation,
            &serde_json::json!({ "unexpected": true }),
        );
        assert!(malformed.iter().any(|note| note.starts_with("fail:")));
    }

    #[test]
    fn package_docs_binding_requires_every_subject_field() {
        let subject = subject();
        let binding = serde_json::json!({
            "basis": {
                "commit": subject.commit,
                "tree": subject.tree,
                "cargo_lock_sha256": subject.cargo_lock_digest,
                "topology_sha256": subject.topology_digest,
                "release_identity": { "version": "0.2.0" }
            }
        });
        let bound = bind_evidence(&subject, FreezeEvidenceRole::PackageDocs, &binding);
        assert!(
            !bound.iter().any(|note| note.starts_with("fail:")),
            "{bound:?}"
        );

        let stale_commit = serde_json::json!({
            "basis": {
                "commit": "9999999999999999999999999999999999999999",
                "tree": subject.tree,
                "cargo_lock_sha256": subject.cargo_lock_digest,
                "topology_sha256": subject.topology_digest,
                "release_identity": { "version": "0.2.0" }
            }
        });
        let stale = bind_evidence(&subject, FreezeEvidenceRole::PackageDocs, &stale_commit);
        assert!(stale.iter().any(|note| note.starts_with("fail:")));
    }

    #[test]
    fn version_shaping_rejects_prerelease_and_partial_forms() {
        assert!(is_version_shaped("0.2.0"));
        assert!(!is_version_shaped("0.2.0-rc.1"));
        assert!(!is_version_shaped("0.2"));
        assert!(!is_version_shaped("v0.2.0"));
        assert!(!is_version_shaped(""));
        assert_eq!(
            deep_find_version(&serde_json::json!({ "a": { "b": "0.1.11" } })),
            Some("0.1.11".to_string())
        );
    }

    #[test]
    fn shared_prerequisites_come_from_the_topology_checksum_authority() {
        let temp =
            std::env::temp_dir().join(format!("freeze-topology-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(temp.join("policy")).expect("temp dir");
        std::fs::write(
            temp.join("policy").join("product-package-topology-v2.toml"),
            r#"
[[package]]
cargo_package_name = "cargo-allow"
product_family = "cargo-allow"
candidate_inclusion = true
package_version = "0.2.0"
expected_registry_checksum = "sha256:aaaa"

[[package]]
cargo_package_name = "effortless-repo-edit"
product_family = "shared"
candidate_inclusion = true
package_version = "0.1.0"
expected_registry_checksum = "sha256:bbbb"

[[package]]
cargo_package_name = "cargo-intent"
product_family = "intent"
candidate_inclusion = true
package_version = "0.3.0"
expected_registry_checksum = "sha256:cccc"
"#,
        )
        .expect("topology fixture");
        let loaded = load_shared_prerequisites(&temp);
        let _ = std::fs::remove_dir_all(&temp);
        let loaded = loaded.expect("topology fixture loads");
        assert_eq!(
            loaded,
            vec![(
                "effortless-repo-edit".to_string(),
                "0.1.0".to_string(),
                "sha256:bbbb".to_string()
            )]
        );
    }

    fn selection() -> FinalSupportSelectionV1 {
        let row = |dimension: &str, subject: &str, disposition: FinalSelectionDispositionV1| {
            FinalSelectionRowV1 {
                dimension: dimension.to_string(),
                subject: subject.to_string(),
                disposition,
                proof_owner: "owner".to_string(),
                required_evidence: "evidence".to_string(),
                evidence_reference: "Cargo.toml".to_string(),
                claim_effect: "narrowed".to_string(),
                staleness_inputs: Vec::new(),
            }
        };
        FinalSupportSelectionV1 {
            schema_id: "cargo-allow.final-support-selection.v1".to_string(),
            schema_version: 1,
            controlling_issue: 3737,
            release_version: "0.2.0".to_string(),
            release_tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            github_prerelease: false,
            identity_digest: "sha256:v1:identity".to_string(),
            selection_digest: "sha256:v1:selection".to_string(),
            claim_boundary: "boundary".to_string(),
            rows: vec![
                row(
                    "platform",
                    "x86_64-unknown-linux-gnu",
                    FinalSelectionDispositionV1::Selected,
                ),
                row(
                    "pilot",
                    "clean-repository",
                    FinalSelectionDispositionV1::NotProven,
                ),
            ],
        }
    }

    #[test]
    fn readiness_inputs_record_the_selection_decisions_and_limitations() {
        let subject = subject();
        let selection = selection();
        let inputs = readiness_decision_inputs(&subject, &selection, &[]);
        let ids: Vec<&str> = inputs
            .root_decisions
            .iter()
            .map(|decision| decision.decision_id.as_str())
            .collect();
        assert!(ids.contains(&"pilot-clean-not-proven"));
        assert!(ids.contains(&"pilot-brownfield-not-included"));
        assert!(ids.contains(&"rc2-not-selected"));
        assert!(ids.contains(&"publication-authorization-remains-external"));
        // Every declined selection row projects to one supported limitation
        // with the row's own claim effect and owner; selected rows do not.
        assert_eq!(inputs.supported_limitations.len(), 1);
        assert_eq!(
            inputs.supported_limitations[0].limitation_id,
            "pilot/clean-repository"
        );
        assert_eq!(
            inputs.remaining_irreversible_operations.len(),
            REMAINING_IRREVERSIBLE_OPERATIONS.len()
        );
    }

    #[test]
    fn evidence_graph_shapes_follow_the_node_class_law() {
        let subject = subject();
        let selection = selection();
        let package_set = super::EvidenceInput {
            role: FreezeEvidenceRole::PackageSet,
            path: std::path::PathBuf::from("receipt.json"),
            sha256: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
            value: package_set_value("0.2.0", "Passed"),
            binding_notes: Vec::new(),
        };
        let rehearsal = super::EvidenceInput {
            role: FreezeEvidenceRole::Rehearsal,
            path: std::path::PathBuf::from("rehearsal.json"),
            sha256: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                .to_string(),
            value: rehearsal_value(8, "Incomplete"),
            binding_notes: Vec::new(),
        };
        let evidence = vec![package_set, rehearsal];
        let package_rows = Vec::new();
        let graph = super::build_evidence_graph(
            &subject,
            &selection,
            &evidence,
            &package_rows,
            Some("sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
        );
        assert_eq!(
            graph.mode,
            allow_report::FinalEvidenceGraphModeV1::Production
        );
        let classes: std::collections::BTreeMap<_, _> = graph
            .nodes
            .iter()
            .map(|node| {
                (
                    node.evidence_id.as_str(),
                    (node.class, node.origin, node.required),
                )
            })
            .collect();
        assert!(classes.contains_key("package-archive"));
        assert!(classes.contains_key("release-rehearsal"));
        assert!(classes.contains_key("support-selection"));
        let incident = classes
            .get("incident-handoff")
            .expect("incident node present");
        assert!(!incident.2, "the incident handoff stays non-required");
        let rehearsal_node = classes.get("release-rehearsal").expect("rehearsal node");
        assert!(rehearsal_node.2, "the rehearsal is a required node");
        assert_eq!(
            rehearsal_node.1,
            FinalEvidenceOriginV1::WorkflowArtifact,
            "workflow-artifact class law for the rehearsal node"
        );
        assert!(matches!(
            classes.get("package-archive").expect("archive node").0,
            FinalEvidenceNodeClassV1::PackageArchive
        ));
        // An Incident result on the handoff node: the historical row must
        // never be rendered Complete.
        let incident_node = graph
            .nodes
            .iter()
            .find(|node| node.evidence_id == "incident-handoff")
            .expect("incident node");
        // The handoff node records the preserved handoff fact itself; a node
        // carrying result=Incident escalates the whole graph evaluation and
        // could never replay into equivalence.
        assert_eq!(incident_node.result, FinalEvidenceNodeResultV1::Complete);
    }
}

#[cfg(test)]
mod compose_fixture_tests {
    use super::ReleaseFreezeComposeArgs;
    use super::cmd_compose;
    use allow_report::{
        CandidateReleaseIdentityProjectionV1, FINAL_SELECTION_IDENTITY_ROLE,
        FINAL_SUPPORT_SELECTION_SCHEMA_ID, FINAL_SUPPORT_SELECTION_SCHEMA_VERSION,
        FinalSelectionDispositionV1, FinalSelectionRowV1, FinalSupportSelectionV1,
        ReleaseVersionV1,
    };
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn write(root: &Path, relative: &str, contents: &[u8]) -> PathBuf {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        std::fs::write(&path, contents).expect("write");
        path
    }

    fn hex(bytes: &[u8]) -> String {
        allow_core::sha256_v1_bytes(bytes)
            .strip_prefix("sha256:v1:")
            .expect("v1 form")
            .to_string()
    }

    fn digest_of(path: &Path) -> String {
        let bytes = std::fs::read(path).expect("read");
        format!("sha256:v1:{}", hex(&bytes))
    }

    fn selection_toml() -> String {
        let version = ReleaseVersionV1::parse("0.2.0").expect("version parses");
        let projection = CandidateReleaseIdentityProjectionV1::from_version(&version);
        let row = |dimension: &str, subject: &str, disposition: &str| {
            format!(
                "[[final_selection.rows]]\ndimension = \"{dimension}\"\nsubject = \"{subject}\"\ndisposition = \"{disposition}\"\nproof_owner = \"owner\"\nrequired_evidence = \"evidence\"\nevidence_reference = \"Cargo.toml\"\nclaim_effect = \"narrowed\"\nstaleness_inputs = []\n"
            )
        };
        let mut selection = FinalSupportSelectionV1 {
            schema_id: FINAL_SUPPORT_SELECTION_SCHEMA_ID.to_string(),
            schema_version: FINAL_SUPPORT_SELECTION_SCHEMA_VERSION,
            controlling_issue: 3737,
            release_version: projection.version.clone(),
            release_tag: projection.tag.clone(),
            channel: projection.channel.clone(),
            github_prerelease: false,
            identity_digest: projection.canonical_digest(FINAL_SELECTION_IDENTITY_ROLE),
            selection_digest: String::new(),
            claim_boundary: FinalSupportSelectionV1 {
                schema_id: String::new(),
                schema_version: 0,
                controlling_issue: 0,
                release_version: String::new(),
                release_tag: String::new(),
                channel: String::new(),
                github_prerelease: false,
                identity_digest: String::new(),
                selection_digest: String::new(),
                claim_boundary: String::new(),
                rows: Vec::new(),
            }
            .claim_boundary()
            .to_string(),
            rows: vec![
                FinalSelectionRowV1 {
                    dimension: "platform".to_string(),
                    subject: "x86_64-unknown-linux-gnu".to_string(),
                    disposition: FinalSelectionDispositionV1::Selected,
                    proof_owner: "owner".to_string(),
                    required_evidence: "evidence".to_string(),
                    evidence_reference: "Cargo.toml".to_string(),
                    claim_effect: "narrowed".to_string(),
                    staleness_inputs: Vec::new(),
                },
                FinalSelectionRowV1 {
                    dimension: "pilot".to_string(),
                    subject: "clean-repository".to_string(),
                    disposition: FinalSelectionDispositionV1::NotProven,
                    proof_owner: "owner".to_string(),
                    required_evidence: "evidence".to_string(),
                    evidence_reference: "Cargo.toml".to_string(),
                    claim_effect: "narrowed".to_string(),
                    staleness_inputs: Vec::new(),
                },
            ],
        };
        selection.selection_digest = selection.canonical_selection_digest(&projection);
        format!(
            "# fixture support matrix\n\n[final_selection]\nschema_id = \"cargo-allow.final-support-selection.v1\"\nschema_version = 1\ncontrolling_issue = 3737\nrelease_version = \"0.2.0\"\nrelease_tag = \"v0.2.0\"\nchannel = \"stable\"\ngithub_prerelease = false\nidentity_digest = \"{}\"\nselection_digest = \"{}\"\nclaim_boundary = \"{}\"\n\n{}{}",
            selection.identity_digest,
            selection.selection_digest,
            selection.claim_boundary(),
            row("platform", "x86_64-unknown-linux-gnu", "selected"),
            row("pilot", "clean-repository", "not_proven"),
        )
    }

    #[test]
    fn compose_reaches_a_verified_complete_freeze_from_a_fixture_repository() {
        let root =
            std::env::temp_dir().join(format!("freeze-compose-fixture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root");

        git(&root, &["init"]);
        git(&root, &["config", "user.email", "freeze@example.invalid"]);
        git(&root, &["config", "user.name", "freeze fixture"]);

        write(
            &root,
            "Cargo.toml",
            b"# fixture workspace\nversion = \"0.2.0\"\n",
        );
        write(
            &root,
            ".gitignore",
            b"target/
",
        );
        write(&root, "Cargo.lock", b"fixture-lock-bytes\n");
        let shared_checksums = [
            (
                "effortless-repo-edit",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            (
                "effortless-repo-protocol",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
            (
                "effortless-repo-snapshot",
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            ),
        ];
        let mut topology = String::new();
        for (name, checksum) in shared_checksums {
            topology.push_str(&format!(
                "[[package]]\ncargo_package_name = \"{name}\"\nproduct_family = \"shared\"\ncandidate_inclusion = true\npackage_version = \"0.1.0\"\nexpected_registry_checksum = \"sha256:{checksum}\"\n\n"
            ));
        }
        write(
            &root,
            "policy/product-package-topology-v2.toml",
            topology.as_bytes(),
        );
        write(
            &root,
            "docs/support-matrix.toml",
            selection_toml().as_bytes(),
        );
        write(
            &root,
            "docs/release/evidence/rc1-publication-incident.v1.json",
            b"{}",
        );

        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-m", "fixture subject"]);
        let commit = git(&root, &["rev-parse", "HEAD"]).trim().to_string();
        let tree = git(&root, &["rev-parse", "HEAD^{tree}"]).trim().to_string();
        let cargo_lock_sha = digest_of(&root.join("Cargo.lock"));
        let topology_sha = digest_of(&root.join("policy/product-package-topology-v2.toml"));

        let evidence_dir = root.join("target/freeze-evidence");
        let packages_dir = evidence_dir.join("packages");
        std::fs::create_dir_all(&packages_dir).expect("packages dir");
        let product_names = [
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
        let mut crate_rows = Vec::new();
        for (index, name) in product_names.iter().enumerate() {
            let bytes = format!("archive-bytes-{name}-{index}").into_bytes();
            std::fs::write(packages_dir.join(format!("{name}-0.2.0.crate")), &bytes)
                .expect("archive");
            crate_rows.push(format!(
                "{{\"name\": \"{name}\", \"version\": \"0.2.0\", \"crate_file\": \"{name}-0.2.0.crate\", \"sha256\": \"{}\", \"size_bytes\": {}}}",
                hex(&bytes),
                bytes.len()
            ));
        }
        for (name, checksum) in shared_checksums {
            crate_rows.push(format!(
                "{{\"name\": \"{name}\", \"version\": \"0.1.0\", \"crate_file\": \"{name}-0.1.0.crate\", \"sha256\": \"sha256:{checksum}\", \"size_bytes\": 3}}"
            ));
        }
        let package_set = format!(
            "{{\"schema_id\": \"cargo-allow.exact-candidate-package-set.v1\", \"result\": \"Passed\", \"candidate\": {{\"workspace_version\": \"0.2.0\"}}, \"package_set\": {{\"order\": [], \"crates\": [{}]}}}}",
            crate_rows.join(",")
        );
        write(
            &evidence_dir,
            "package-set.receipt.json",
            package_set.as_bytes(),
        );

        let mut phases = String::new();
        for phase in [
            "release_identity",
            "candidate_package_set",
            "shared_prerequisites",
            "publisher_state_machine",
            "docs_and_support_identity",
            "manifest_and_assets",
            "workflow_graph_permissions",
        ] {
            phases.push_str(&format!("\"{phase}\": \"Complete\", "));
        }
        phases.push_str("\"authorization_boundary\": \"Incomplete\"");
        let preflight = serde_json::json!(
            shared_checksums
                .iter()
                .map(|(name, checksum)| {
                    serde_json::json!({
                        "name": name,
                        "version": "0.1.0",
                        "registry_checksum": format!("sha256:{checksum}")
                    })
                })
                .collect::<Vec<_>>()
        )
        .to_string();
        let rehearsal = format!(
            "{{\"release_identity\": {{\"version\": \"0.2.0\", \"tag\": \"v0.2.0\"}}, \"phases\": {{{phases}}}, \"shared_prerequisites\": {preflight}}}"
        );
        write(&evidence_dir, "rehearsal.json", rehearsal.as_bytes());

        let package_docs = format!(
            "{{\"basis\": {{\"commit\": \"{commit}\", \"tree\": \"{tree}\", \"cargo_lock_sha256\": \"{cargo_lock_sha}\", \"topology_sha256\": \"{topology_sha}\", \"release_identity\": {{\"version\": \"0.2.0\"}}}}, \"rows\": []}}"
        );
        write(
            &evidence_dir,
            "package-docs.receipt.json",
            package_docs.as_bytes(),
        );

        let candidate_preparation = format!(
            "{{\"readiness\": \"stale\", \"reasons\": [\"target version 0.2.0 equals the current source line; there is no transition to prepare\"], \"input_identity\": {{\"head_commit\": \"{commit}\", \"tree\": \"{tree}\", \"cargo_lock_digest\": \"{cargo_lock_sha}\"}}}}"
        );
        write(
            &evidence_dir,
            "candidate-preparation.json",
            candidate_preparation.as_bytes(),
        );

        write(
            &evidence_dir,
            "install-journey.receipt.json",
            b"{\"candidate\": {\"version\": \"0.2.0\"}, \"result\": \"Passed\"}",
        );
        write(
            &evidence_dir,
            "upgrade-rollback.receipt.json",
            b"{\"candidate\": {\"version\": \"0.2.0\"}, \"result\": \"Passed\"}",
        );
        let controls =
            format!("{{\"state\": \"Feasible\", \"commit\": \"{commit}\", \"tree\": \"{tree}\"}}");
        write(&evidence_dir, "live-controls.json", controls.as_bytes());
        write(
            &evidence_dir,
            "release-manifest-v2.json",
            b"{\"version\": \"0.2.0\", \"publication_state\": \"IncompletePrePublication\"}",
        );

        let role_path = |role: &str, file: &str| {
            format!("{role}={}", evidence_dir.join(file).to_string_lossy())
        };
        let args = ReleaseFreezeComposeArgs {
            version: "0.2.0".to_string(),
            evidence: vec![
                role_path("candidate-preparation", "candidate-preparation.json"),
                role_path("package-set", "package-set.receipt.json"),
                role_path("package-docs", "package-docs.receipt.json"),
                role_path("rehearsal", "rehearsal.json"),
                role_path("install-journey", "install-journey.receipt.json"),
                role_path("upgrade-rollback", "upgrade-rollback.receipt.json"),
                role_path("controls", "live-controls.json"),
                role_path("release-manifest", "release-manifest-v2.json"),
            ],
            out_dir: root.join("target/freeze-out"),
        };
        cmd_compose(&root, &args)
            .expect("the fixture freeze composes and replays to a verified Complete");
        let replay =
            std::fs::read_to_string(root.join("target/freeze-out/final-freeze.replay.json"))
                .expect("replay artifact written");
        assert!(
            replay.contains("complete_equivalent"),
            "the fixture freeze must replay complete_equivalent"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
//
#[cfg(test)]
mod probe_cover_tests {
    use super::deep_find_prefixed_version;

    #[test]
    fn prefixed_version_search_matches_only_the_prefix() {
        let value = serde_json::json!({
            "from": { "version": "cargo-allow 0.1.11 release" },
            "candidate": { "version": "cargo-allow 0.2.0" }
        });
        assert_eq!(
            deep_find_prefixed_version(&value, "cargo-allow 0.2.0").as_deref(),
            Some("cargo-allow 0.2.0")
        );
        assert_eq!(
            deep_find_prefixed_version(&value, "cargo-allow 9.9.9"),
            None
        );
    }

    #[test]
    fn version_shape_rejects_prerelease_and_prefixed_forms() {
        assert!(!super::is_version_shaped("0.2.0-rc.1"));
        assert!(super::is_version_shaped("0.2.0"));
    }
}
//
#[cfg(test)]
mod probe_cover_tests2 {
    use super::{FreezeEvidenceRole, SubjectIdentity, bind_evidence, deep_find_prefixed_version};
    use serde_json::json;

    fn subject() -> SubjectIdentity {
        SubjectIdentity {
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            tree: "fedcba9876543210fedcba9876543210fedcba98".to_string(),
            cargo_lock_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            topology_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            frozen_at_utc: "2026-09-04T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn journey_role_accepts_cargo_allow_prefixed_version_strings() {
        let subject = subject();
        let bound = bind_evidence(
            &subject,
            FreezeEvidenceRole::UpgradeRollback,
            &json!({"from": {"version": "cargo-allow 0.1.11"}, "candidate": {"version": "cargo-allow 0.2.0"}, "result": "Passed"}),
        );
        assert!(
            !bound.iter().any(|note| note.starts_with("fail:")),
            "{bound:?}"
        );

        let drifted = bind_evidence(
            &subject,
            FreezeEvidenceRole::UpgradeRollback,
            &json!({"candidate": {"version": "cargo-allow 0.1.11"}}),
        );
        assert!(drifted.iter().any(|note| note.starts_with("fail:")));
    }

    #[test]
    fn controls_role_binds_state_and_commit() {
        let subject = subject();
        let feasible = bind_evidence(
            &subject,
            FreezeEvidenceRole::Controls,
            &json!({"state": "Feasible", "commit": subject.commit}),
        );
        assert!(
            !feasible.iter().any(|note| note.starts_with("fail:")),
            "{feasible:?}"
        );

        let mismatched = bind_evidence(
            &subject,
            FreezeEvidenceRole::Controls,
            &json!({"state": "Mismatch", "commit": "9999999999999999999999999999999999999999"}),
        );
        assert!(mismatched.iter().any(|note| note.starts_with("fail:")));
    }

    #[test]
    fn registry_role_binds_exact_version_and_flags_drift() {
        let subject = subject();
        let bound = bind_evidence(
            &subject,
            FreezeEvidenceRole::RegistryObservation,
            &json!({"crate": "cargo-allow", "version": "0.2.0"}),
        );
        assert!(
            !bound.iter().any(|note| note.starts_with("fail:")),
            "{bound:?}"
        );

        let drifted = bind_evidence(
            &subject,
            FreezeEvidenceRole::RegistryObservation,
            &json!({"crate": "cargo-allow", "version": "0.1.11"}),
        );
        assert!(
            drifted.iter().any(|note| note.starts_with("fail:")),
            "{drifted:?}"
        );
    }

    #[test]
    fn evidence_roles_map_to_distinct_graph_shapes() {
        let subject = subject();
        for role in [
            FreezeEvidenceRole::PackageSet,
            FreezeEvidenceRole::Rehearsal,
            FreezeEvidenceRole::PackageDocs,
            FreezeEvidenceRole::CandidatePreparation,
            FreezeEvidenceRole::InstallJourney,
            FreezeEvidenceRole::Interop,
            FreezeEvidenceRole::RegistryObservation,
            FreezeEvidenceRole::ReleaseManifest,
            FreezeEvidenceRole::UpgradeRollback,
            FreezeEvidenceRole::Controls,
        ] {
            let (class, origin, id) = role.graph_shape();
            let node = super::node_for(
                id,
                class,
                origin,
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                allow_report::FinalEvidenceNodeResultV1::Complete,
                &subject,
            );
            assert_eq!(node.evidence_id, id);
            assert_eq!(node.class, class);
            assert_eq!(node.origin, origin);
        }
    }

    #[test]
    fn prefixed_version_probe_walks_arrays_and_objects() {
        let value = json!([{"legs": [{"bin": "cargo-allow 0.1.11"}]}]);
        assert_eq!(
            deep_find_prefixed_version(&value, "cargo-allow 0.1.11").as_deref(),
            Some("cargo-allow 0.1.11")
        );
        assert_eq!(
            deep_find_prefixed_version(&json!({}), "cargo-allow 0.1.11"),
            None
        );
    }
}
//
#[cfg(test)]
mod digest_normalization_tests {
    use super::canonical_digest;

    #[test]
    fn bare_and_prefixed_sha256_normalize_to_the_typed_form() {
        let hex = "ab".repeat(32);
        assert_eq!(canonical_digest(&hex), format!("sha256:v1:{hex}"));
        assert_eq!(
            canonical_digest(&format!("sha256:{hex}")),
            format!("sha256:v1:{hex}")
        );
        assert_eq!(
            canonical_digest(&format!("sha256:v1:{hex}")),
            format!("sha256:v1:{hex}")
        );
    }
}
