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
    CandidateCustodyInitV1, CandidatePreparationReceiptV1, CandidatePreparationStateV1,
    CargoAllowFinalFreezeReceiptV1, CargoAllowFinalFreezeReplayInputsV1,
    CargoAllowFrozenCandidateCustodyV1, ConfidentialityClassV1, CustodyFileV1,
    FinalEvidenceAuthorityScopeV1, FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1,
    FinalEvidenceEdgeV1, FinalEvidenceGraphModeV1, FinalEvidenceGraphV1,
    FinalEvidenceInvalidationDimensionV1, FinalEvidenceNodeClassV1, FinalEvidenceNodeResultV1,
    FinalEvidenceNodeV1, FinalEvidenceOriginV1, FinalEvidencePackageRoleV1,
    FinalEvidencePackageSubjectV1, FinalEvidenceProducerV1, FinalEvidenceReleaseIdentityV1,
    FinalEvidenceSelectedSubjectV1, FinalEvidenceSubjectBindingV1, FinalFreezeManifestBindingV1,
    FinalFreezeManifestResultV1, FinalFreezeReceiptInitV1, FinalFreezeReplayResultV1,
    FinalReadinessCustodyPostureV1, FinalReadinessDecisionInputsV1, FinalReadinessDecisionStateV1,
    FinalReadinessPostMergePostureV1, FinalReadinessRootDecisionV1, FinalReadinessRowKindV1,
    FinalReadinessRowV1, FinalReadinessSupportedLimitationV1, FinalReadinessVerdictV1,
    FinalSelectionDispositionV1, FinalSupportSelectionV1, ObservationFreshnessV1,
    ObservationReadingV1, RefreshableObservationAdapterV1, RefreshableObservationKindV1,
    RefreshableObservationV1, ReleaseChannelV1, ReleaseVersionV1, RetainedArtifactBytesV1,
    RetainedCustodyItemV1, RetainedExactArtifactV1, aggregate_final_readiness,
    evaluate_final_evidence_graph, final_evidence_graph_digest, render_final_freeze_replay_json,
    render_final_freeze_replay_markdown, render_final_readiness_json, replay_final_freeze,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::Value as Json;

#[path = "release_freeze_rehearsal.rs"]
mod rehearsal;

#[path = "release_freeze_registry.rs"]
mod registry;

#[path = "release_freeze_qualification.rs"]
mod qualification;

#[path = "release_freeze_experience.rs"]
mod experience;

#[cfg(test)]
#[path = "release_freeze_rehearsal_tests.rs"]
mod rehearsal_tests;

#[cfg(test)]
#[path = "release_freeze_environment_tests.rs"]
mod environment_tests;

#[cfg(test)]
#[path = "release_freeze_registry_tests.rs"]
mod registry_tests;

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
pub(crate) const WORKSPACE_MANIFEST_PATH: &str = "Cargo.toml";
pub(crate) const CARGO_LOCK_PATH: &str = "Cargo.lock";
pub(crate) const TOPOLOGY_PATH: &str = "policy/product-package-topology-v2.toml";
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
    /// Prepare immutable receipt/graph bytes from checked original inputs.
    Prepare {
        #[command(flatten)]
        args: ReleaseFreezeComposeArgs,
        #[arg(long)]
        readback_input: PathBuf,
    },
    /// Compute qualification/custody/replay over a freshly read provider set.
    /// This hidden computation is not a provider attestation.
    Qualify {
        #[command(flatten)]
        args: ReleaseFreezeComposeArgs,
        #[arg(long)]
        readback_input: PathBuf,
    },
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
    /// Original ReleaseExperienceInputV1 selected for required admission.
    #[value(name = "experience-input")]
    ReleaseExperienceInput,
    /// Original CargoAllowReleaseExperienceV1, reconciled against its input.
    #[value(name = "release-experience")]
    ReleaseExperience,
    /// Exact-candidate interop smoke receipt.
    #[value(name = "interop")]
    Interop,
    /// FinalRegistryPreflightInputV1 from the final registry observer.
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
            Self::ReleaseExperienceInput => (
                FinalEvidenceNodeClassV1::CandidateArtifact,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "release-experience-input",
            ),
            Self::ReleaseExperience => (
                FinalEvidenceNodeClassV1::InstalledJourney,
                FinalEvidenceOriginV1::WorkflowArtifact,
                "release-experience",
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
        ReleaseFreezeSubcommand::Prepare {
            args,
            readback_input,
        } => qualification::prepare(&root, args, readback_input),
        ReleaseFreezeSubcommand::Qualify {
            args,
            readback_input,
        } => qualification::qualify(&root, args, readback_input),
    }
}

fn prepare_inputs(
    root: &Path,
    args: &ReleaseFreezeComposeArgs,
    readback: Option<&qualification::ReadbackInput>,
) -> CargoAllowResult<PreparedInputs> {
    let mut subject =
        SubjectIdentity::collect(&mut FilesystemSubjectInputs { root }, &args.version)?;
    if let Some(readback) = readback {
        subject.frozen_at_utc = readback.preparation_time()?;
    }
    let selection = load_selection(root, &subject)?;
    let shared = load_shared_prerequisites(root)?;
    let evidence = collect_evidence(root, args, &subject)?;
    let incident_digest = load_incident_handoff(root);

    // The exact 10+3 package graph must exist before the evidence graph:
    // the graph's selected subject carries the full row set.
    let package_rows = subject.package_rows(&shared, &evidence)?;
    // #3792/#2501 must supply independently verified current registry context,
    // evaluation time, and a selected freshness window. Retained input fields
    // cannot fill this authority gap or make the production freeze Current.
    let registry = registry::reconcile(&subject, &package_rows, &evidence, None);
    let mut graph = build_evidence_graph(
        &subject,
        &selection,
        &evidence,
        &package_rows,
        incident_digest.as_deref(),
        (registry.0, &registry.2),
    );
    if let Some(readback) = readback {
        readback.bind_original_evidence(&subject, &evidence, &mut graph)?;
    }
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

    Ok(PreparedInputs {
        subject,
        selection,
        evidence,
        graph,
        package_rows,
        archives,
        manifest_bytes,
        receipt,
        receipt_bytes,
        registry,
    })
}

struct PreparedInputs {
    subject: SubjectIdentity,
    selection: FinalSupportSelectionV1,
    evidence: Vec<EvidenceInput>,
    graph: FinalEvidenceGraphV1,
    package_rows: Vec<FinalEvidencePackageSubjectV1>,
    archives: ArchiveSet,
    manifest_bytes: Option<Vec<u8>>,
    receipt: CargoAllowFinalFreezeReceiptV1,
    receipt_bytes: Vec<u8>,
    registry: (
        FinalEvidenceNodeResultV1,
        RefreshableObservationV1,
        ObservationReadingV1,
    ),
}

