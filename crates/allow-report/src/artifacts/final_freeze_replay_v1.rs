//! Read-only replay of the final freeze verdict (#3919) from retained inputs.
//!
//! The replay reconstructs the Complete final-freeze verdict (#2501) from
//! retained exact artifacts alone: the frozen candidate custody (#3917), the
//! final evidence graph (#3913), the typed freeze-receipt input contract, the
//! retained transfer envelopes (#3916), the retained exact artifact bytes, and
//! explicitly refreshable provider/control observations. Every semantic and
//! exact-artifact digest is recomputed from retained bytes. The replay is pure:
//! it takes only the retained input set, reads nothing else (no filesystem, no
//! environment, no ambient cache), and cannot strengthen `Incomplete`,
//! `NotProven`, `Unsupported`, `ProviderUnavailable`, or incident facts. It
//! never tags, uploads, publishes, authorizes, or mutates any live setting.

use super::final_evidence_graph_v1::{
    FinalEvidenceAuthorityScopeV1, FinalEvidenceEvaluationResultV1, FinalEvidenceGraphModeV1,
    FinalEvidenceGraphV1, FinalEvidenceNodeClassV1, FinalEvidencePackageRoleV1,
    FinalEvidencePackageSubjectV1, FinalEvidenceReleaseIdentityV1, evaluate_final_evidence_graph,
};
use super::frozen_candidate_custody_v1::{
    CargoAllowFrozenCandidateCustodyV1, CustodyDispositionV1,
};
use super::release_artifact_transfer_v1::CargoAllowReleaseArtifactTransferV1;
use super::release_identity_v1::{ReleaseChannelV1, ReleaseIdentityV1};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FINAL_FREEZE_REPLAY_SCHEMA_ID: &str = "cargo-allow.final-freeze-replay.v1";
pub const FINAL_FREEZE_REPLAY_SCHEMA_VERSION: u32 = 1;
pub const FINAL_FREEZE_RECEIPT_SCHEMA_ID: &str = "cargo-allow.final-freeze-receipt.v1";
pub const FINAL_FREEZE_RECEIPT_SCHEMA_VERSION: u32 = 1;

/// The selected final-freeze denominator (#2501): ten upload-candidate package
/// archives plus three existing shared prerequisite rows.
pub const FINAL_FREEZE_EXPECTED_UPLOAD_ROWS_V1: u32 = 10;
/// The selected final-freeze denominator (#2501): ten upload-candidate package
/// archives plus three existing shared prerequisite rows.
pub const FINAL_FREEZE_EXPECTED_SHARED_ROWS_V1: u32 = 3;

const FREEZE_RECEIPT_ROLE: &str = "FreezeReceipt";
const PACKAGE_ARCHIVE_ROLE: &str = "PackageArchive";

const REPLAY_CLAIM_BOUNDARY: &str = "This replay reconstructs the final-freeze verdict from retained immutable inputs plus explicitly refreshable observations and recomputes every semantic and exact-artifact digest from retained bytes. It reads nothing outside its retained input set, cannot strengthen Incomplete, NotProven, Unsupported, ProviderUnavailable, or incident facts, and never tags, uploads, publishes, authorizes, mints release state, reads a secret, or mutates any live setting.";

const RECEIPT_CLAIM_BOUNDARY: &str = "The final-freeze receipt records the completed #2501 candidate freeze: the bound custody aggregate, the exact evidence graph digest, the selected 10+3 denominator, the prepublication manifest result, the RC.1 exclusion with its incident handoff, and the remaining irreversible operations. It records a completed freeze; it does not authorize publication.";

/// Closed replay result vocabulary for the reconstructed final freeze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalFreezeReplayResultV1 {
    CompleteEquivalent,
    Incomplete,
    Stale,
    Mismatch,
    MissingArtifact,
    ProviderUnavailable,
    InstrumentFailure,
}

impl FinalFreezeReplayResultV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CompleteEquivalent => "complete_equivalent",
            Self::Incomplete => "incomplete",
            Self::Stale => "stale",
            Self::Mismatch => "mismatch",
            Self::MissingArtifact => "missing_artifact",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// Closed replay row vocabulary, declared in aggregation severity order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalFreezeReplayRowKindV1 {
    InstrumentFailure,
    Mismatch,
    MissingArtifact,
    ProviderUnavailable,
    Stale,
    Incomplete,
}

impl FinalFreezeReplayRowKindV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InstrumentFailure => "instrument_failure",
            Self::Mismatch => "mismatch",
            Self::MissingArtifact => "missing_artifact",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::Stale => "stale",
            Self::Incomplete => "incomplete",
        }
    }

    /// The replay result a single row of this kind forces.
    const fn forced_result(self) -> FinalFreezeReplayResultV1 {
        match self {
            Self::InstrumentFailure => FinalFreezeReplayResultV1::InstrumentFailure,
            Self::Mismatch => FinalFreezeReplayResultV1::Mismatch,
            Self::MissingArtifact => FinalFreezeReplayResultV1::MissingArtifact,
            Self::ProviderUnavailable => FinalFreezeReplayResultV1::ProviderUnavailable,
            Self::Stale => FinalFreezeReplayResultV1::Stale,
            Self::Incomplete => FinalFreezeReplayResultV1::Incomplete,
        }
    }
}

/// One deterministic replay finding. `subject` names the custody item,
/// artifact, evidence node, or observation the row is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalFreezeReplayRowV1 {
    pub kind: FinalFreezeReplayRowKindV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub message: String,
}

/// Kinds of explicitly refreshable observations. `AmbientCache` readings are
/// recorded but never authoritative: ambient caches cannot satisfy missing
/// retained inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshableObservationKindV1 {
    SourceLiveControl,
    RegistryFeasibility,
    AmbientCache,
}

impl RefreshableObservationKindV1 {
    /// Required observation kinds whose freshness the replay re-evaluates.
    const fn required(self) -> bool {
        matches!(self, Self::SourceLiveControl | Self::RegistryFeasibility)
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SourceLiveControl => "source_live_control",
            Self::RegistryFeasibility => "registry_feasibility",
            Self::AmbientCache => "ambient_cache",
        }
    }
}

/// One refreshable provider/control observation input. Only the observation
/// identity and kind are retained; the reading is re-evaluated through the
/// observation's own adapter at replay time and stays distinct from immutable
/// candidate evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshableObservationV1 {
    pub observation_id: String,
    pub kind: RefreshableObservationKindV1,
    pub observed_at_utc: String,
}

/// Freshness of one re-evaluated observation reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationFreshnessV1 {
    Current,
    Stale,
    Mismatch,
    ProviderUnavailable,
    InstrumentFailure,
}

/// The reading an observation adapter returns for one observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationReadingV1 {
    pub freshness: ObservationFreshnessV1,
    pub detail: String,
}

/// The recorded, deterministic reading row for one observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationReadingRowV1 {
    pub observation_id: String,
    pub kind: RefreshableObservationKindV1,
    pub freshness: ObservationFreshnessV1,
    pub detail: String,
    /// Whether this kind can influence the replay result. Ambient-cache
    /// readings are never authoritative.
    pub authoritative: bool,
}

/// Adapter port through which refreshable observations are re-evaluated at
/// replay time. Implementations own the external contact; the replay itself
/// performs none.
pub trait RefreshableObservationAdapterV1 {
    fn refresh(&self, observation: &RefreshableObservationV1) -> ObservationReadingV1;
}

/// Opaque retained payload bytes. The bytes are readable only through digest
/// and size recomputation, so a replay cannot leak or transform the payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedArtifactBytesV1 {
    bytes: Vec<u8>,
}

impl RetainedArtifactBytesV1 {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    fn recomputed_digest(&self) -> String {
        allow_core::sha256_v1_bytes(&self.bytes)
    }

    fn size_bytes(&self) -> u64 {
        self.bytes.len() as u64
    }
}

/// One retained exact artifact: opaque bytes plus the digest the retained
/// envelope/custody chain declares for it. Every digest is recomputed from the
/// bytes at replay time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedExactArtifactV1 {
    pub role: String,
    pub artifact_id: String,
    pub declared_sha256: String,
    pub bytes: RetainedArtifactBytesV1,
}

/// The prepublication manifest result bound into the freeze receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalFreezeManifestResultV1 {
    Exact,
    Failed,
    NotRun,
}

/// The receipt's binding of the prepublication manifest result to one retained
/// manifest artifact digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalFreezeManifestBindingV1 {
    pub result: FinalFreezeManifestResultV1,
    pub artifact_id: String,
    pub payload_sha256: String,
}

/// Typed input contract for the #2501 final-freeze receipt. This is the
/// minimal surface the replay consumes; the receipt's own integrity is
/// re-established by recomputing its canonical serialized digest against the
/// retained `FreezeReceipt` custody item and exact artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalFreezeReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub freeze_id: String,
    pub frozen_custody_id: String,
    pub frozen_at_utc: String,
    pub release_identity: FinalEvidenceReleaseIdentityV1,
    pub repository: String,
    pub commit: String,
    pub tree: String,
    pub cargo_lock_digest: String,
    pub topology_digest: String,
    pub expected_upload_rows: u32,
    pub expected_shared_rows: u32,
    pub package_rows: Vec<FinalEvidencePackageSubjectV1>,
    pub prepublication_manifest: FinalFreezeManifestBindingV1,
    pub rc1_excluded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rc1_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_handoff_id: Option<String>,
    pub recorded_graph_digest: String,
    pub remaining_irreversible_operations: Vec<String>,
    pub claim_boundary: String,
}