fn cmd_compose(root: &Path, args: &ReleaseFreezeComposeArgs) -> CargoAllowResult<()> {
    let PreparedInputs {
        subject,
        selection,
        evidence,
        graph,
        package_rows,
        archives,
        manifest_bytes,
        receipt,
        receipt_bytes,
        registry,
    } = prepare_inputs(root, args, None)?;
    let evaluation = evaluate_final_evidence_graph(&graph);
    let readiness = readiness_decision_inputs_observed(&subject, &selection, &evidence, None, None)
        .map(|inputs| aggregate_final_readiness(&graph, &inputs));
    let custody = build_custody(
        &subject,
        &package_rows,
        &archives,
        &receipt_bytes,
        manifest_bytes.as_deref(),
    )?;
    let transfers = Vec::new();
    let retained_artifacts =
        build_retained_artifacts(&archives, &receipt_bytes, manifest_bytes.as_deref());

    let replay_inputs = CargoAllowFinalFreezeReplayInputsV1 {
        custody,
        evidence_graph: graph,
        freeze_receipt: receipt,
        retained_transfers: transfers,
        retained_artifacts,
        observations: observation_set(&evidence, registry.1),
        replayed_at_utc: subject.frozen_at_utc.clone(),
    };
    let replayed = replay_final_freeze(
        &replay_inputs,
        &FreezeObservationAdapter {
            source_current: false,
            registry_reading: registry.2,
        },
    );
    let receipt_sha256 = sha256_v1_bytes(&receipt_bytes);

    let graph_complete =
        evaluation.findings.is_empty() && evidence.iter().all(EvidenceInput::bound_ok);
    let complete = graph_complete
        && readiness
            .as_ref()
            .is_ok_and(|value| value.verdict == FinalReadinessVerdictV1::ReadyForFreeze)
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
        readiness_verdict: readiness
            .as_ref()
            .map(|value| format!("{:?}", value.verdict))
            .unwrap_or_else(|_| "Incomplete".to_string()),
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
            .chain(
                readiness_rows(&readiness)
                    .iter()
                    .map(|row| row.message.clone()),
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
                replayed.result, summary.readiness_verdict, replayed.retained_bytes_verified
            ),
        ))
    }
}

/// Acquisition boundary for the single subject-collection orchestration.
pub(crate) trait SubjectInputs {
    fn git(&mut self, args: &[&str]) -> CargoAllowResult<String>;
    fn read_working_bytes(&mut self, path: &str) -> CargoAllowResult<Vec<u8>>;
}

struct FilesystemSubjectInputs<'a> {
    root: &'a Path,
}

impl SubjectInputs for FilesystemSubjectInputs<'_> {
    fn git(&mut self, args: &[&str]) -> CargoAllowResult<String> {
        git(self.root, args)
    }

    fn read_working_bytes(&mut self, path: &str) -> CargoAllowResult<Vec<u8>> {
        std::fs::read(self.root.join(path))
            .map_err(|error| instrument(format!("read {path}: {error}")))
    }
}

/// The exact source subject the freeze binds. Collected from the clean
/// committed HEAD; a dirty worktree is an instrument failure because the
/// packaged archives must come from the committed tree.
#[derive(Debug)]
pub(crate) struct SubjectIdentity {
    pub(crate) version: String,
    tag: String,
    channel: String,
    commit: String,
    tree: String,
    pub(crate) cargo_lock_digest: String,
    pub(crate) topology_digest: String,
    frozen_at_utc: String,
}