/// Construction input for the typed freeze-receipt contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalFreezeReceiptInitV1 {
    pub freeze_id: String,
    pub frozen_custody_id: String,
    pub frozen_at_utc: String,
    pub release_identity: FinalEvidenceReleaseIdentityV1,
    pub repository: String,
    pub commit: String,
    pub tree: String,
    pub cargo_lock_digest: String,
    pub topology_digest: String,
    pub expected_upload_rows: u32,
    pub expected_shared_rows: u32,
    pub package_rows: Vec<FinalEvidencePackageSubjectV1>,
    pub prepublication_manifest: FinalFreezeManifestBindingV1,
    pub rc1_excluded: bool,
    pub rc1_version: Option<String>,
    pub incident_handoff_id: Option<String>,
    pub recorded_graph_digest: String,
    pub remaining_irreversible_operations: Vec<String>,
}

impl CargoAllowFinalFreezeReceiptV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = FINAL_FREEZE_RECEIPT_SCHEMA_ID;
    pub const CURRENT_SCHEMA_VERSION: u32 = FINAL_FREEZE_RECEIPT_SCHEMA_VERSION;

    #[must_use]
    pub fn new(init: FinalFreezeReceiptInitV1) -> Self {
        Self {
            schema_id: Self::CURRENT_SCHEMA_ID.to_string(),
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            freeze_id: init.freeze_id,
            frozen_custody_id: init.frozen_custody_id,
            frozen_at_utc: init.frozen_at_utc,
            release_identity: init.release_identity,
            repository: init.repository,
            commit: init.commit,
            tree: init.tree,
            cargo_lock_digest: init.cargo_lock_digest,
            topology_digest: init.topology_digest,
            expected_upload_rows: init.expected_upload_rows,
            expected_shared_rows: init.expected_shared_rows,
            package_rows: init.package_rows,
            prepublication_manifest: init.prepublication_manifest,
            rc1_excluded: init.rc1_excluded,
            rc1_version: init.rc1_version,
            incident_handoff_id: init.incident_handoff_id,
            recorded_graph_digest: init.recorded_graph_digest,
            remaining_irreversible_operations: init.remaining_irreversible_operations,
            claim_boundary: RECEIPT_CLAIM_BOUNDARY.to_string(),
        }
    }
}

/// The complete retained input set. This is the only thing the replay reads:
/// there is no path, environment, or ambient capability anywhere in the input
/// surface, so a missing retained input can never be satisfied by `main`, a
/// branch head, a package builder, or an ambient cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalFreezeReplayInputsV1 {
    pub custody: CargoAllowFrozenCandidateCustodyV1,
    pub evidence_graph: FinalEvidenceGraphV1,
    pub freeze_receipt: CargoAllowFinalFreezeReceiptV1,
    pub retained_transfers: Vec<CargoAllowReleaseArtifactTransferV1>,
    pub retained_artifacts: Vec<RetainedExactArtifactV1>,
    pub observations: Vec<RefreshableObservationV1>,
    pub replayed_at_utc: String,
}

/// The versioned final-freeze replay result. Human and machine projections
/// derive from this one typed value; it carries no operation capability of any
/// kind — nothing in it can tag, upload, publish, authorize, or mutate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalFreezeReplayV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub result: FinalFreezeReplayResultV1,
    pub custody_id: String,
    pub custody_disposition: CustodyDispositionV1,
    pub evidence_result: FinalEvidenceEvaluationResultV1,
    pub evidence_graph_digest: String,
    pub receipt_digest: String,
    pub release_identity: FinalEvidenceReleaseIdentityV1,
    pub repository: String,
    pub commit: String,
    pub tree: String,
    pub cargo_lock_digest: String,
    pub topology_digest: String,
    pub selected_channel: String,
    pub selected_upload_rows: u32,
    pub selected_shared_rows: u32,
    pub selected_package_rows: u32,
    pub retained_artifact_count: u32,
    pub manifest_result: FinalFreezeManifestResultV1,
    pub manifest_payload_digest: String,
    pub rc1_excluded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_handoff_id: Option<String>,
    pub incident_handoff_present: bool,
    pub observation_readings: Vec<ObservationReadingRowV1>,
    pub remaining_irreversible_operations: Vec<String>,
    pub rows: Vec<FinalFreezeReplayRowV1>,
    pub retained_bytes_verified: bool,
    pub claim_boundary: String,
}

/// Replay the final freeze from the retained input set alone, re-evaluating
/// refreshable observations through their adapter. Pure: no filesystem,
/// environment, network, or ambient state is consulted, and no external state
/// is mutated.
#[must_use]
pub fn replay_final_freeze(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    adapters: &dyn RefreshableObservationAdapterV1,
) -> CargoAllowFinalFreezeReplayV1 {
    let mut rows = Vec::new();

    let digests = retained_digest_map(&inputs.retained_artifacts, &mut rows);
    let custody_disposition = replay_custody_binding(inputs, &mut rows);
    replay_subject_binding(inputs, &mut rows);
    let (evidence_result, graph_digest) = replay_evidence_graph(inputs, &mut rows);
    let incident_present = replay_incident_handoff(inputs, &mut rows);
    replay_retained_bytes(inputs, &digests, &mut rows);
    replay_transfer_coverage(inputs, &digests, &mut rows);
    replay_manifest_binding(inputs, &digests, &mut rows);
    let readings = replay_observations(inputs, adapters, &mut rows);
    let operations = replay_remaining_operations(inputs, &mut rows);

    rows.sort_by(|left, right| {
        (&left.kind, left.subject.as_deref(), left.message.as_str()).cmp(&(
            &right.kind,
            right.subject.as_deref(),
            right.message.as_str(),
        ))
    });

    let result = rows
        .iter()
        .map(|row| row.kind)
        .min()
        .map_or(FinalFreezeReplayResultV1::CompleteEquivalent, |kind| {
            kind.forced_result()
        });

    let integrity_clean = !rows.iter().any(|row| {
        matches!(
            row.kind,
            FinalFreezeReplayRowKindV1::Mismatch | FinalFreezeReplayRowKindV1::MissingArtifact
        )
    });

    let identity = &inputs.freeze_receipt.release_identity;
    let channel =
        ReleaseIdentityV1::parse(&identity.version, &identity.tag, identity.github_prerelease)
            .map_or(ReleaseChannelV1::Stable, |parsed| {
                parsed.version().channel()
            });

    let selected = &inputs.evidence_graph.selected_subject;
    CargoAllowFinalFreezeReplayV1 {
        schema_id: FINAL_FREEZE_REPLAY_SCHEMA_ID.to_string(),
        schema_version: FINAL_FREEZE_REPLAY_SCHEMA_VERSION,
        result,
        custody_id: inputs.custody.custody_id.clone(),
        custody_disposition,
        evidence_result,
        evidence_graph_digest: graph_digest,
        receipt_digest: receipt_digest(inputs),
        release_identity: identity.clone(),
        repository: inputs.freeze_receipt.repository.clone(),
        commit: inputs.freeze_receipt.commit.clone(),
        tree: inputs.freeze_receipt.tree.clone(),
        cargo_lock_digest: selected.cargo_lock_digest.clone(),
        topology_digest: selected.topology_digest.clone(),
        selected_channel: channel_label(channel).to_string(),
        selected_upload_rows: selected.expected_upload_rows,
        selected_shared_rows: selected.expected_shared_rows,
        selected_package_rows: selected.package_rows.len() as u32,
        retained_artifact_count: inputs.retained_artifacts.len() as u32,
        manifest_result: inputs.freeze_receipt.prepublication_manifest.result,
        manifest_payload_digest: inputs
            .freeze_receipt
            .prepublication_manifest
            .payload_sha256
            .clone(),
        rc1_excluded: inputs.freeze_receipt.rc1_excluded,
        incident_handoff_id: inputs.freeze_receipt.incident_handoff_id.clone(),
        incident_handoff_present: incident_present,
        observation_readings: readings,
        remaining_irreversible_operations: operations,
        rows,
        retained_bytes_verified: integrity_clean
            && custody_disposition == CustodyDispositionV1::Complete,
        claim_boundary: REPLAY_CLAIM_BOUNDARY.to_string(),
    }
}

/// Render a replay result as JSON (machine projection).
pub fn render_final_freeze_replay_json(
    replay: &CargoAllowFinalFreezeReplayV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(replay)
}

/// Render a replay result as deterministic Markdown (human projection).
#[must_use]
pub fn render_final_freeze_replay_markdown(replay: &CargoAllowFinalFreezeReplayV1) -> String {
    let mut output = String::new();
    output.push_str("# Final freeze replay\n\n");
    output.push_str(&format!(
        "- Result: `{}`\n",
        markdown_escape(replay.result.label())
    ));
    output.push_str(&format!(
        "- Custody: `{}` (`{}`)\n",
        markdown_escape(&replay.custody_id),
        markdown_escape(custody_label(replay.custody_disposition))
    ));
    output.push_str(&format!(
        "- Evidence graph: `{}` (`{}`)\n",
        markdown_escape(&replay.evidence_graph_digest),
        markdown_escape(evidence_label(replay.evidence_result))
    ));
    output.push_str(&format!(
        "- Release identity: `{}` / `{}` (channel `{}`)\n",
        markdown_escape(&replay.release_identity.version),
        markdown_escape(&replay.release_identity.tag),
        markdown_escape(&replay.selected_channel)
    ));
    output.push_str(&format!(
        "- Selected denominator: {} upload + {} shared = {} package rows\n",
        replay.selected_upload_rows, replay.selected_shared_rows, replay.selected_package_rows
    ));
    output.push_str(&format!(
        "- RC.1 excluded: {} ; incident handoff present: {}\n",
        replay.rc1_excluded, replay.incident_handoff_present
    ));
    output.push_str(&format!(
        "- Retained bytes verified: {}\n\n",
        replay.retained_bytes_verified
    ));

    output.push_str("## Refreshable observations\n\n");
    output.push_str("| Observation | Kind | Freshness | Authoritative |\n");
    output.push_str("| --- | --- | --- | --- |\n");
    for reading in &replay.observation_readings {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            markdown_escape(&reading.observation_id),
            markdown_escape(reading.kind.label()),
            markdown_escape(freshness_label(reading.freshness)),
            if reading.authoritative { "yes" } else { "no" }
        ));
    }

    append_word_list(
        &mut output,
        "Remaining irreversible operations",
        &replay.remaining_irreversible_operations,
    );

    output.push_str("\n## Rows\n\n");
    if replay.rows.is_empty() {
        output.push_str("No rows.\n");
    } else {
        for row in &replay.rows {
            let subject = row.subject.as_deref().unwrap_or("replay");
            output.push_str(&format!(
                "- `{}` on `{}`: {}\n",
                row.kind.label(),
                markdown_escape(subject),
                markdown_escape(&row.message)
            ));
        }
    }

    output.push_str("\n## Claim boundary\n\n");
    output.push_str(&markdown_escape(&replay.claim_boundary));
    output.push('\n');
    output
}

/// Recompute every retained artifact digest from its retained bytes. Duplicate
/// artifact identities are instrument failures because they make the retained
/// set ambiguous.
fn retained_digest_map(
    artifacts: &[RetainedExactArtifactV1],
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) -> BTreeMap<String, String> {
    let mut digests = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for artifact in artifacts {
        if artifact.artifact_id.trim().is_empty()
            || artifact.role.trim().is_empty()
            || !is_sha256_digest(&artifact.declared_sha256)
        {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::InstrumentFailure,
                Some(artifact.artifact_id.clone()),
                "a retained exact artifact lacks an identity or a well-formed declared digest",
            );
            continue;
        }
        if !seen.insert(artifact.artifact_id.clone()) {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::InstrumentFailure,
                Some(artifact.artifact_id.clone()),
                "the retained exact artifact set contains a duplicate artifact identity",
            );
        }
        digests.insert(
            artifact.artifact_id.clone(),
            artifact.bytes.recomputed_digest(),
        );
    }
    digests
}

/// Recompute the custody disposition against the receipt's exact candidate
/// identity and bind the receipt to the custody aggregate it froze.
fn replay_custody_binding(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) -> CustodyDispositionV1 {
    let receipt = &inputs.freeze_receipt;
    let custody = &inputs.custody;
    if receipt.frozen_custody_id != custody.custody_id {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(custody.custody_id.clone()),
            "the retained custody aggregate is not the custody aggregate bound by the freeze receipt",
        );
    }
    if custody.git_tree != receipt.tree {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(custody.custody_id.clone()),
            "the retained custody git tree differs from the receipt's selected source tree",
        );
    }

    let disposition = custody.evaluate_custody(
        &receipt.commit,
        &receipt.release_identity.version,
        &inputs.replayed_at_utc,
    );
    match disposition {
        CustodyDispositionV1::Complete => {}
        CustodyDispositionV1::Mismatch => push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(custody.custody_id.clone()),
            "the retained custody does not bind the freeze receipt's exact candidate identity",
        ),
        CustodyDispositionV1::Stale => push_row(
            rows,
            FinalFreezeReplayRowKindV1::Stale,
            Some(custody.custody_id.clone()),
            "the retained custody binds a different source commit than the freeze receipt",
        ),
        CustodyDispositionV1::Expiring => push_row(
            rows,
            FinalFreezeReplayRowKindV1::Stale,
            Some(custody.custody_id.clone()),
            "the retained custody retention expired before the replay",
        ),
        CustodyDispositionV1::Missing => push_row(
            rows,
            FinalFreezeReplayRowKindV1::Incomplete,
            Some(custody.custody_id.clone()),
            "the retained custody readback is unverified, so exact retention is not proven",
        ),
        CustodyDispositionV1::ProviderUnavailable => push_row(
            rows,
            FinalFreezeReplayRowKindV1::ProviderUnavailable,
            Some(custody.custody_id.clone()),
            "the retained custody lacks a storage locator for one of its items",
        ),
        CustodyDispositionV1::InstrumentFailure => push_row(
            rows,
            FinalFreezeReplayRowKindV1::InstrumentFailure,
            Some(custody.custody_id.clone()),
            "the retained custody aggregate is structurally unusable",
        ),
    }
    disposition
}

/// Cross-check the receipt against the retained graph: release identity,
/// repository, source identities, Cargo.lock/topology digests, and the exact
/// selected package row set.
fn replay_subject_binding(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) {
    let receipt = &inputs.freeze_receipt;
    let selected = &inputs.evidence_graph.selected_subject;

    if receipt.release_identity != selected.release_identity {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(receipt.freeze_id.clone()),
            "the receipt release identity differs from the selected evidence identity",
        );
    }
    if receipt.repository != inputs.evidence_graph.repository
        || receipt.repository != selected.repository
    {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(receipt.freeze_id.clone()),
            "the receipt repository differs from the selected evidence repository",
        );
    }
    for (field, receipt_value, selected_value) in [
        ("commit", &receipt.commit, &selected.commit),
        ("tree", &receipt.tree, &selected.tree),
        (
            "cargo_lock_digest",
            &receipt.cargo_lock_digest,
            &selected.cargo_lock_digest,
        ),
        (
            "topology_digest",
            &receipt.topology_digest,
            &selected.topology_digest,
        ),
    ] {
        if receipt_value != selected_value {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::Mismatch,
                Some(receipt.freeze_id.clone()),
                &format!("the receipt {field} differs from the selected evidence subject"),
            );
        }
    }

    if receipt.expected_upload_rows != FINAL_FREEZE_EXPECTED_UPLOAD_ROWS_V1
        || receipt.expected_shared_rows != FINAL_FREEZE_EXPECTED_SHARED_ROWS_V1
    {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(receipt.freeze_id.clone()),
            "the receipt denominator deviates from the selected 10+3 final-freeze contract",
        );
    }
    if selected.expected_upload_rows != FINAL_FREEZE_EXPECTED_UPLOAD_ROWS_V1
        || selected.expected_shared_rows != FINAL_FREEZE_EXPECTED_SHARED_ROWS_V1
    {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(receipt.freeze_id.clone()),
            "the selected evidence denominator deviates from the selected 10+3 final-freeze contract",
        );
    }

    let receipt_rows = canonical_rows(&receipt.package_rows);
    let selected_rows = canonical_rows(&selected.package_rows);
    if receipt_rows != selected_rows {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(receipt.freeze_id.clone()),
            "the receipt package rows differ from the selected evidence package rows",
        );
    }
    let receipt_upload = receipt
        .package_rows
        .iter()
        .filter(|row| row.role == FinalEvidencePackageRoleV1::UploadCandidate)
        .count();
    let receipt_shared = receipt
        .package_rows
        .iter()
        .filter(|row| row.role == FinalEvidencePackageRoleV1::ExistingSharedPrerequisite)
        .count();
    if receipt_upload != receipt.expected_upload_rows as usize
        || receipt_shared != receipt.expected_shared_rows as usize
    {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(receipt.freeze_id.clone()),
            "the receipt package rows do not match the receipt's declared denominator",
        );
    }

    let identity_parse = ReleaseIdentityV1::parse(
        &receipt.release_identity.version,
        &receipt.release_identity.tag,
        receipt.release_identity.github_prerelease,
    );
    match identity_parse {
        Ok(identity) => {
            if identity.version().channel() != ReleaseChannelV1::Stable {
                push_row(
                    rows,
                    FinalFreezeReplayRowKindV1::Mismatch,
                    Some(receipt.freeze_id.clone()),
                    "a final freeze replay requires a stable channel identity",
                );
            }
        }
        Err(_) => push_row(
            rows,
            FinalFreezeReplayRowKindV1::InstrumentFailure,
            Some(receipt.freeze_id.clone()),
            "the receipt release identity is malformed and cannot be parsed",
        ),
    }

    if inputs.evidence_graph.mode != FinalEvidenceGraphModeV1::Production {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(receipt.freeze_id.clone()),
            "the retained evidence graph was supplied in fixture mode and cannot support a final freeze replay",
        );
    }
}