impl SubjectIdentity {
    pub(crate) fn collect(
        inputs: &mut impl SubjectInputs,
        version: &str,
    ) -> CargoAllowResult<Self> {
        let dirty = inputs.git(&["status", "--porcelain", "--untracked-files=all"])?;
        if !dirty.trim().is_empty() {
            return Err(instrument(
                "the worktree is dirty; the freeze binds the committed subject only",
            ));
        }
        // Ordinary status cannot see assume-unchanged or skip-worktree
        // edits: flagged files keep a clean status while their working
        // bytes diverge from the committed blob. Reject every hidden
        // index state conservatively instead of pairing the committed
        // identity with unverified working bytes.
        let flags = inputs.git(&["ls-files", "-v", "-z"])?;
        for record in flags.split('\0').filter(|record| !record.is_empty()) {
            let tag = record.chars().next().unwrap_or('?');
            if tag == 'S' || tag.is_ascii_lowercase() {
                return Err(instrument(
                    "the index carries hidden state (assume-unchanged or skip-worktree); the freeze cannot pair committed identity with unverified working bytes",
                ));
            }
        }
        let commit = inputs.git(&["rev-parse", "HEAD"])?;
        let tree = inputs.git(&["rev-parse", "HEAD^{tree}"])?;
        // Defense in depth for the three admitted inputs: read each
        // working file exactly once, verify it against its HEAD blob,
        // and compute the receipt digests from those same verified
        // bytes so no unguarded second read can escape the binding.
        // Line endings are checkout framing, not content.
        let mut verified = std::collections::BTreeMap::new();
        for path in [WORKSPACE_MANIFEST_PATH, CARGO_LOCK_PATH, TOPOLOGY_PATH] {
            let committed = strip_line_endings(&inputs.git(&["show", &format!("HEAD:{path}")])?);
            let raw = inputs.read_working_bytes(path)?;
            let working = strip_line_endings(
                &String::from_utf8(raw.clone())
                    .map_err(|error| instrument(format!("{path}: {error}")))?,
            );
            if committed != working {
                return Err(instrument(format!(
                    "working bytes for {path} differ from the committed subject; the freeze binds the committed bytes only"
                )));
            }
            verified.insert(path, raw);
        }
        let declared = verified_workspace_version(&verified)?;
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
        let cargo_lock_digest =
            sha256_v1_bytes(verified_subject_input(&verified, CARGO_LOCK_PATH)?);
        let topology_digest = sha256_v1_bytes(verified_subject_input(&verified, TOPOLOGY_PATH)?);
        // No evaluation or retention clock can be derived from a source
        // commit's historical author/committer time. The checked producer
        // supplies preparation time explicitly; local diagnosis leaves it absent.
        let frozen_at_utc = String::new();
        // The subject must not move while it is being collected.
        let commit_now = inputs.git(&["rev-parse", "HEAD"])?;
        let tree_now = inputs.git(&["rev-parse", "HEAD^{tree}"])?;
        if commit_now != commit || tree_now != tree {
            return Err(instrument("the subject moved during collection"));
        }
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
        let bytes = if matches!(
            role,
            FreezeEvidenceRole::ReleaseExperienceInput | FreezeEvidenceRole::ReleaseExperience
        ) {
            qualification::read_experience_original(&path)?
        } else {
            std::fs::read(&path).map_err(|error| {
                usage(format!(
                    "evidence {role_text} at {}: {error}",
                    path.display()
                ))
            })?
        };
        let value: Json = if matches!(
            role,
            FreezeEvidenceRole::Rehearsal
                | FreezeEvidenceRole::RegistryObservation
                | FreezeEvidenceRole::ReleaseExperienceInput
                | FreezeEvidenceRole::ReleaseExperience
        ) {
            rehearsal::decode(&bytes)
        } else {
            serde_json::from_slice(&bytes)
        }
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
            // Subject binding (#4175): a same-version rehearsal from a
            // different source subject must not read as current. Each
            // documented subject field is required and compared exactly
            // (digest payloads modulo the typed prefix spelling).
            let commit_sha = value.pointer("/commit_sha").and_then(Json::as_str);
            match commit_sha {
                None => notes.push("fail:rehearsal receipt records no commit_sha".to_string()),
                Some(sha) if sha != subject.commit => notes.push(format!(
                    "fail:rehearsal commit {sha:?} is not the selected subject commit {:?}",
                    subject.commit
                )),
                _ => {}
            }
            for (label, recorded, selected) in [
                (
                    "lockfile",
                    value
                        .pointer("/subject_lockfile_digest")
                        .and_then(Json::as_str),
                    subject.cargo_lock_digest.as_str(),
                ),
                (
                    "topology",
                    value
                        .pointer("/subject_topology_digest")
                        .and_then(Json::as_str),
                    subject.topology_digest.as_str(),
                ),
            ] {
                match (recorded, hex_payload(recorded.unwrap_or(""))) {
                    (None, _) => notes.push(format!(
                        "fail:rehearsal receipt records no subject_{label}_digest"
                    )),
                    (_, None) => notes.push(format!(
                        "fail:rehearsal subject_{label}_digest {recorded:?} is not a documented digest spelling"
                    )),
                    (Some(_), Some(payload)) if payload == hex_payload(selected).unwrap_or(selected) => {}
                    (Some(recorded_digest), _) => notes.push(format!(
                        "fail:rehearsal subject_{label}_digest {recorded_digest:?} is not the selected subject {selected:?}"
                    )),
                }
            }
            notes.extend(rehearsal::binding_notes(value, &subject.tag));
        }
        FreezeEvidenceRole::PackageDocs => {
            // commit/tree are exact identity strings; the two sha256
            // rows go through the strict digest-payload parser.
            let expected: [(&str, &str, bool); 4] = [
                ("commit", subject.commit.as_str(), false),
                ("tree", subject.tree.as_str(), false),
                (
                    "cargo_lock_sha256",
                    hex_payload(&subject.cargo_lock_digest).unwrap_or_default(),
                    true,
                ),
                (
                    "topology_sha256",
                    hex_payload(&subject.topology_digest).unwrap_or_default(),
                    true,
                ),
            ];
            for (key, expected, is_digest) in expected {
                let found = value
                    .pointer(&format!("/basis/{key}"))
                    .and_then(Json::as_str)
                    .map(|found| {
                        if is_digest {
                            hex_payload(found)
                        } else {
                            Some(found)
                        }
                    });
                match found {
                    Some(Some(found)) if found == expected => {}
                    Some(Some(found)) => notes.push(format!(
                        "fail:package-docs basis {key} {found} does not bind the freeze subject"
                    )),
                    _ => notes.push(format!("fail:package-docs basis has no {key}")),
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
            notes.extend(registry::binding_notes(subject, value));
        }
        FreezeEvidenceRole::ReleaseExperienceInput | FreezeEvidenceRole::ReleaseExperience => {
            // Original-pair, referenced-byte and producer admission is separate
            // from these legacy probes. Neither role may acquire Complete from
            // a version substring or an empty binding-note list.
            notes.push("note:required installed-experience admission is evaluated separately".to_string());
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
    registry: (FinalEvidenceNodeResultV1, &ObservationReadingV1),
) -> FinalEvidenceGraphV1 {
    let mut nodes = Vec::new();
    let mut required_ids = Vec::new();

    for input in evidence
        .iter()
        .filter(|input| {
            !matches!(
                input.role,
                FreezeEvidenceRole::RegistryObservation
                    | FreezeEvidenceRole::ReleaseExperienceInput
                    | FreezeEvidenceRole::ReleaseExperience
            )
        })
    {
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
        let mut node = node_for(id, class, origin, &input.sha256, result, subject);
        node.required = required;
        nodes.push(node);
    }

    // Both original experience records are required even when absent. Their
    // computed admission never borrows a caller's Complete flag or a root
    // decision declining a pilot. Readback later binds their actual producers.
    let experience_admission = experience::reconcile(subject, package_rows, evidence, None);
    for role in [
        FreezeEvidenceRole::ReleaseExperienceInput,
        FreezeEvidenceRole::ReleaseExperience,
    ] {
        let (class, origin, id) = role.graph_shape();
        let digest = evidence_role(evidence, role)
            .map(|input| input.sha256.clone())
            .unwrap_or_else(|| sha256_v1_bytes(format!("{id}:absent").as_bytes()));
        required_ids.push(id.to_string());
        nodes.push(node_for(
            id,
            class,
            origin,
            &digest,
            experience_admission.graph_result(),
            subject,
        ));
    }

    // Registry admission is required even when no input was supplied. Its
    // result and currentness come from the typed consumer, never bound_ok or
    // the number of rows in a legacy rehearsal receipt.
    let registry_digest = evidence_role(evidence, FreezeEvidenceRole::RegistryObservation)
        .map(|input| input.sha256.clone())
        .unwrap_or_else(|| sha256_v1_bytes(b"registry-observation-absent"));
    let mut registry_node = node_for(
        "registry-observation",
        FinalEvidenceNodeClassV1::RegistryObservation,
        FinalEvidenceOriginV1::ProviderObservation,
        &registry_digest,
        registry.0,
        subject,
    );
    registry_node.currentness = match registry.1.freshness {
        ObservationFreshnessV1::Current => FinalEvidenceCurrentnessV1::Current,
        ObservationFreshnessV1::Stale => FinalEvidenceCurrentnessV1::Stale,
        ObservationFreshnessV1::Mismatch => FinalEvidenceCurrentnessV1::Mismatch,
        ObservationFreshnessV1::ProviderUnavailable => {
            FinalEvidenceCurrentnessV1::ProviderUnavailable
        }
        ObservationFreshnessV1::InstrumentFailure => FinalEvidenceCurrentnessV1::InstrumentFailure,
    };
    registry_node.invalidation_dimensions = vec![
        FinalEvidenceInvalidationDimensionV1::Source,
        FinalEvidenceInvalidationDimensionV1::PackageBytes,
        FinalEvidenceInvalidationDimensionV1::CargoLock,
        FinalEvidenceInvalidationDimensionV1::Topology,
        FinalEvidenceInvalidationDimensionV1::Workflow,
        FinalEvidenceInvalidationDimensionV1::ProviderObservation,
        FinalEvidenceInvalidationDimensionV1::LiveControls,
    ];
    // Provider diagnostics belong to observation/readiness rows, not support limitations.
    required_ids.push(registry_node.evidence_id.clone());
    nodes.push(registry_node);

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
            "release-experience-input",
            "release-experience",
            FinalEvidenceEdgeKindV1::ProducedFrom,
        ),
        (
            "installed-journey",
            "release-experience",
            FinalEvidenceEdgeKindV1::RequiresCurrent,
        ),
        (
            "support-selection",
            "release-experience",
            FinalEvidenceEdgeKindV1::RequiresCurrent,
        ),
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

    let mut graph = FinalEvidenceGraphV1 {
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
    };
    experience::apply(&mut graph, &experience_admission);
    graph
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

/// Fixture posture for the selection and external-authorization decisions.
/// Pilot applicability is never fabricated here; required experience admission
/// remains a graph obligation independently of supported limitations.
#[cfg(test)]
fn readiness_decision_inputs(
    subject: &SubjectIdentity,
    selection: &FinalSupportSelectionV1,
    evidence: &[EvidenceInput],
) -> FinalReadinessDecisionInputsV1 {
    readiness_decision_inputs_known(
        selection,
        evidence,
        FinalReadinessPostMergePostureV1 {
            merge_commit: subject.commit.clone(),
            merge_subject_current: true,
            qualification: allow_report::FinalReadinessQualificationPostureV1::Current,
            owner: "explicit-unit-fixture".to_string(),
        },
        FinalReadinessCustodyPostureV1 {
            replay_feasible: true,
            expires_before_authorization_window: false,
            owner: "explicit-unit-fixture".to_string(),
        },
    )
}

fn missing_observation(id: &str, message: &str) -> FinalReadinessRowV1 {
    FinalReadinessRowV1 {
        kind: FinalReadinessRowKindV1::MissingEvidence,
        evidence_id: Some(id.to_string()),
        message: message.to_string(),
        owner: "#2501".to_string(),
        next_action: "run the selected authenticated freeze qualifier and retain its exact inputs"
            .to_string(),
    }
}

fn readiness_decision_inputs_observed(
    subject: &SubjectIdentity,
    selection: &FinalSupportSelectionV1,
    evidence: &[EvidenceInput],
    post_merge: Option<FinalReadinessPostMergePostureV1>,
    custody: Option<FinalReadinessCustodyPostureV1>,
) -> Result<FinalReadinessDecisionInputsV1, Vec<FinalReadinessRowV1>> {
    let mut missing = Vec::new();
    if post_merge.is_none() {
        missing.push(missing_observation(
            "post-merge-qualification",
            "independent reviewed/merged qualification and current main readback are absent",
        ));
    }
    if custody.is_none() {
        for (id, message) in [
            (
                "custody-readback",
                "independent immutable artifact readback is absent",
            ),
            (
                "evaluation-clock",
                "the provider-checked evaluation clock is absent",
            ),
            (
                "authorization-window",
                "the independently selected authorization window is absent",
            ),
        ] {
            missing.push(missing_observation(id, message));
        }
    }
    let (Some(post_merge), Some(custody)) = (post_merge, custody) else {
        return Err(missing);
    };
    if post_merge.merge_commit != subject.commit {
        return Err(vec![missing_observation(
            "post-merge-subject",
            "qualification does not select this exact committed subject",
        )]);
    }
    Ok(readiness_decision_inputs_known(
        selection, evidence, post_merge, custody,
    ))
}

fn readiness_decision_inputs_known(
    selection: &FinalSupportSelectionV1,
    evidence: &[EvidenceInput],
    post_merge: FinalReadinessPostMergePostureV1,
    custody: FinalReadinessCustodyPostureV1,
) -> FinalReadinessDecisionInputsV1 {
    let decided = |decision_id: &str, owner: &str| FinalReadinessRootDecisionV1 {
        decision_id: decision_id.to_string(),
        owner: owner.to_string(),
        state: FinalReadinessDecisionStateV1::Decided,
        required: true,
    };
    let mut root_decisions = vec![
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
        post_merge,
        custody,
        remaining_reversible_work: evidence.iter().filter(|input| !input.bound_ok())
            .map(|input| format!("repair exact {} evidence: {}", input.role.label(), input.binding_notes.join("; ")))
            .chain([
                "#3792: obtain independently qualified current registry context and observations".to_string(),
                "#2284: refresh current source/live-control observations through their production owner".to_string(),
            ]).collect(),
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
            storage_locator: String::new(),
            retention_expiry_utc: String::new(),
            readback_verified: false,
            readback_sha256: None,
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
        storage_locator: String::new(),
        retention_expiry_utc: String::new(),
        readback_verified: false,
        readback_sha256: None,
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
            storage_locator: String::new(),
            retention_expiry_utc: String::new(),
            readback_verified: false,
            readback_sha256: None,
            confidentiality_class: ConfidentialityClassV1::Public,
        });
    }
    let mut custody = CargoAllowFrozenCandidateCustodyV1::new(CandidateCustodyInitV1 {
        custody_id: format!("candidate-custody-{}-final", subject.version),
        candidate_version: subject.version.clone(),
        git_commit: subject.commit.clone(),
        git_tree: subject.tree.clone(),
        items,
        created_at_utc: subject.frozen_at_utc.clone(),
    });
    custody.claim_boundary = vec![
        "local_diagnostic_bytes_only".to_string(),
        "provider_retention_not_established".to_string(),
        "independent_readback_not_verified".to_string(),
        "release_authorization_not_granted".to_string(),
    ];
    Ok(custody)
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
    registry_reading: ObservationReadingV1,
}

impl RefreshableObservationAdapterV1 for FreezeObservationAdapter {
    fn refresh(&self, observation: &RefreshableObservationV1) -> ObservationReadingV1 {
        let current = match observation.kind {
            RefreshableObservationKindV1::SourceLiveControl => self.source_current,
            RefreshableObservationKindV1::RegistryFeasibility => {
                return self.registry_reading.clone();
            }
            RefreshableObservationKindV1::AmbientCache => true,
        };
        let freshness = if current {
            ObservationFreshnessV1::Current
        } else {
            ObservationFreshnessV1::ProviderUnavailable
        };
        ObservationReadingV1 {
            freshness,
            detail: "source live-control readback remains independently required (#2284); ambient cache is non-authoritative".to_string(),
        }
    }
}

fn observation_set(
    evidence: &[EvidenceInput],
    registry: RefreshableObservationV1,
) -> Vec<RefreshableObservationV1> {
    vec![
        RefreshableObservationV1 {
            observation_id: "obs:source-live-control".to_string(),
            kind: RefreshableObservationKindV1::SourceLiveControl,
            observed_at_utc: evidence_role(evidence, FreezeEvidenceRole::Controls)
                .map(|input| input.sha256.clone())
                .unwrap_or_else(|| "absent".to_string()),
        },
        registry,
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
    readiness: &Result<allow_report::CargoAllowFinalReadinessV1, Vec<FinalReadinessRowV1>>,
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
    let readiness_json = match readiness {
        Ok(value) => render_final_readiness_json(value),
        Err(_) => serde_json::to_string(&Option::<allow_report::CargoAllowFinalReadinessV1>::None),
    }
    .map_err(|error| instrument(format!("readiness render: {error}")))?;
    std::fs::write(out.join("final-freeze.readiness.json"), readiness_json)
        .map_err(|error| instrument(format!("readiness write: {error}")))?;
    qualification::write_json(
        &out.join("final-freeze.readiness-rows.json"),
        readiness_rows(readiness),
    )?;
    qualification::write_json(&out.join("final-freeze.replay-inputs.json"), replay_inputs)?;
    qualification::write_json(
        &out.join("final-freeze.evidence-graph.json"),
        &replay_inputs.evidence_graph,
    )?;
    qualification::write_json(
        &out.join("final-freeze.custody.json"),
        &replay_inputs.custody,
    )?;
    qualification::write_json(
        &out.join("final-freeze.transfers.json"),
        &replay_inputs.retained_transfers,
    )?;
    Ok(())
}

fn readiness_rows(
    readiness: &Result<allow_report::CargoAllowFinalReadinessV1, Vec<FinalReadinessRowV1>>,
) -> &[FinalReadinessRowV1] {
    match readiness {
        Ok(value) => &value.rows,
        Err(rows) => rows,
    }
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

/// The hex payload of a digest, accepting only the documented
/// spellings — bare 64-char lowercase hex, `sha256:<hex>`, or
/// `sha256:v1:<hex>`. Repeated, partial, or non-hex prefixes are
/// malformed and fail closed instead of binding.
fn hex_payload(digest: &str) -> Option<&str> {
    let payload = digest
        .strip_prefix("sha256:v1:")
        .or_else(|| digest.strip_prefix("sha256:"))
        .unwrap_or(digest);
    (payload.len() == 64
        && payload
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
    .then_some(payload)
}

/// Line endings are not content for the admitted text inputs; the
/// committed-blob comparison normalizes CRLF to LF. Lone carriage
/// returns are content and stay distinct.
fn strip_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Derive identity only from the bytes admitted by the collector. These
/// helpers deliberately have no filesystem capability or repository root.
pub(crate) fn verified_subject_input<'a>(
    verified: &'a BTreeMap<&str, Vec<u8>>,
    relative: &str,
) -> CargoAllowResult<&'a [u8]> {
    verified
        .get(relative)
        .map(Vec::as_slice)
        .ok_or_else(|| instrument(format!("verified input is missing: {relative}")))
}

pub(crate) fn verified_workspace_version(
    verified: &BTreeMap<&str, Vec<u8>>,
) -> CargoAllowResult<String> {
    let manifest = std::str::from_utf8(verified_subject_input(verified, WORKSPACE_MANIFEST_PATH)?)
        .map_err(|error| instrument(format!("{WORKSPACE_MANIFEST_PATH}: {error}")))?;
    let parsed: toml::Value = toml::from_str(manifest).map_err(|error| {
        instrument(format!(
            "{WORKSPACE_MANIFEST_PATH} is not valid TOML: {error}"
        ))
    })?;
    parsed
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            instrument("the workspace manifest must declare workspace.package.version as a string")
        })
}