/// Re-evaluate the retained evidence graph and bind it to the receipt's
/// recorded graph digest. Altered edges change the canonical graph digest even
/// when every leaf digest is preserved.
fn replay_evidence_graph(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) -> (FinalEvidenceEvaluationResultV1, String) {
    let evaluation = evaluate_final_evidence_graph(&inputs.evidence_graph);
    if !is_sha256_digest(&inputs.freeze_receipt.recorded_graph_digest) {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::InstrumentFailure,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "the receipt's recorded graph digest is malformed",
        );
    }
    if evaluation.graph_digest != inputs.freeze_receipt.recorded_graph_digest {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "the retained evidence graph no longer hashes to the freeze receipt's recorded graph digest",
        );
    }
    match evaluation.result {
        FinalEvidenceEvaluationResultV1::Complete => {}
        FinalEvidenceEvaluationResultV1::Incomplete => push_row(
            rows,
            FinalFreezeReplayRowKindV1::Incomplete,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "the retained evidence graph does not compose to Complete; incomplete facts cannot be strengthened",
        ),
        FinalEvidenceEvaluationResultV1::Stale => push_row(
            rows,
            FinalFreezeReplayRowKindV1::Stale,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "the retained evidence graph is stale or transitively non-current",
        ),
        FinalEvidenceEvaluationResultV1::Mismatch | FinalEvidenceEvaluationResultV1::Conflict => {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::Mismatch,
                Some(inputs.freeze_receipt.freeze_id.clone()),
                "the retained evidence graph conflicts with the selected final subject",
            );
        }
        FinalEvidenceEvaluationResultV1::Incident => push_row(
            rows,
            FinalFreezeReplayRowKindV1::Mismatch,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "incident facts are preserved verbatim and cannot be replayed into equivalence",
        ),
        FinalEvidenceEvaluationResultV1::ProviderUnavailable => push_row(
            rows,
            FinalFreezeReplayRowKindV1::ProviderUnavailable,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "the retained evidence graph reports an unavailable provider",
        ),
        FinalEvidenceEvaluationResultV1::InstrumentFailure
        | FinalEvidenceEvaluationResultV1::MalformedGraph => push_row(
            rows,
            FinalFreezeReplayRowKindV1::InstrumentFailure,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "the retained evidence graph is structurally unusable",
        ),
    }
    (evaluation.result, evaluation.graph_digest)
}

/// Verify the RC.1 exclusion and its incident handoff row are retained in the
/// graph. Incident facts are historical authority forever.
fn replay_incident_handoff(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) -> bool {
    let receipt = &inputs.freeze_receipt;
    if !receipt.rc1_excluded {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Incomplete,
            Some(receipt.freeze_id.clone()),
            "the freeze receipt does not record the RC.1 exclusion",
        );
        return false;
    }
    let Some(handoff_id) = receipt
        .incident_handoff_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Incomplete,
            Some(receipt.freeze_id.clone()),
            "the freeze receipt records the RC.1 exclusion without its incident handoff id",
        );
        return false;
    };
    let present = inputs.evidence_graph.nodes.iter().any(|node| {
        node.evidence_id == handoff_id
            && node.class == FinalEvidenceNodeClassV1::IncidentHandoff
            && node.authority_scope == FinalEvidenceAuthorityScopeV1::HistoricalIncident
    });
    if !present {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Incomplete,
            Some(handoff_id.to_string()),
            "the retained incident handoff evidence row is missing from the evidence graph",
        );
    }
    present
}

/// Recompute every retained digest against its declared digest, its custody
/// record, and the receipt's typed content. A removed archive is a
/// `MissingArtifact`, never silently rebuildable from current source.
fn replay_retained_bytes(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    digests: &BTreeMap<String, String>,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) {
    let custody_items = inputs
        .custody
        .items
        .iter()
        .map(|item| (item.artifact_id.clone(), item))
        .collect::<BTreeMap<_, _>>();

    let mut retained_roles = BTreeMap::<&str, Vec<&RetainedExactArtifactV1>>::new();
    for artifact in &inputs.retained_artifacts {
        retained_roles
            .entry(artifact.role.as_str())
            .or_default()
            .push(artifact);
        let recomputed = match digests.get(&artifact.artifact_id) {
            Some(digest) => digest,
            None => continue,
        };
        if recomputed != &artifact.declared_sha256 {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::Mismatch,
                Some(artifact.artifact_id.clone()),
                "the retained bytes no longer hash to the artifact's declared digest",
            );
        }
        match custody_items.get(&artifact.artifact_id) {
            None => push_row(
                rows,
                FinalFreezeReplayRowKindV1::MissingArtifact,
                Some(artifact.artifact_id.clone()),
                "retained bytes exist outside the custody aggregate",
            ),
            Some(item) => {
                let Some(file) = item.files.first() else {
                    push_row(
                        rows,
                        FinalFreezeReplayRowKindV1::InstrumentFailure,
                        Some(item.artifact_id.clone()),
                        "the custody item carries no bound file digest",
                    );
                    continue;
                };
                if file.sha256 != *recomputed {
                    push_row(
                        rows,
                        FinalFreezeReplayRowKindV1::Mismatch,
                        Some(item.artifact_id.clone()),
                        "the custody digest does not match the recomputed retained-bytes digest",
                    );
                }
                if file.size_bytes != artifact.bytes.size_bytes() {
                    push_row(
                        rows,
                        FinalFreezeReplayRowKindV1::Mismatch,
                        Some(item.artifact_id.clone()),
                        "the custody size binding does not match the retained bytes",
                    );
                }
            }
        }
    }

    for item in &inputs.custody.items {
        if !digests.contains_key(&item.artifact_id) {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::MissingArtifact,
                Some(item.artifact_id.clone()),
                "a retained exact artifact recorded in custody is absent from the replay input set",
            );
        }
    }

    let selected = inputs.evidence_graph.canonicalized().selected_subject;
    let artifact_roles = inputs
        .retained_artifacts
        .iter()
        .map(|artifact| (artifact.artifact_id.clone(), artifact.role.clone()))
        .collect::<BTreeMap<_, _>>();
    for row in &selected.package_rows {
        if row.role != FinalEvidencePackageRoleV1::UploadCandidate {
            continue;
        }
        match digests.get(&row.package_name) {
            None => push_row(
                rows,
                FinalFreezeReplayRowKindV1::MissingArtifact,
                Some(row.package_name.clone()),
                "a retained package archive recorded in the selected denominator is absent from the retained set",
            ),
            Some(recomputed) => {
                if recomputed != &row.expected_digest {
                    push_row(
                        rows,
                        FinalFreezeReplayRowKindV1::Mismatch,
                        Some(row.package_name.clone()),
                        "the retained package archive digest differs from the selected expected digest",
                    );
                }
                if artifact_roles.get(&row.package_name).map(String::as_str)
                    != Some(PACKAGE_ARCHIVE_ROLE)
                {
                    push_row(
                        rows,
                        FinalFreezeReplayRowKindV1::Mismatch,
                        Some(row.package_name.clone()),
                        "the retained package archive is not retained under the package-archive role",
                    );
                }
            }
        }
    }

    let receipt_artifacts = retained_roles
        .get(FREEZE_RECEIPT_ROLE)
        .cloned()
        .unwrap_or_default();
    match receipt_artifacts.as_slice() {
        [artifact] => {
            let recomputed = receipt_digest(inputs);
            if artifact.bytes.recomputed_digest() != recomputed
                || artifact.declared_sha256 != recomputed
            {
                push_row(
                    rows,
                    FinalFreezeReplayRowKindV1::Mismatch,
                    Some(artifact.artifact_id.clone()),
                    "the typed freeze receipt no longer hashes to its retained receipt bytes",
                );
            }
        }
        [] => push_row(
            rows,
            FinalFreezeReplayRowKindV1::MissingArtifact,
            Some(FREEZE_RECEIPT_ROLE.to_string()),
            "the retained freeze receipt bytes are absent from the replay input set",
        ),
        _ => push_row(
            rows,
            FinalFreezeReplayRowKindV1::InstrumentFailure,
            Some(FREEZE_RECEIPT_ROLE.to_string()),
            "the retained set contains more than one freeze receipt artifact",
        ),
    }
}

/// Bind every retained exact artifact to a retained transfer envelope with an
/// exact file digest/size, and every envelope to a retained artifact.
fn replay_transfer_coverage(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    digests: &BTreeMap<String, String>,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) {
    for envelope in &inputs.retained_transfers {
        if envelope.producer.release_version != inputs.freeze_receipt.release_identity.version {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::Mismatch,
                Some(envelope.transfer_id.clone()),
                "a retained transfer envelope was produced for a different release version",
            );
        }
        if envelope.files.is_empty() {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::InstrumentFailure,
                Some(envelope.transfer_id.clone()),
                "a retained transfer envelope carries no bound file digest",
            );
        }
        if !digests.contains_key(&envelope.stable_artifact_id) {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::MissingArtifact,
                Some(envelope.stable_artifact_id.clone()),
                "a retained transfer envelope references an artifact absent from the replay input set",
            );
        }
    }

    for artifact in &inputs.retained_artifacts {
        let Some(recomputed) = digests.get(&artifact.artifact_id) else {
            continue;
        };
        let mut covered = false;
        let mut envelope_present = false;
        for envelope in &inputs.retained_transfers {
            if envelope.stable_artifact_id != artifact.artifact_id {
                continue;
            }
            envelope_present = true;
            if envelope.files.iter().any(|file| {
                file.sha256 == *recomputed && file.size_bytes == artifact.bytes.size_bytes()
            }) {
                covered = true;
            }
        }
        if covered {
            continue;
        }
        if envelope_present {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::Mismatch,
                Some(artifact.artifact_id.clone()),
                "no retained transfer envelope binds the recomputed retained-bytes digest",
            );
        } else {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::MissingArtifact,
                Some(artifact.artifact_id.clone()),
                "a retained exact artifact is not covered by any retained transfer envelope",
            );
        }
    }
}