fn read_repo_file(root: &Path, relative: &str) -> CargoAllowResult<String> {
    let bytes = std::fs::read(root.join(relative))
        .map_err(|error| instrument(format!("read {relative}: {error}")))?;
    String::from_utf8(bytes).map_err(|error| instrument(format!("{relative}: {error}")))
}

/// Discover the selected worktree from cwd through the same isolated Git boundary.
fn git_root() -> CargoAllowResult<PathBuf> {
    let cwd = std::env::current_dir()
        .map_err(|error| instrument(format!("read current directory: {error}")))?;
    let root = git(&cwd, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(root.trim()))
}

fn git(root: &Path, args: &[&str]) -> CargoAllowResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
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

    pub(super) fn subject() -> SubjectIdentity {
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
        rehearsal_value_for(&subject(), phases, boundary)
    }

    fn rehearsal_value_for(
        subject: &SubjectIdentity,
        phases: u32,
        boundary: &str,
    ) -> serde_json::Value {
        let mut phase_map = serde_json::Map::new();
        for phase in [
            "release_identity",
            "candidate_package_set",
            "shared_prerequisites",
            "publisher_state_machine",
            "docs_and_support_identity",
            "manifest_and_assets",
            "workflow_graph_permissions",
        ]
        .into_iter()
        .take(phases.saturating_sub(1) as usize)
        {
            phase_map.insert(
                phase.to_string(),
                serde_json::Value::String("Complete".into()),
            );
        }
        phase_map.insert(
            "authorization_boundary".to_string(),
            serde_json::Value::String(boundary.into()),
        );
        serde_json::json!({
            "schema_version": "1.0",
            "release_identity": { "version": "0.2.0", "tag": "v0.2.0" },
            "phases": phase_map,
            "commit_sha": subject.commit,
            "subject_lockfile_digest": subject.cargo_lock_digest,
            "subject_topology_digest": subject.topology_digest,
            "aggregate_status": "Incomplete",
            "authorization_boundary": {
                "authorization_artifact": "release/authorize-v0.2.0.json",
                "schema": "cargo-allow.release-authorization.v1",
                "named_release": "v0.2.0",
                "candidate_commit": subject.commit,
                "token_present": false,
                "phase_status_note": "authorization remains reserved",
            },
            "zero_mutation_proof": {
                "tag_mutation_prevented": false,
                "token_read_prevented": false,
                "cargo_publish_prevented": false,
                "registry_mutation_prevented": false,
                "github_release_mutation_prevented": false,
                "live_setting_mutation_prevented": false,
                "external_repository_mutation_prevented": false,
            },
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
    fn rehearsal_binding_requires_all_phases_and_open_authorization()
    -> Result<(), Box<dyn std::error::Error>> {
        let subject = subject();
        let full = bind_evidence(
            &subject,
            FreezeEvidenceRole::Rehearsal,
            &rehearsal_value(8, "Incomplete"),
        );
        if full.len() != 7
            || full
                .iter()
                .any(|note| !note.starts_with("fail:rehearsal zero_mutation_proof."))
        {
            return Err(
                format!("characterization must retain its seven proof gaps: {full:?}").into(),
            );
        }

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

        // Subject binding (#4175): a same-version receipt from another
        // source subject must not read as current. Each documented
        // subject field has its own negative control, plus the
        // missing-field case the old helper silently accepted.
        let stranger = bind_evidence(
            &subject,
            FreezeEvidenceRole::Rehearsal,
            &rehearsal_value_for(
                &SubjectIdentity {
                    commit: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
                    version: "0.2.0".to_string(),
                    tag: "v0.2.0".to_string(),
                    channel: "stable".to_string(),
                    tree: "fedcba9876543210fedcba9876543210fedcba98".to_string(),
                    cargo_lock_digest:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_string(),
                    topology_digest:
                        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                            .to_string(),
                    frozen_at_utc: "2026-09-03T00:00:00Z".to_string(),
                },
                8,
                "Incomplete",
            ),
        );
        assert!(
            stranger
                .iter()
                .any(|note| note.contains("is not the selected subject commit")),
            "a foreign commit is rejected: {stranger:?}"
        );
        let mut missing_commit = rehearsal_value_for(&subject, 8, "Incomplete");
        missing_commit
            .as_object_mut()
            .expect("object")
            .remove("commit_sha");
        assert!(
            bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &missing_commit)
                .iter()
                .any(|note| note.contains("records no commit_sha")),
            "a missing commit_sha fails closed"
        );
        let mut wrong_lock = rehearsal_value_for(&subject, 8, "Incomplete");
        wrong_lock.as_object_mut().expect("object").insert(
            "subject_lockfile_digest".to_string(),
            serde_json::json!(
                "sha256:v1:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            ),
        );
        assert!(
            bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &wrong_lock)
                .iter()
                .any(|note| note.contains("subject_lockfile_digest")),
            "a foreign lockfile digest is rejected"
        );
        let mut malformed = rehearsal_value_for(&subject, 8, "Incomplete");
        malformed.as_object_mut().expect("object").insert(
            "subject_lockfile_digest".to_string(),
            serde_json::json!(
                "sha256:sha256:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
        );
        assert!(
            bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &malformed)
                .iter()
                .any(|note| note.contains("is not a documented digest spelling")),
            "a repeated-prefix digest spelling fails closed"
        );
        let mut missing_topology = rehearsal_value_for(&subject, 8, "Incomplete");
        missing_topology
            .as_object_mut()
            .expect("object")
            .remove("subject_topology_digest");
        assert!(
            bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &missing_topology)
                .iter()
                .any(|note| note.contains("records no subject_topology_digest")),
            "a missing topology digest fails closed"
        );
        Ok(())
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

    pub(super) fn selection() -> FinalSupportSelectionV1 {
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
        assert!(!ids.contains(&"pilot-clean-not-proven"));
        assert!(!ids.contains(&"pilot-brownfield-not-included"));
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
        let registry = super::registry::reconcile(&subject, &package_rows, &evidence, None);
        let graph = super::build_evidence_graph(
            &subject,
            &selection,
            &evidence,
            &package_rows,
            Some("sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            (registry.0, &registry.2),
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

    #[test]
    fn evidence_graph_preserves_required_roles_in_both_consumers()
    -> Result<(), Box<dyn std::error::Error>> {
        use allow_report::{
            FinalEvidenceFindingKindV1, aggregate_final_readiness, evaluate_final_evidence_graph,
        };

        let subject = subject();
        let selection = selection();
        for (role, required) in [
            (FreezeEvidenceRole::CandidatePreparation, true),
            (FreezeEvidenceRole::PackageSet, true),
            (FreezeEvidenceRole::PackageDocs, true),
            (FreezeEvidenceRole::Rehearsal, true),
            (FreezeEvidenceRole::InstallJourney, true),
            (FreezeEvidenceRole::Interop, false),
            (FreezeEvidenceRole::RegistryObservation, true),
            (FreezeEvidenceRole::ReleaseManifest, false),
            (FreezeEvidenceRole::UpgradeRollback, true),
            (FreezeEvidenceRole::Controls, true),
        ] {
            // One already-admitted input isolates role classification. This
            // intentionally sparse graph is not complete freeze evidence.
            let evidence = [graph_role_input(role)];
            let registry = super::registry::reconcile(&subject, &[], &evidence, None);
            let mut graph = super::build_evidence_graph(
                &subject,
                &selection,
                &evidence,
                &[],
                None,
                (registry.0, &registry.2),
            );
            // Isolate the consumers' required-node classification from the
            // builder's support-selection edges for journey/control inputs.
            graph.edges.clear();
            let id = role.graph_shape().2;
            let node = graph
                .nodes
                .iter()
                .find(|node| node.evidence_id == id)
                .ok_or("the supplied evidence node is missing")?;
            let evaluation = evaluate_final_evidence_graph(&graph);
            let readiness = aggregate_final_readiness(
                &graph,
                &readiness_decision_inputs(&subject, &selection, &evidence),
            );
            let orphan_required = evaluation.findings.iter().any(|finding| {
                finding.kind == FinalEvidenceFindingKindV1::OrphanRequiredNode
                    && finding.evidence_id.as_deref() == Some(id)
            });
            if node.required != required
                || graph.required_node_ids.iter().any(|value| value == id) != required
                || readiness.required_evidence.iter().any(|row| row.evidence_id == id) != required
                // These inputs have no edges: only required nodes are orphans.
                || orphan_required != required
            {
                return Err(format!(
                    "{role:?} required={required} disagrees across producer and consumers"
                )
                .into());
            }
        }
        Ok(())
    }

    fn graph_role_input(role: FreezeEvidenceRole) -> super::EvidenceInput {
        super::EvidenceInput {
            role,
            path: std::path::PathBuf::from("synthetic-admitted-receipt.json"),
            sha256: allow_core::sha256_v1_bytes(b"synthetic admitted receipt"),
            value: serde_json::json!({}),
            binding_notes: Vec::new(),
        }
    }

    #[test]
    fn evidence_graph_still_validates_supplied_optional_nodes()
    -> Result<(), Box<dyn std::error::Error>> {
        use allow_report::{
            FinalEvidenceFindingKindV1, FinalReadinessRowKindV1, aggregate_final_readiness,
            evaluate_final_evidence_graph,
        };

        let subject = subject();
        let selection = selection();
        for role in [
            FreezeEvidenceRole::Interop,
            FreezeEvidenceRole::ReleaseManifest,
        ] {
            let evidence = [graph_role_input(role)];
            let registry = super::registry::reconcile(&subject, &[], &evidence, None);
            for malformed_schema in [true, false] {
                let mut graph = super::build_evidence_graph(
                    &subject,
                    &selection,
                    &evidence,
                    &[],
                    None,
                    (registry.0, &registry.2),
                );
                let id = role.graph_shape().2;
                let node = graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.evidence_id == id)
                    .ok_or("the supplied optional evidence node is missing")?;
                let expected = if malformed_schema {
                    node.schema_version = 99;
                    FinalEvidenceFindingKindV1::InvalidSchema
                } else {
                    node.semantic_digest = "malformed".to_string();
                    FinalEvidenceFindingKindV1::InvalidDigest
                };
                let evaluation = evaluate_final_evidence_graph(&graph);
                let readiness = aggregate_final_readiness(
                    &graph,
                    &readiness_decision_inputs(&subject, &selection, &evidence),
                );
                if !evaluation.findings.iter().any(|finding| {
                    finding.kind == expected && finding.evidence_id.as_deref() == Some(id)
                }) || !readiness.rows.iter().any(|row| {
                    row.kind == FinalReadinessRowKindV1::MissingEvidence
                        && row.evidence_id.as_deref() == Some(id)
                }) {
                    return Err(format!(
                        "invalid optional {role:?} lost its {expected:?} validation finding"
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    #[test]
    fn rejected_rehearsal_evidence_cannot_become_complete_by_subject_assignment() {
        // A foreign-subject rehearsal receipt binds with fail: notes;
        // the graph node it produces must stay a Mismatch on the
        // required rehearsal row even though node_for stamps the
        // current freeze subject and Current-style provenance onto
        // every node (#4175).
        let subject = subject();
        let stranger_subject = SubjectIdentity {
            commit: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            tree: "fedcba9876543210fedcba9876543210fedcba98".to_string(),
            cargo_lock_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            topology_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            frozen_at_utc: "2026-09-03T00:00:00Z".to_string(),
        };
        let binding_notes = bind_evidence(
            &subject,
            FreezeEvidenceRole::Rehearsal,
            &rehearsal_value_for(&stranger_subject, 8, "Incomplete"),
        );
        assert!(binding_notes.iter().any(|note| note.starts_with("fail:")));

        let rehearsal = super::EvidenceInput {
            role: FreezeEvidenceRole::Rehearsal,
            path: std::path::PathBuf::from("rehearsal.json"),
            sha256: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                .to_string(),
            value: rehearsal_value_for(&stranger_subject, 8, "Incomplete"),
            binding_notes,
        };
        let package_set = super::EvidenceInput {
            role: FreezeEvidenceRole::PackageSet,
            path: std::path::PathBuf::from("receipt.json"),
            sha256: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
            value: package_set_value("0.2.0", "Passed"),
            binding_notes: Vec::new(),
        };
        let evidence = [package_set, rehearsal];
        let registry = super::registry::reconcile(&subject, &[], &evidence, None);
        let graph = super::build_evidence_graph(
            &subject,
            &selection(),
            &evidence,
            &Vec::new(),
            Some("sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            (registry.0, &registry.2),
        );
        let rehearsal_node = graph
            .nodes
            .iter()
            .find(|node| node.evidence_id == "release-rehearsal")
            .expect("rehearsal node");
        assert_eq!(
            rehearsal_node.result,
            FinalEvidenceNodeResultV1::Mismatch,
            "rejected rehearsal evidence stays a Mismatch node"
        );
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

    /// Setup and expected identities must not depend on the production Git helper.
    pub(super) fn fixture_git(
        root: &Path,
        args: &[&str],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut command = Command::new("git");
        command.args(args).current_dir(root);
        let output = crate::repository_environment::isolate_repository(&mut command).output()?;
        if !output.status.success() {
            return Err(format!(
                "fixture git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

    /// Minimal committed subject with explicit checkout framing for its text inputs.
    pub(super) fn committed_subject_fixture() -> Result<PathBuf, Box<dyn std::error::Error>> {
        static NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let nonce = NONCE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("freeze-subject-{}-{nonce}", std::process::id()));
        // A collision is a setup error, never permission to remove an existing directory.
        std::fs::create_dir(&root)?;
        std::fs::create_dir(root.join("policy"))?;

        fixture_git(&root, &["init"])?;
        for (key, value) in [
            ("user.email", "freeze@example.invalid"),
            ("user.name", "freeze fixture"),
            ("core.autocrlf", "false"),
            ("core.eol", "lf"),
            ("core.safecrlf", "false"),
        ] {
            fixture_git(&root, &["config", key, value])?;
        }
        std::fs::write(
            root.join(".gitattributes"),
            b"* text eol=lf\nCargo.lock text eol=crlf\n",
        )?;
        std::fs::write(
            root.join("Cargo.toml"),
            b"[workspace.package]\nversion = \"0.2.0\"\n",
        )?;
        std::fs::write(root.join("Cargo.lock"), b"fixture-lock-bytes\n")?;
        std::fs::write(
            root.join("policy/product-package-topology-v2.toml"),
            b"[[package]]\ncargo_package_name = \"shared\"\n",
        )?;
        fixture_git(&root, &["add", "-A"])?;
        fixture_git(&root, &["commit", "-m", "fixture subject"])?;
        Ok(root)
    }

    #[test]
    fn collect_accepts_a_genuinely_clean_subject() -> Result<(), Box<dyn std::error::Error>> {
        let root = committed_subject_fixture()?;
        let commit = fixture_git(&root, &["rev-parse", "HEAD"])?
            .trim()
            .to_string();
        let tree = fixture_git(&root, &["rev-parse", "HEAD^{tree}"])?
            .trim()
            .to_string();

        let subject = super::SubjectIdentity::collect(
            &mut super::FilesystemSubjectInputs { root: &root },
            "0.2.0",
        )?;
        if subject.commit != commit || subject.tree != tree {
            return Err("clean subject did not retain its committed identity".into());
        }
        if subject.cargo_lock_digest != allow_core::sha256_v1_bytes(b"fixture-lock-bytes\n") {
            return Err("clean subject did not digest the working lock bytes".into());
        }
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn collect_rejects_an_assume_unchanged_hidden_lock() -> Result<(), Box<dyn std::error::Error>> {
        // Ordinary status is empty for this edit; the collector must
        // still reject it before pairing identity with bytes.
        let root = committed_subject_fixture()?;
        fixture_git(&root, &["update-index", "--assume-unchanged", "Cargo.lock"])?;
        std::fs::write(root.join("Cargo.lock"), b"hidden-lock-bytes\n")?;
        if !fixture_git(&root, &["status", "--porcelain"])?
            .trim()
            .is_empty()
        {
            return Err("the hidden-lock fixture did not preserve clean Git status".into());
        }

        let err = super::SubjectIdentity::collect(
            &mut super::FilesystemSubjectInputs { root: &root },
            "0.2.0",
        )
        .err()
        .ok_or("the collector accepted a hidden assume-unchanged edit")?;
        if !err.to_string().contains("hidden state")
            && !err
                .to_string()
                .contains("differ from the committed subject")
        {
            return Err(format!("incorrect hidden-lock rejection: {err}").into());
        }
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn collect_rejects_a_skip_worktree_hidden_topology() -> Result<(), Box<dyn std::error::Error>> {
        let root = committed_subject_fixture()?;
        fixture_git(
            &root,
            &[
                "update-index",
                "--skip-worktree",
                "policy/product-package-topology-v2.toml",
            ],
        )?;
        std::fs::write(
            root.join("policy/product-package-topology-v2.toml"),
            b"[[package]]\ncargo_package_name = \"tampered\"\n",
        )?;
        if !fixture_git(&root, &["status", "--porcelain"])?
            .trim()
            .is_empty()
        {
            return Err("the hidden-topology fixture did not preserve clean Git status".into());
        }

        let err = super::SubjectIdentity::collect(
            &mut super::FilesystemSubjectInputs { root: &root },
            "0.2.0",
        )
        .err()
        .ok_or("the collector accepted a hidden skip-worktree edit")?;
        if !err.to_string().contains("hidden state")
            && !err
                .to_string()
                .contains("differ from the committed subject")
        {
            return Err(format!("incorrect hidden-topology rejection: {err}").into());
        }
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn collect_accepts_crlf_checkout_framing_of_committed_content()
    -> Result<(), Box<dyn std::error::Error>> {
        // A CRLF checkout shows CRLF working bytes over an
        // LF blob: the content is identical and must be accepted, with
        // the receipt digests computed from the exact working bytes.
        let root = committed_subject_fixture()?;
        std::fs::write(root.join("Cargo.lock"), b"fixture-lock-bytes\r\n")?;
        // Refresh the index stat information after changing checkout framing.
        // Text normalization must leave the staged blob identical to HEAD.
        fixture_git(&root, &["add", "--", "Cargo.lock"])?;
        fixture_git(&root, &["diff", "--cached", "--exit-code"])?;

        let working = std::fs::read(root.join("Cargo.lock"))?;
        let committed = fixture_git(&root, &["show", "HEAD:Cargo.lock"])?;
        if !working.ends_with(b"\r\n")
            || committed.as_bytes() != b"fixture-lock-bytes\n"
            || working.as_slice() == committed.as_bytes()
        {
            return Err(format!(
                "CRLF fixture must have distinct CRLF working and LF committed bytes: working={working:?}, committed={committed:?}"
            )
            .into());
        }
        let status = fixture_git(&root, &["status", "--porcelain"])?;
        if !status.trim().is_empty() {
            return Err(format!("CRLF fixture is not a clean checkout: {status}").into());
        }

        let subject = super::SubjectIdentity::collect(
            &mut super::FilesystemSubjectInputs { root: &root },
            "0.2.0",
        )?;
        if subject.cargo_lock_digest != allow_core::sha256_v1_bytes(&working)
            || subject.cargo_lock_digest == allow_core::sha256_v1_bytes(committed.as_bytes())
        {
            return Err("CRLF subject digest must bind working bytes, not the LF blob".into());
        }
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn line_ending_comparison_preserves_lone_carriage_returns()
    -> Result<(), Box<dyn std::error::Error>> {
        for (input, expected) in [
            ("first\r\nsecond\rthird\n", "first\nsecond\rthird\n"),
            ("\r", "\r"),
            ("\n", "\n"),
            ("", ""),
        ] {
            if super::strip_line_endings(input) != expected {
                return Err(format!("incorrect line-ending comparison for {input:?}").into());
            }
        }
        Ok(())
    }

    #[test]
    fn collect_still_rejects_ordinary_dirty_subjects() -> Result<(), Box<dyn std::error::Error>> {
        let root = committed_subject_fixture()?;
        std::fs::write(root.join("Cargo.lock"), b"ordinary-dirty-bytes\n")?;

        let err = super::SubjectIdentity::collect(
            &mut super::FilesystemSubjectInputs { root: &root },
            "0.2.0",
        )
        .err()
        .ok_or("the collector accepted an ordinary dirty worktree")?;
        if !err.to_string().contains("dirty") {
            return Err(format!("incorrect ordinary-dirty rejection: {err}").into());
        }
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn compose_retains_rehearsal_denials_through_graph_readiness_and_replay()
    -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("freeze-compose-fixture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;

        fixture_git(&root, &["init"])?;
        fixture_git(&root, &["config", "user.email", "freeze@example.invalid"])?;
        fixture_git(&root, &["config", "user.name", "freeze fixture"])?;

        write(
            &root,
            "Cargo.toml",
            b"[workspace.package]\nversion = \"0.2.0\"\n",
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

        fixture_git(&root, &["add", "-A"])?;
        fixture_git(&root, &["commit", "-m", "fixture subject"])?;
        let commit = fixture_git(&root, &["rev-parse", "HEAD"])?
            .trim()
            .to_string();
        let tree = fixture_git(&root, &["rev-parse", "HEAD^{tree}"])?
            .trim()
            .to_string();
        let cargo_lock_sha = digest_of(&root.join("Cargo.lock"));
        let topology_sha = digest_of(&root.join("policy/product-package-topology-v2.toml"));

        let evidence_dir = root.join("target/freeze-evidence");
        let packages_dir = evidence_dir.join("packages");
        std::fs::create_dir_all(&packages_dir)?;
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
            std::fs::write(packages_dir.join(format!("{name}-0.2.0.crate")), &bytes)?;
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

        let subject = super::SubjectIdentity::collect(
            &mut super::FilesystemSubjectInputs { root: &root },
            "0.2.0",
        )?;
        let preflight = serde_json::json!(
            shared_checksums
                .iter()
                .map(|(name, checksum)| {
                    serde_json::json!({
                        "name": name,
                        "version": "0.1.0",
                        "state": "already_published_exact",
                        "registry_checksum": format!("sha256:{checksum}")
                    })
                })
                .collect::<Vec<_>>()
        );
        let mut rehearsal = super::rehearsal_tests::producer_characterization(&subject)?;
        rehearsal
            .as_object_mut()
            .ok_or("producer receipt is not an object")?
            .insert("shared_prerequisites".to_string(), preflight);

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
        // The unchanged real producer is structurally compatible, but its
        // seven false proof flags cannot become Complete. The separately
        // required registry row is NotProven until trusted context is wired.
        write(
            &evidence_dir,
            "rehearsal.json",
            &serde_json::to_vec(&rehearsal)?,
        );
        super::rehearsal_tests::require_noncomplete_composition(
            &root,
            &args,
            &subject,
            "zero_mutation_proof.tag_mutation_prevented",
        )?;

        for (pointer, replacement, diagnostic) in [
            (
                "/phases/release_identity",
                serde_json::json!("Mismatch"),
                "phase release_identity",
            ),
            (
                "/phases/candidate_package_set",
                serde_json::json!("Incomplete"),
                "phase candidate_package_set",
            ),
            (
                "/phases/shared_prerequisites",
                serde_json::json!("ProviderUnavailable"),
                "phase shared_prerequisites",
            ),
            (
                "/phases/publisher_state_machine",
                serde_json::json!("InstrumentFailure"),
                "phase publisher_state_machine",
            ),
            (
                "/phases/docs_and_support_identity",
                serde_json::json!("Unsupported"),
                "phase docs_and_support_identity",
            ),
            (
                "/phases/manifest_and_assets",
                serde_json::json!(false),
                "phase manifest_and_assets",
            ),
            (
                "/phases/workflow_graph_permissions",
                serde_json::json!("Failed"),
                "phase workflow_graph_permissions",
            ),
            (
                "/phases/authorization_boundary",
                serde_json::json!("Complete"),
                "phase authorization_boundary",
            ),
            (
                "/aggregate_status",
                serde_json::json!("Complete"),
                "aggregate_status",
            ),
            (
                "/aggregate_status",
                serde_json::json!("Mismatch"),
                "aggregate_status",
            ),
            (
                "/authorization_boundary",
                serde_json::Value::Null,
                "authorization_boundary evidence object",
            ),
            (
                "/authorization_boundary/token_present",
                serde_json::json!(true),
                "authorization_boundary.token_present",
            ),
            ("/phases", serde_json::json!({}), "phase release_identity"),
            (
                "/zero_mutation_proof",
                serde_json::Value::Null,
                "zero_mutation_proof object",
            ),
        ] {
            let mut invalid = rehearsal.clone();
            super::rehearsal_tests::replace(&mut invalid, pointer, replacement)?;
            write(
                &evidence_dir,
                "rehearsal.json",
                &serde_json::to_vec(&invalid)?,
            );
            super::rehearsal_tests::require_noncomplete_composition(
                &root, &args, &subject, diagnostic,
            )?;
        }

        // A duplicate key must fail at raw-byte admission, before its failed
        // result disappears into Value or a stale successful artifact is used.
        let duplicated = serde_json::to_string(&rehearsal)?.replace(
            "\"release_identity\":\"Complete\"",
            "\"release_identity\":\"Mismatch\",\"release_identity\":\"Complete\"",
        );
        if duplicated == serde_json::to_string(&rehearsal)? {
            return Err("the duplicate-phase control did not change receipt bytes".into());
        }
        write(&evidence_dir, "rehearsal.json", duplicated.as_bytes());
        std::fs::remove_dir_all(&args.out_dir)?;
        let error = cmd_compose(&root, &args)
            .err()
            .ok_or("duplicate phase was admitted")?;
        if error.kind() != allow_core::CargoAllowErrorKind::Usage
            || !error.to_string().contains("duplicate JSON object key")
            || args.out_dir.exists()
        {
            return Err(format!("duplicate phase did not fail before composition: {error}").into());
        }
        write(
            &evidence_dir,
            "rehearsal.json",
            &serde_json::to_vec(&rehearsal)?,
        );
        super::registry_tests::require_noncomplete_composition(&root, &args, &subject)?;
        std::fs::remove_dir_all(&root)?;
        Ok(())
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
    fn registry_role_binds_exact_version_and_flags_drift() -> Result<(), Box<dyn std::error::Error>>
    {
        let subject = subject();
        let (input, _, _) = super::registry_tests::fixture(&subject, None)?;
        let bound = bind_evidence(
            &subject,
            FreezeEvidenceRole::RegistryObservation,
            &serde_json::to_value(&input)?,
        );
        if bound.iter().any(|note| note.starts_with("fail:")) {
            return Err(format!("typed registry candidate lost subject binding: {bound:?}").into());
        }

        let mut changed = input;
        changed.candidate.root_package_version = "0.1.11".to_string();
        let drifted = bind_evidence(
            &subject,
            FreezeEvidenceRole::RegistryObservation,
            &serde_json::to_value(changed)?,
        );
        if !drifted.iter().any(|note| note.starts_with("fail:")) {
            return Err("foreign registry candidate version acquired subject binding".into());
        }
        for version in ["0.2.0", "0.1.11"] {
            let legacy = bind_evidence(
                &subject,
                FreezeEvidenceRole::RegistryObservation,
                &json!({"crate": "cargo-allow", "version": version}),
            );
            if !legacy
                .iter()
                .any(|note| note.contains("FinalRegistryPreflightInputV1"))
            {
                return Err(format!("legacy registry JSON was admitted: {legacy:?}").into());
            }
        }
        Ok(())
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