/// Reconstruct the prepublication manifest result: the receipt must record an
/// Exact result bound to retained manifest bytes with the recorded digest.
fn replay_manifest_binding(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    digests: &BTreeMap<String, String>,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) {
    let manifest = &inputs.freeze_receipt.prepublication_manifest;
    if manifest.result != FinalFreezeManifestResultV1::Exact {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Incomplete,
            Some(manifest.artifact_id.clone()),
            "the retained prepublication manifest result in the freeze receipt is not Exact",
        );
    }
    match digests.get(&manifest.artifact_id) {
        None => push_row(
            rows,
            FinalFreezeReplayRowKindV1::MissingArtifact,
            Some(manifest.artifact_id.clone()),
            "the retained prepublication manifest artifact is absent from the replay input set",
        ),
        Some(recomputed) => {
            if recomputed != &manifest.payload_sha256 {
                push_row(
                    rows,
                    FinalFreezeReplayRowKindV1::Mismatch,
                    Some(manifest.artifact_id.clone()),
                    "the retained manifest bytes differ from the receipt's manifest digest binding",
                );
            }
        }
    }
}

/// Re-evaluate every refreshable observation through its adapter. Required
/// kinds must be present and current; ambient-cache readings are recorded but
/// can never influence the result.
fn replay_observations(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    adapters: &dyn RefreshableObservationAdapterV1,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) -> Vec<ObservationReadingRowV1> {
    let mut readings = Vec::new();
    let mut present = BTreeSet::new();
    for observation in &inputs.observations {
        if observation.observation_id.trim().is_empty() {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::InstrumentFailure,
                None,
                "a refreshable observation lacks a stable observation id",
            );
            continue;
        }
        let reading = adapters.refresh(observation);
        let authoritative = observation.kind.required();
        if authoritative {
            present.insert(observation.kind);
            match reading.freshness {
                ObservationFreshnessV1::Current => {}
                ObservationFreshnessV1::Stale => push_row(
                    rows,
                    FinalFreezeReplayRowKindV1::Stale,
                    Some(observation.observation_id.clone()),
                    "a required refreshable observation is not current",
                ),
                ObservationFreshnessV1::Mismatch => push_row(
                    rows,
                    FinalFreezeReplayRowKindV1::Mismatch,
                    Some(observation.observation_id.clone()),
                    "a required refreshable observation disagrees with the retained subject",
                ),
                ObservationFreshnessV1::ProviderUnavailable => push_row(
                    rows,
                    FinalFreezeReplayRowKindV1::ProviderUnavailable,
                    Some(observation.observation_id.clone()),
                    "the provider behind a required refreshable observation is unavailable",
                ),
                ObservationFreshnessV1::InstrumentFailure => push_row(
                    rows,
                    FinalFreezeReplayRowKindV1::InstrumentFailure,
                    Some(observation.observation_id.clone()),
                    "the instrument behind a required refreshable observation failed",
                ),
            }
        }
        readings.push(ObservationReadingRowV1 {
            observation_id: observation.observation_id.clone(),
            kind: observation.kind,
            freshness: reading.freshness,
            detail: reading.detail,
            authoritative,
        });
    }
    for kind in [
        RefreshableObservationKindV1::SourceLiveControl,
        RefreshableObservationKindV1::RegistryFeasibility,
    ] {
        if !present.contains(&kind) {
            push_row(
                rows,
                FinalFreezeReplayRowKindV1::Incomplete,
                Some(kind.label().to_string()),
                "a required refreshable observation kind is absent from the retained set",
            );
        }
    }
    readings.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    readings
}

/// Echo the remaining irreversible operations canonically. A freeze receipt
/// with none left is incomplete: publication has not happened and the list
/// cannot be silently dropped from the replay.
fn replay_remaining_operations(
    inputs: &CargoAllowFinalFreezeReplayInputsV1,
    rows: &mut Vec<FinalFreezeReplayRowV1>,
) -> Vec<String> {
    let mut operations = inputs
        .freeze_receipt
        .remaining_irreversible_operations
        .iter()
        .map(|operation| operation.trim().to_string())
        .filter(|operation| !operation.is_empty())
        .collect::<Vec<_>>();
    operations.sort();
    operations.dedup();
    if operations.is_empty() {
        push_row(
            rows,
            FinalFreezeReplayRowKindV1::Incomplete,
            Some(inputs.freeze_receipt.freeze_id.clone()),
            "the freeze receipt records no remaining irreversible operations",
        );
    }
    operations
}

/// Recompute the canonical serialized digest of the typed freeze receipt. Any
/// tampered receipt field changes this digest even though the filename,
/// artifact id, and every other retained object stay the same.
fn receipt_digest(inputs: &CargoAllowFinalFreezeReplayInputsV1) -> String {
    match serde_json::to_vec(&inputs.freeze_receipt) {
        Ok(bytes) => allow_core::sha256_v1_bytes(&bytes),
        Err(_) => format!("unavailable:{}", FINAL_FREEZE_RECEIPT_SCHEMA_ID),
    }
}

fn canonical_rows(rows: &[FinalEvidencePackageSubjectV1]) -> Vec<FinalEvidencePackageSubjectV1> {
    let mut canonical = rows.to_vec();
    canonical.sort();
    canonical
}

fn is_sha256_digest(value: &str) -> bool {
    let hex = value
        .strip_prefix("sha256:v1:")
        .or_else(|| value.strip_prefix("sha256:"));
    hex.is_some_and(|hex| {
        hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
    })
}

const fn channel_label(channel: ReleaseChannelV1) -> &'static str {
    match channel {
        ReleaseChannelV1::Stable => "stable",
        ReleaseChannelV1::ReleaseCandidate { .. } => "release_candidate",
    }
}

const fn custody_label(disposition: CustodyDispositionV1) -> &'static str {
    match disposition {
        CustodyDispositionV1::Complete => "complete",
        CustodyDispositionV1::Missing => "missing",
        CustodyDispositionV1::Expiring => "expiring",
        CustodyDispositionV1::Stale => "stale",
        CustodyDispositionV1::Mismatch => "mismatch",
        CustodyDispositionV1::ProviderUnavailable => "provider_unavailable",
        CustodyDispositionV1::InstrumentFailure => "instrument_failure",
    }
}

const fn evidence_label(result: FinalEvidenceEvaluationResultV1) -> &'static str {
    match result {
        FinalEvidenceEvaluationResultV1::Complete => "complete",
        FinalEvidenceEvaluationResultV1::Incomplete => "incomplete",
        FinalEvidenceEvaluationResultV1::Stale => "stale",
        FinalEvidenceEvaluationResultV1::Mismatch => "mismatch",
        FinalEvidenceEvaluationResultV1::Conflict => "conflict",
        FinalEvidenceEvaluationResultV1::MalformedGraph => "malformed_graph",
        FinalEvidenceEvaluationResultV1::ProviderUnavailable => "provider_unavailable",
        FinalEvidenceEvaluationResultV1::InstrumentFailure => "instrument_failure",
        FinalEvidenceEvaluationResultV1::Incident => "incident",
    }
}

const fn freshness_label(freshness: ObservationFreshnessV1) -> &'static str {
    match freshness {
        ObservationFreshnessV1::Current => "current",
        ObservationFreshnessV1::Stale => "stale",
        ObservationFreshnessV1::Mismatch => "mismatch",
        ObservationFreshnessV1::ProviderUnavailable => "provider_unavailable",
        ObservationFreshnessV1::InstrumentFailure => "instrument_failure",
    }
}

fn push_row(
    rows: &mut Vec<FinalFreezeReplayRowV1>,
    kind: FinalFreezeReplayRowKindV1,
    subject: Option<String>,
    message: &str,
) {
    rows.push(FinalFreezeReplayRowV1 {
        kind,
        subject,
        message: message.to_string(),
    });
}

fn append_word_list(output: &mut String, title: &str, words: &[String]) {
    output.push_str(&format!("\n## {title}\n\n"));
    if words.is_empty() {
        output.push_str("None.\n");
        return;
    }
    for word in words {
        output.push_str(&format!("- {}\n", markdown_escape(word)));
    }
}

fn markdown_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use super::super::final_evidence_graph_v1::{
        FINAL_EVIDENCE_EDGE_SCHEMA_ID, FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
        FINAL_EVIDENCE_GRAPH_SCHEMA_ID, FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
        FINAL_EVIDENCE_NODE_SCHEMA_ID, FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
        FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1, FinalEvidenceEdgeV1,
        FinalEvidenceInvalidationDimensionV1, FinalEvidenceNodeResultV1, FinalEvidenceNodeV1,
        FinalEvidenceOriginV1, FinalEvidenceProducerV1, FinalEvidenceSelectedSubjectV1,
        FinalEvidenceSubjectBindingV1, final_evidence_graph_digest,
    };
    use super::super::frozen_candidate_custody_v1::{
        CandidateCustodyInitV1, ConfidentialityClassV1, CustodyFileV1, RetainedCustodyItemV1,
    };
    use super::super::release_artifact_transfer_v1::{
        ArtifactTransferFileV1, ArtifactTransferInitV1, ProducerIdentityV1, TrustClassV1,
        UntrustedInputPostureV1,
    };
    use super::*;

    const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    const TREE: &str = "fedcba9876543210fedcba9876543210fedcba98";
    const VERSION: &str = "0.2.0";
    const TAG: &str = "v0.2.0";
    const CUSTODY_ID: &str = "candidate-custody-0.2.0-final";
    const RECEIPT_ARTIFACT_ID: &str = "final-freeze-receipt";
    const MANIFEST_ARTIFACT_ID: &str = "release-manifest-v2";
    const INCIDENT_HANDOFF_ID: &str = "incident-handoff";
    const REPLAYED_AT: &str = "2026-08-28T00:00:00Z";
    const SOURCE_OBSERVATION: &str = "obs:source-live-control";
    const REGISTRY_OBSERVATION: &str = "obs:registry-feasibility";
    const AMBIENT_OBSERVATION: &str = "obs:ambient-cache";

    struct RetainedFixture {
        inputs: CargoAllowFinalFreezeReplayInputsV1,
    }

    fn digest(seed: u64) -> String {
        format!("sha256:v1:{seed:064x}")
    }

    fn upload_names() -> Vec<String> {
        [
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
        .map(|name| (*name).to_string())
        .collect()
    }

    fn archive_bytes(name: &str) -> Vec<u8> {
        format!("exact-archive-bytes:{name}:{VERSION}").into_bytes()
    }

    fn archive_digest(name: &str) -> String {
        allow_core::sha256_v1_bytes(&archive_bytes(name))
    }

    fn manifest_bytes() -> Vec<u8> {
        format!("exact-manifest-bytes:{VERSION}").into_bytes()
    }

    fn shared_rows() -> Vec<FinalEvidencePackageSubjectV1> {
        [
            ("effortless-repo-edit", "0.1.0"),
            ("effortless-repo-protocol", "0.1.0"),
            ("effortless-repo-snapshot", "0.1.0"),
        ]
        .iter()
        .enumerate()
        .map(|(index, (name, version))| FinalEvidencePackageSubjectV1 {
            logical_id: (*name).to_string(),
            package_name: (*name).to_string(),
            version: (*version).to_string(),
            role: FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            expected_digest: digest(300 + index as u64),
            observed_digest: Some(digest(300 + index as u64)),
        })
        .collect()
    }

    fn upload_rows() -> Vec<FinalEvidencePackageSubjectV1> {
        upload_names()
            .iter()
            .map(|name| FinalEvidencePackageSubjectV1 {
                logical_id: name.clone(),
                package_name: name.clone(),
                version: VERSION.to_string(),
                role: FinalEvidencePackageRoleV1::UploadCandidate,
                expected_digest: archive_digest(name),
                observed_digest: Some(archive_digest(name)),
            })
            .collect()
    }

    fn package_rows() -> Vec<FinalEvidencePackageSubjectV1> {
        let mut rows = upload_rows();
        rows.extend(shared_rows());
        rows
    }

    fn binding() -> FinalEvidenceSubjectBindingV1 {
        FinalEvidenceSubjectBindingV1 {
            repository: REPOSITORY.to_string(),
            commit: Some(COMMIT.to_string()),
            tree: Some(TREE.to_string()),
            cargo_lock_digest: Some(digest(1)),
            topology_digest: Some(digest(2)),
            release_identity: Some(FinalEvidenceReleaseIdentityV1 {
                version: VERSION.to_string(),
                tag: TAG.to_string(),
                github_prerelease: false,
            }),
            package_rows: Vec::new(),
        }
    }

    fn node(
        evidence_id: &str,
        class: FinalEvidenceNodeClassV1,
        origin: FinalEvidenceOriginV1,
        authority: FinalEvidenceAuthorityScopeV1,
        required: bool,
    ) -> FinalEvidenceNodeV1 {
        FinalEvidenceNodeV1 {
            schema_id: FINAL_EVIDENCE_NODE_SCHEMA_ID.to_string(),
            schema_version: FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
            evidence_id: evidence_id.to_string(),
            class,
            origin,
            authority_scope: authority,
            required,
            producer: FinalEvidenceProducerV1 {
                producer_id: format!("producer:{evidence_id}"),
                tool: "cargo-allow".to_string(),
                generation: 1,
                identity_digest: digest(9_000),
                workflow_path: Some(".github/workflows/release.yml".to_string()),
                workflow_run_id: Some(7),
                workflow_attempt: Some(1),
                job: Some(evidence_id.to_string()),
            },
            producer_expectation: None,
            subject: binding(),
            semantic_digest: digest(3_000),
            expected_semantic_digest: Some(digest(3_000)),
            artifact_digest: Some(digest(4_000)),
            expected_artifact_digest: Some(digest(4_000)),
            result: FinalEvidenceNodeResultV1::Complete,
            currentness: FinalEvidenceCurrentnessV1::Current,
            invalidation_dimensions: vec![FinalEvidenceInvalidationDimensionV1::Source],
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
            claim_boundary: format!("{from} supplies the selected {kind:?} relationship to {to}."),
        }
    }

    fn evidence_graph() -> FinalEvidenceGraphV1 {
        let nodes = vec![
            node(
                "package-archive",
                FinalEvidenceNodeClassV1::PackageArchive,
                FinalEvidenceOriginV1::CandidateBytes,
                FinalEvidenceAuthorityScopeV1::FinalExact,
                true,
            ),
            node(
                "installed-journey",
                FinalEvidenceNodeClassV1::InstalledJourney,
                FinalEvidenceOriginV1::WorkflowArtifact,
                FinalEvidenceAuthorityScopeV1::FinalExact,
                true,
            ),
            node(
                "support-selection",
                FinalEvidenceNodeClassV1::SupportSelection,
                FinalEvidenceOriginV1::SourceAuthority,
                FinalEvidenceAuthorityScopeV1::FinalExact,
                true,
            ),
            node(
                "manifest-result",
                FinalEvidenceNodeClassV1::ManifestResult,
                FinalEvidenceOriginV1::WorkflowArtifact,
                FinalEvidenceAuthorityScopeV1::FinalExact,
                true,
            ),
            node(
                INCIDENT_HANDOFF_ID,
                FinalEvidenceNodeClassV1::IncidentHandoff,
                FinalEvidenceOriginV1::HistoricalObservation,
                FinalEvidenceAuthorityScopeV1::HistoricalIncident,
                false,
            ),
        ];
        let required = nodes
            .iter()
            .filter(|node| node.required)
            .map(|node| node.evidence_id.clone())
            .collect();
        FinalEvidenceGraphV1 {
            schema_id: FINAL_EVIDENCE_GRAPH_SCHEMA_ID.to_string(),
            schema_version: FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
            mode: FinalEvidenceGraphModeV1::Production,
            repository: REPOSITORY.to_string(),
            selected_subject: FinalEvidenceSelectedSubjectV1 {
                repository: REPOSITORY.to_string(),
                commit: COMMIT.to_string(),
                tree: TREE.to_string(),
                cargo_lock_digest: digest(1),
                topology_digest: digest(2),
                release_identity: FinalEvidenceReleaseIdentityV1 {
                    version: VERSION.to_string(),
                    tag: TAG.to_string(),
                    github_prerelease: false,
                },
                expected_upload_rows: FINAL_FREEZE_EXPECTED_UPLOAD_ROWS_V1,
                expected_shared_rows: FINAL_FREEZE_EXPECTED_SHARED_ROWS_V1,
                package_rows: package_rows(),
            },
            required_node_ids: required,
            nodes,
            edges: vec![
                edge(
                    "package-archive",
                    "installed-journey",
                    FinalEvidenceEdgeKindV1::ProducedFrom,
                ),
                edge(
                    "support-selection",
                    "installed-journey",
                    FinalEvidenceEdgeKindV1::Projects,
                ),
                edge(
                    "manifest-result",
                    "installed-journey",
                    FinalEvidenceEdgeKindV1::ConsumedBy,
                ),
            ],
            limitations: Vec::new(),
            claim_boundary: "Exact final-release evidence fixture.".to_string(),
        }
    }

    fn freeze_receipt(graph: &FinalEvidenceGraphV1) -> CargoAllowFinalFreezeReceiptV1 {
        let recorded_graph_digest = final_evidence_graph_digest(graph)
            .map_err(|error| format!("fixture graph digest failed: {error}"))
            .unwrap_or_else(|error| error);
        CargoAllowFinalFreezeReceiptV1::new(FinalFreezeReceiptInitV1 {
            freeze_id: "freeze-0.2.0-final".to_string(),
            frozen_custody_id: CUSTODY_ID.to_string(),
            frozen_at_utc: "2026-08-26T12:00:00Z".to_string(),
            release_identity: graph.selected_subject.release_identity.clone(),
            repository: REPOSITORY.to_string(),
            commit: COMMIT.to_string(),
            tree: TREE.to_string(),
            cargo_lock_digest: digest(1),
            topology_digest: digest(2),
            expected_upload_rows: FINAL_FREEZE_EXPECTED_UPLOAD_ROWS_V1,
            expected_shared_rows: FINAL_FREEZE_EXPECTED_SHARED_ROWS_V1,
            package_rows: package_rows(),
            prepublication_manifest: FinalFreezeManifestBindingV1 {
                result: FinalFreezeManifestResultV1::Exact,
                artifact_id: MANIFEST_ARTIFACT_ID.to_string(),
                payload_sha256: allow_core::sha256_v1_bytes(&manifest_bytes()),
            },
            rc1_excluded: true,
            rc1_version: Some("0.2.0-rc.1".to_string()),
            incident_handoff_id: Some(INCIDENT_HANDOFF_ID.to_string()),
            recorded_graph_digest,
            remaining_irreversible_operations: vec![
                "push tag v0.2.0".to_string(),
                "upload 10 package rows to crates.io".to_string(),
                "publish the GitHub release".to_string(),
            ],
        })
    }

    fn custody_item(
        role: &str,
        artifact_id: &str,
        path: &str,
        payload: &[u8],
    ) -> RetainedCustodyItemV1 {
        let sha256 = allow_core::sha256_v1_bytes(payload);
        RetainedCustodyItemV1 {
            role: role.to_string(),
            artifact_id: artifact_id.to_string(),
            files: vec![CustodyFileV1 {
                path: path.to_string(),
                size_bytes: payload.len() as u64,
                sha256: sha256.clone(),
            }],
            storage_locator: format!("s3://release-custody-2026/0.2.0/{artifact_id}"),
            retention_expiry_utc: "2027-01-01T00:00:00Z".to_string(),
            readback_verified: true,
            readback_sha256: Some(sha256),
            confidentiality_class: ConfidentialityClassV1::Public,
        }
    }

    fn retained_artifact(
        role: &str,
        artifact_id: &str,
        payload: Vec<u8>,
    ) -> RetainedExactArtifactV1 {
        RetainedExactArtifactV1 {
            role: role.to_string(),
            artifact_id: artifact_id.to_string(),
            declared_sha256: allow_core::sha256_v1_bytes(&payload),
            bytes: RetainedArtifactBytesV1::new(payload),
        }
    }

    fn transfer_envelope(artifact_id: &str, payload: &[u8]) -> CargoAllowReleaseArtifactTransferV1 {
        CargoAllowReleaseArtifactTransferV1::new(ArtifactTransferInitV1 {
            transfer_id: format!("transfer:{artifact_id}"),
            role: PACKAGE_ARCHIVE_ROLE.to_string(),
            stable_artifact_id: artifact_id.to_string(),
            producer: ProducerIdentityV1 {
                repository: REPOSITORY.to_string(),
                workflow_path: ".github/workflows/release.yml".to_string(),
                git_ref: format!("refs/tags/{TAG}"),
                run_id: 7,
                run_attempt: 1,
                job_id: format!("job:{artifact_id}"),
                commit_sha: COMMIT.to_string(),
                tree_sha: TREE.to_string(),
                release_version: VERSION.to_string(),
                tool_name: "cargo-allow".to_string(),
                schema_id: "cargo-allow.release-artifact-transfer.v1".to_string(),
                producer_generation: 1,
            },
            provider_id: "github-actions".to_string(),
            provider_artifact_name: artifact_id.to_string(),
            files: vec![ArtifactTransferFileV1 {
                path: format!("{artifact_id}.bin"),
                size_bytes: payload.len() as u64,
                sha256: allow_core::sha256_v1_bytes(payload),
            }],
            semantic_payload_digest: None,
            trust_class: TrustClassV1::TagWorkflow,
            untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
            created_at_utc: "2026-08-26T12:00:00Z".to_string(),
        })
    }

    fn fixture() -> Result<RetainedFixture, String> {
        fixture_with(evidence_graph(), |_| {})
    }

    /// Build a fully consistent retained set around a possibly customized
    /// graph, with an optional receipt customization applied before every
    /// derived digest (receipt bytes, custody records, envelopes) is computed.
    fn fixture_with(
        graph: FinalEvidenceGraphV1,
        customize_receipt: impl FnOnce(&mut CargoAllowFinalFreezeReceiptV1),
    ) -> Result<RetainedFixture, String> {
        let mut receipt = freeze_receipt(&graph);
        customize_receipt(&mut receipt);
        let receipt_payload = serde_json::to_vec(&receipt)
            .map_err(|error| format!("fixture receipt serialization failed: {error}"))?;

        let mut artifacts = upload_names()
            .iter()
            .map(|name| retained_artifact(PACKAGE_ARCHIVE_ROLE, name, archive_bytes(name)))
            .collect::<Vec<_>>();
        artifacts.push(retained_artifact(
            FREEZE_RECEIPT_ROLE,
            RECEIPT_ARTIFACT_ID,
            receipt_payload.clone(),
        ));
        artifacts.push(retained_artifact(
            "ReleaseManifest",
            MANIFEST_ARTIFACT_ID,
            manifest_bytes(),
        ));

        let mut envelopes = upload_names()
            .iter()
            .map(|name| transfer_envelope(name, &archive_bytes(name)))
            .collect::<Vec<_>>();
        envelopes.push(transfer_envelope(RECEIPT_ARTIFACT_ID, &receipt_payload));
        envelopes.push(transfer_envelope(MANIFEST_ARTIFACT_ID, &manifest_bytes()));

        let mut items = upload_names()
            .iter()
            .map(|name| {
                custody_item(
                    PACKAGE_ARCHIVE_ROLE,
                    name,
                    &format!("packages/{name}-{VERSION}.crate"),
                    &archive_bytes(name),
                )
            })
            .collect::<Vec<_>>();
        items.push(custody_item(
            FREEZE_RECEIPT_ROLE,
            RECEIPT_ARTIFACT_ID,
            "candidate-freeze.receipt.json",
            &receipt_payload,
        ));
        items.push(custody_item(
            "ReleaseManifest",
            MANIFEST_ARTIFACT_ID,
            "release-manifest.v2.json",
            &manifest_bytes(),
        ));

        let custody = CargoAllowFrozenCandidateCustodyV1::new(CandidateCustodyInitV1 {
            custody_id: CUSTODY_ID.to_string(),
            candidate_version: VERSION.to_string(),
            git_commit: COMMIT.to_string(),
            git_tree: TREE.to_string(),
            items,
            created_at_utc: "2026-08-26T06:00:00Z".to_string(),
        });

        let inputs = CargoAllowFinalFreezeReplayInputsV1 {
            custody,
            evidence_graph: graph,
            freeze_receipt: receipt,
            retained_transfers: envelopes,
            retained_artifacts: artifacts,
            observations: vec![
                RefreshableObservationV1 {
                    observation_id: SOURCE_OBSERVATION.to_string(),
                    kind: RefreshableObservationKindV1::SourceLiveControl,
                    observed_at_utc: "2026-08-27T00:00:00Z".to_string(),
                },
                RefreshableObservationV1 {
                    observation_id: REGISTRY_OBSERVATION.to_string(),
                    kind: RefreshableObservationKindV1::RegistryFeasibility,
                    observed_at_utc: "2026-08-27T00:00:00Z".to_string(),
                },
                RefreshableObservationV1 {
                    observation_id: AMBIENT_OBSERVATION.to_string(),
                    kind: RefreshableObservationKindV1::AmbientCache,
                    observed_at_utc: "2026-08-27T00:00:00Z".to_string(),
                },
            ],
            replayed_at_utc: REPLAYED_AT.to_string(),
        };
        Ok(RetainedFixture { inputs })
    }

    /// Deterministic adapter with per-kind freshness overrides.
    struct FixtureAdapter {
        source: ObservationFreshnessV1,
        registry: ObservationFreshnessV1,
    }

    impl FixtureAdapter {
        fn current() -> Self {
            Self {
                source: ObservationFreshnessV1::Current,
                registry: ObservationFreshnessV1::Current,
            }
        }
    }

    impl RefreshableObservationAdapterV1 for FixtureAdapter {
        fn refresh(&self, observation: &RefreshableObservationV1) -> ObservationReadingV1 {
            let freshness = match observation.kind {
                RefreshableObservationKindV1::SourceLiveControl => self.source,
                RefreshableObservationKindV1::RegistryFeasibility => self.registry,
                RefreshableObservationKindV1::AmbientCache => ObservationFreshnessV1::Current,
            };
            ObservationReadingV1 {
                freshness,
                detail: "fixture reading".to_string(),
            }
        }
    }

    fn replay(fixture: &RetainedFixture) -> CargoAllowFinalFreezeReplayV1 {
        replay_final_freeze(&fixture.inputs, &FixtureAdapter::current())
    }

    fn row_with<'a>(
        replay: &'a CargoAllowFinalFreezeReplayV1,
        needle: &str,
    ) -> Option<&'a FinalFreezeReplayRowV1> {
        replay.rows.iter().find(|row| row.message.contains(needle))
    }

    #[test]
    fn complete_retained_set_replays_complete_equivalent_and_deterministic() -> Result<(), String> {
        let fixture = fixture()?;
        let first = replay(&fixture);
        let second = replay(&fixture);
        if first.result != FinalFreezeReplayResultV1::CompleteEquivalent {
            return Err(format!(
                "expected complete_equivalent, got {:?}",
                first.result
            ));
        }
        if first != second {
            return Err("the replay was not deterministic for the same retained set".to_string());
        }
        if !first.retained_bytes_verified {
            return Err("complete replay did not verify the retained bytes".to_string());
        }
        if first.selected_upload_rows != 10 || first.selected_shared_rows != 3 {
            return Err("the 10+3 denominator was not reconstructed".to_string());
        }
        if first.selected_channel != "stable" {
            return Err("the stable channel identity was not reconstructed".to_string());
        }
        if !first.incident_handoff_present || !first.rc1_excluded {
            return Err(
                "the RC.1 exclusion and incident handoff were not reconstructed".to_string(),
            );
        }
        if first.remaining_irreversible_operations.len() != 3 {
            return Err("remaining irreversible operations were not echoed".to_string());
        }
        Ok(())
    }

    #[test]
    fn removed_archive_is_missing_artifact_and_not_rebuilt_from_source() -> Result<(), String> {
        let mut fixture = fixture()?;
        let removed = "allow-core";
        fixture.inputs.retained_artifacts.retain(|artifact| {
            !(artifact.artifact_id == removed && artifact.role == PACKAGE_ARCHIVE_ROLE)
        });
        fixture
            .inputs
            .retained_transfers
            .retain(|envelope| envelope.stable_artifact_id != removed);
        fixture
            .inputs
            .custody
            .items
            .retain(|item| item.artifact_id != removed);

        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::MissingArtifact {
            return Err(format!(
                "expected missing_artifact, got {:?}",
                replayed.result
            ));
        }
        if !replayed
            .rows
            .iter()
            .any(|row| row.subject.as_deref() == Some(removed))
        {
            return Err("the missing-artifact row did not name the removed archive".to_string());
        }
        Ok(())
    }

    #[test]
    fn ambient_cache_cannot_satisfy_a_missing_archive() -> Result<(), String> {
        let mut fixture = fixture()?;
        fixture
            .inputs
            .retained_artifacts
            .retain(|artifact| artifact.artifact_id != "allow-core");
        fixture
            .inputs
            .retained_transfers
            .retain(|envelope| envelope.stable_artifact_id != "allow-core");
        fixture
            .inputs
            .custody
            .items
            .retain(|item| item.artifact_id != "allow-core");

        let replayed = replay(&fixture);
        let ambient = replayed
            .observation_readings
            .iter()
            .find(|reading| reading.observation_id == AMBIENT_OBSERVATION)
            .ok_or_else(|| "the ambient cache reading is missing".to_string())?;
        if ambient.authoritative {
            return Err("an ambient cache reading became authoritative".to_string());
        }
        if replayed.result != FinalFreezeReplayResultV1::MissingArtifact {
            return Err(format!(
                "an ambient cache satisfied a missing artifact, got {:?}",
                replayed.result
            ));
        }
        Ok(())
    }

    #[test]
    fn modified_retained_receipt_is_caught_by_digest() -> Result<(), String> {
        let mut fixture = fixture()?;
        fixture.inputs.freeze_receipt.expected_shared_rows = 4;
        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::Mismatch {
            return Err(format!("expected mismatch, got {:?}", replayed.result));
        }
        if row_with(&replayed, "no longer hashes to its retained receipt bytes").is_none() {
            return Err("the modified receipt was not caught by digest recomputation".to_string());
        }
        Ok(())
    }

    #[test]
    fn earlier_custody_aggregate_with_same_version_is_rejected() -> Result<(), String> {
        let mut fixture = fixture()?;
        fixture.inputs.custody.custody_id = "candidate-custody-0.2.0-earlier".to_string();
        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::Mismatch {
            return Err(format!("expected mismatch, got {:?}", replayed.result));
        }
        if row_with(
            &replayed,
            "not the custody aggregate bound by the freeze receipt",
        )
        .is_none()
        {
            return Err("the earlier custody aggregate was not rejected".to_string());
        }
        Ok(())
    }

    #[test]
    fn altered_graph_edges_are_caught_even_with_leaf_bytes_preserved() -> Result<(), String> {
        let mut fixture = fixture()?;
        let first_edge = fixture
            .inputs
            .evidence_graph
            .edges
            .first_mut()
            .ok_or_else(|| "fixture lost its first edge".to_string())?;
        first_edge.kind = FinalEvidenceEdgeKindV1::SupportsOnly;
        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::Mismatch {
            return Err(format!("expected mismatch, got {:?}", replayed.result));
        }
        if row_with(
            &replayed,
            "no longer hashes to the freeze receipt's recorded graph digest",
        )
        .is_none()
        {
            return Err("the altered edges were not caught by the graph digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn expired_registry_observation_forces_stale() -> Result<(), String> {
        let fixture = fixture()?;
        let adapter = FixtureAdapter {
            source: ObservationFreshnessV1::Current,
            registry: ObservationFreshnessV1::Stale,
        };
        let replayed = replay_final_freeze(&fixture.inputs, &adapter);
        if replayed.result != FinalFreezeReplayResultV1::Stale {
            return Err(format!(
                "an expired registry observation must force stale, got {:?}",
                replayed.result
            ));
        }
        Ok(())
    }

    #[test]
    fn provider_unavailable_observation_maps_to_provider_unavailable() -> Result<(), String> {
        let fixture = fixture()?;
        let adapter = FixtureAdapter {
            source: ObservationFreshnessV1::ProviderUnavailable,
            registry: ObservationFreshnessV1::Current,
        };
        let replayed = replay_final_freeze(&fixture.inputs, &adapter);
        if replayed.result != FinalFreezeReplayResultV1::ProviderUnavailable {
            return Err(format!(
                "expected provider_unavailable, got {:?}",
                replayed.result
            ));
        }
        Ok(())
    }

    #[test]
    fn rc1_custody_replayed_as_final_is_a_mismatch() -> Result<(), String> {
        let mut fixture = fixture()?;
        fixture.inputs.custody.candidate_version = "0.2.0-rc.1".to_string();
        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::Mismatch {
            return Err(format!(
                "rc.1 custody replayed as final must mismatch, got {:?}",
                replayed.result
            ));
        }
        if row_with(
            &replayed,
            "does not bind the freeze receipt's exact candidate identity",
        )
        .is_none()
        {
            return Err("the rc.1 custody row did not name the identity binding".to_string());
        }
        Ok(())
    }

    #[test]
    fn omitted_remaining_irreversible_operations_is_incomplete() -> Result<(), String> {
        let fixture = fixture_with(evidence_graph(), |receipt| {
            receipt.remaining_irreversible_operations = Vec::new();
        })?;
        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::Incomplete {
            return Err(format!("expected incomplete, got {:?}", replayed.result));
        }
        if row_with(&replayed, "records no remaining irreversible operations").is_none() {
            return Err("the empty operation list was not flagged".to_string());
        }
        Ok(())
    }

    #[test]
    fn scrambled_operations_echo_canonically() -> Result<(), String> {
        let fixture = fixture_with(evidence_graph(), |receipt| {
            receipt.remaining_irreversible_operations.reverse();
        })?;
        let replayed = replay(&fixture);
        let expected = [
            "publish the GitHub release".to_string(),
            "push tag v0.2.0".to_string(),
            "upload 10 package rows to crates.io".to_string(),
        ];
        if replayed.remaining_irreversible_operations != expected {
            return Err("the echoed operations were not canonical".to_string());
        }
        Ok(())
    }

    #[test]
    fn incomplete_evidence_cannot_be_strengthened() -> Result<(), String> {
        let mut graph = evidence_graph();
        let journey = graph
            .nodes
            .iter_mut()
            .find(|node| node.evidence_id == "installed-journey")
            .ok_or_else(|| "fixture lost its installed-journey node".to_string())?;
        journey.result = FinalEvidenceNodeResultV1::NotProven;
        let fixture = fixture_with(graph, |_| {})?;
        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::Incomplete {
            return Err(format!(
                "not-proven evidence must stay incomplete, got {:?}",
                replayed.result
            ));
        }
        if replayed.evidence_result != FinalEvidenceEvaluationResultV1::Incomplete {
            return Err("the evidence verdict was not retained verbatim".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_incident_handoff_row_is_incomplete() -> Result<(), String> {
        let mut graph = evidence_graph();
        graph
            .nodes
            .retain(|node| node.evidence_id != INCIDENT_HANDOFF_ID);
        let fixture = fixture_with(graph, |_| {})?;
        let replayed = replay(&fixture);
        if replayed.result != FinalFreezeReplayResultV1::Incomplete {
            return Err(format!("expected incomplete, got {:?}", replayed.result));
        }
        if replayed.incident_handoff_present {
            return Err("a missing handoff was reported present".to_string());
        }
        Ok(())
    }

    #[test]
    fn retained_input_set_roundtrips_through_serde() -> Result<(), String> {
        let fixture = fixture()?;
        let json = serde_json::to_string(&fixture.inputs)
            .map_err(|error| format!("input serialization failed: {error}"))?;
        let parsed: CargoAllowFinalFreezeReplayInputsV1 = serde_json::from_str(&json)
            .map_err(|error| format!("input parsing failed: {error}"))?;
        let direct = replay_final_freeze(&fixture.inputs, &FixtureAdapter::current());
        let roundtripped = replay_final_freeze(&parsed, &FixtureAdapter::current());
        if direct != roundtripped {
            return Err(
                "the replay depended on something outside the retained input set".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn projections_carry_the_result_and_claim_boundary() -> Result<(), String> {
        let fixture = fixture()?;
        let replayed = replay(&fixture);
        let markdown = render_final_freeze_replay_markdown(&replayed);
        if !markdown.contains("complete_equivalent") || !markdown.contains("Claim boundary") {
            return Err("the markdown projection lost the result or claim boundary".to_string());
        }
        let json = render_final_freeze_replay_json(&replayed)
            .map_err(|error| format!("json projection failed: {error}"))?;
        if !json.contains(FINAL_FREEZE_REPLAY_SCHEMA_ID) {
            return Err("the json projection lost the schema id".to_string());
        }
        if !replayed
            .claim_boundary
            .contains("never tags, uploads, publishes, authorizes")
        {
            return Err("the replay claim boundary lost the no-mutation statement".to_string());
        }
        Ok(())
    }
}
