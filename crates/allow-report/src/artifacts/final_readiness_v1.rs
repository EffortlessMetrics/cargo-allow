//! Deterministic final pre-freeze readiness aggregate over the exact current
//! evidence graph (#3929).
//!
//! The aggregate consumes one exact `FinalEvidenceGraphV1` plus the current
//! explicit campaign/support decision inputs and compiles one bounded verdict:
//! whether the selected campaign subject may enter candidate freeze (#2501).
//! Issue state, merged PRs, green CI, labels, checklists, and prose never
//! substitute for a receipt or evidence result; every blocking, stale,
//! mismatched, not-proven, unsupported, provider, instrument, and decision row
//! carries one exact owner and next action. The aggregate is pure: it never
//! creates package bytes, freeze receipts, authorization, tags, uploads, or
//! any other release state.

use super::final_evidence_graph_v1::{
    FinalEvidenceAuthorityScopeV1, FinalEvidenceCurrentnessV1, FinalEvidenceFindingKindV1,
    FinalEvidenceGraphEvaluationV1, FinalEvidenceGraphV1, FinalEvidenceNodeClassV1,
    FinalEvidenceNodeDispositionV1, FinalEvidenceNodeResultV1, FinalEvidenceReleaseIdentityV1,
    evaluate_final_evidence_graph,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FINAL_READINESS_SCHEMA_ID: &str = "cargo-allow.final-readiness.v1";
pub const FINAL_READINESS_SCHEMA_VERSION: u32 = 1;

const CLAIM_BOUNDARY: &str = "A deterministic final pre-freeze readiness verdict compiled from the exact current evidence graph, explicit campaign and support decisions, incident exclusions, and the post-merge subject. It decides only whether candidate freeze may begin; it does not generate package bytes, freeze, mint authorization, tag, upload, publish, or create any release state.";

/// Closed readiness verdict vocabulary for the pre-freeze decision surface.
/// `ReadyForFreeze` requires every load-bearing evidence node to be exact and
/// current, every required root decision to be explicitly recorded, and no
/// blocking row to remain; it is impossible from narration alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalReadinessVerdictV1 {
    ReadyForFreeze,
    Incomplete,
    Stale,
    Mismatch,
    NeedsDecision,
    ProviderUnavailable,
    Unsupported,
    InstrumentFailure,
    NotProven,
}

impl FinalReadinessVerdictV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReadyForFreeze => "ready_for_freeze",
            Self::Incomplete => "incomplete",
            Self::Stale => "stale",
            Self::Mismatch => "mismatch",
            Self::NeedsDecision => "needs_decision",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::Unsupported => "unsupported",
            Self::InstrumentFailure => "instrument_failure",
            Self::NotProven => "not_proven",
        }
    }
}

/// Closed row vocabulary. Every row kind except `ClaimNarrowed` blocks
/// `ReadyForFreeze`; `ClaimNarrowed` records an explicitly permitted
/// support-only narrowing that never becomes proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalReadinessRowKindV1 {
    MissingEvidence,
    Stale,
    Mismatch,
    NotProven,
    Unsupported,
    ProviderUnavailable,
    InstrumentFailure,
    DecisionRequired,
    ClaimNarrowed,
    CustodyExpiring,
}

impl FinalReadinessRowKindV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MissingEvidence => "missing_evidence",
            Self::Stale => "stale",
            Self::Mismatch => "mismatch",
            Self::NotProven => "not_proven",
            Self::Unsupported => "unsupported",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InstrumentFailure => "instrument_failure",
            Self::DecisionRequired => "decision_required",
            Self::ClaimNarrowed => "claim_narrowed",
            Self::CustodyExpiring => "custody_expiring",
        }
    }

    /// The verdict a single row of this kind forces, if any.
    const fn forced_verdict(self) -> Option<FinalReadinessVerdictV1> {
        match self {
            Self::Mismatch => Some(FinalReadinessVerdictV1::Mismatch),
            Self::ProviderUnavailable => Some(FinalReadinessVerdictV1::ProviderUnavailable),
            Self::InstrumentFailure => Some(FinalReadinessVerdictV1::InstrumentFailure),
            Self::Stale | Self::CustodyExpiring => Some(FinalReadinessVerdictV1::Stale),
            Self::Unsupported => Some(FinalReadinessVerdictV1::Unsupported),
            Self::DecisionRequired => Some(FinalReadinessVerdictV1::NeedsDecision),
            Self::MissingEvidence => Some(FinalReadinessVerdictV1::Incomplete),
            Self::NotProven => Some(FinalReadinessVerdictV1::NotProven),
            Self::ClaimNarrowed => None,
        }
    }

    /// Deterministic precedence when rows of several kinds coexist: the
    /// smallest rank wins. Ties are impossible for distinct kinds because the
    /// mapping above is injective on the forced verdicts except for the
    /// `Stale`/`CustodyExpiring` pair, which forces the same verdict.
    const fn precedence(self) -> u8 {
        match self {
            Self::Mismatch => 0,
            Self::ProviderUnavailable => 1,
            Self::InstrumentFailure => 2,
            Self::Stale => 3,
            Self::CustodyExpiring => 4,
            Self::Unsupported => 5,
            Self::DecisionRequired => 6,
            Self::MissingEvidence => 7,
            Self::NotProven => 8,
            Self::ClaimNarrowed => 9,
        }
    }
}

/// One exact owner-and-next-action row. `owner` and `next_action` are never
/// empty on emitted rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessRowV1 {
    pub kind: FinalReadinessRowKindV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
    pub message: String,
    pub owner: String,
    pub next_action: String,
}

/// Explicit state of one root campaign/support decision. Decisions are human
/// facts and can never be inferred from implementation or evidence state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalReadinessDecisionStateV1 {
    Decided,
    Missing,
}

/// One root decision input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessRootDecisionV1 {
    pub decision_id: String,
    pub owner: String,
    pub state: FinalReadinessDecisionStateV1,
    pub required: bool,
}

/// One supported-limitation disposition. A supported limitation is valid only
/// with a selected user-facing support/claim projection and an exact owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessSupportedLimitationV1 {
    pub limitation_id: String,
    pub user_facing_projection: Option<String>,
    pub owner: Option<String>,
}

/// An explicit support decision permitting one `NotProven` evidence node to
/// narrow its claim. The narrowed claim stays out of proof narration forever.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessClaimNarrowingV1 {
    pub evidence_id: String,
    pub permitted_by_decision: String,
    pub owner: String,
}

/// Post-merge qualification posture for the reviewed release subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalReadinessQualificationPostureV1 {
    Current,
    RequiresRerun,
}

/// Exact merged-main subject and its requalification posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessPostMergePostureV1 {
    pub merge_commit: String,
    pub merge_subject_current: bool,
    pub qualification: FinalReadinessQualificationPostureV1,
    pub owner: String,
}

/// Current custody/replay feasibility posture for the frozen candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessCustodyPostureV1 {
    pub replay_feasible: bool,
    pub expires_before_authorization_window: bool,
    pub owner: String,
}

/// Explicit campaign/support decision inputs consumed beside the graph.
/// Nothing here is derived from issue state, CI, labels, or prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessDecisionInputsV1 {
    /// Owner for graph-level rows (rebuild/repair responsibility).
    pub graph_owner: String,
    pub root_decisions: Vec<FinalReadinessRootDecisionV1>,
    pub supported_limitations: Vec<FinalReadinessSupportedLimitationV1>,
    pub permitted_claim_narrowings: Vec<FinalReadinessClaimNarrowingV1>,
    pub post_merge: FinalReadinessPostMergePostureV1,
    pub custody: FinalReadinessCustodyPostureV1,
    pub remaining_reversible_work: Vec<String>,
    pub remaining_irreversible_operations: Vec<String>,
}

/// Exact result class retained for one required evidence node. Only exact
/// current graph nodes satisfy required rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReadinessRequiredEvidenceV1 {
    pub evidence_id: String,
    pub class: FinalEvidenceNodeClassV1,
    pub result: FinalEvidenceNodeResultV1,
    pub currentness: FinalEvidenceCurrentnessV1,
}

/// The versioned final pre-freeze readiness aggregate (`#3929`). Human and
/// machine projections derive from this one typed value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFinalReadinessV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub graph_digest: String,
    pub verdict: FinalReadinessVerdictV1,
    pub repository: String,
    pub release_identity: FinalEvidenceReleaseIdentityV1,
    pub selected_upload_rows: u32,
    pub selected_shared_rows: u32,
    pub selected_package_rows: u32,
    pub merge_commit: String,
    pub post_merge_qualification: FinalReadinessQualificationPostureV1,
    pub custody_replay_feasible: bool,
    pub custody_expires_before_authorization_window: bool,
    pub support_only_evidence_ids: Vec<String>,
    pub supported_limitation_ids: Vec<String>,
    pub required_evidence: Vec<FinalReadinessRequiredEvidenceV1>,
    pub rows: Vec<FinalReadinessRowV1>,
    pub remaining_reversible_work: Vec<String>,
    pub remaining_irreversible_operations: Vec<String>,
    pub claim_boundary: String,
}

/// Compile the final pre-freeze readiness verdict from the exact current
/// evidence graph plus explicit campaign/support decisions, without I/O.
#[must_use]
pub fn aggregate_final_readiness(
    graph: &FinalEvidenceGraphV1,
    inputs: &FinalReadinessDecisionInputsV1,
) -> CargoAllowFinalReadinessV1 {
    let evaluation = evaluate_final_evidence_graph(graph);
    let mut rows = Vec::new();

    collect_graph_rows(&evaluation, inputs, &mut rows);
    collect_node_rows(&evaluation, inputs, &mut rows);
    collect_decision_rows(inputs, &mut rows);
    collect_post_merge_rows(inputs, &mut rows);
    collect_custody_rows(inputs, &mut rows);

    rows.sort_by(|left, right| {
        (
            left.kind.precedence(),
            left.kind,
            left.evidence_id.as_deref(),
            left.message.as_str(),
        )
            .cmp(&(
                right.kind.precedence(),
                right.kind,
                right.evidence_id.as_deref(),
                right.message.as_str(),
            ))
    });

    let verdict = rows
        .iter()
        .filter_map(|row| row.kind.forced_verdict())
        .min_by_key(|verdict| verdict_rank(*verdict))
        .unwrap_or(FinalReadinessVerdictV1::ReadyForFreeze);

    let support_only_evidence_ids = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.authority_scope,
                FinalEvidenceAuthorityScopeV1::SupportOnly
                    | FinalEvidenceAuthorityScopeV1::HistoricalIncident
            )
        })
        .map(|node| node.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let supported_limitation_ids = inputs
        .supported_limitations
        .iter()
        .filter(|limitation| supported_limitation_is_valid(limitation, inputs))
        .map(|limitation| limitation.limitation_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let required_ids = graph
        .required_node_ids
        .iter()
        .chain(
            graph
                .nodes
                .iter()
                .filter(|node| node.required)
                .map(|node| &node.evidence_id),
        )
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut required_evidence = graph
        .nodes
        .iter()
        .filter(|node| required_ids.contains(&node.evidence_id))
        .map(|node| FinalReadinessRequiredEvidenceV1 {
            evidence_id: node.evidence_id.clone(),
            class: node.class,
            result: node.result,
            currentness: node.currentness,
        })
        .collect::<Vec<_>>();
    required_evidence.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));

    CargoAllowFinalReadinessV1 {
        schema_id: FINAL_READINESS_SCHEMA_ID.to_string(),
        schema_version: FINAL_READINESS_SCHEMA_VERSION,
        graph_digest: evaluation.graph_digest.clone(),
        verdict,
        repository: graph.repository.clone(),
        release_identity: graph.selected_subject.release_identity.clone(),
        selected_upload_rows: graph.selected_subject.expected_upload_rows,
        selected_shared_rows: graph.selected_subject.expected_shared_rows,
        selected_package_rows: graph.selected_subject.package_rows.len() as u32,
        merge_commit: inputs.post_merge.merge_commit.clone(),
        post_merge_qualification: inputs.post_merge.qualification,
        custody_replay_feasible: inputs.custody.replay_feasible,
        custody_expires_before_authorization_window: inputs
            .custody
            .expires_before_authorization_window,
        support_only_evidence_ids,
        supported_limitation_ids,
        required_evidence,
        rows,
        remaining_reversible_work: canonical_words(&inputs.remaining_reversible_work),
        remaining_irreversible_operations: canonical_words(
            &inputs.remaining_irreversible_operations,
        ),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

/// Render a readiness aggregate as JSON (machine projection).
pub fn render_final_readiness_json(
    readiness: &CargoAllowFinalReadinessV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(readiness)
}

/// Render a readiness aggregate as deterministic Markdown (human projection).
#[must_use]
pub fn render_final_readiness_markdown(readiness: &CargoAllowFinalReadinessV1) -> String {
    let mut output = String::new();
    output.push_str("# Final readiness\n\n");
    output.push_str(&format!(
        "- Verdict: `{}`\n",
        markdown_escape(readiness.verdict.label())
    ));
    output.push_str(&format!(
        "- Graph digest: `{}`\n",
        markdown_escape(&readiness.graph_digest)
    ));
    output.push_str(&format!(
        "- Release identity: `{}` / `{}`\n",
        markdown_escape(&readiness.release_identity.version),
        markdown_escape(&readiness.release_identity.tag)
    ));
    output.push_str(&format!(
        "- Selected denominator: {} upload + {} shared = {} package rows\n",
        readiness.selected_upload_rows,
        readiness.selected_shared_rows,
        readiness.selected_package_rows
    ));
    output.push_str(&format!(
        "- Post-merge subject: `{}` (qualification `{}`)\n",
        markdown_escape(&readiness.merge_commit),
        markdown_escape(qualification_label(readiness.post_merge_qualification))
    ));
    output.push_str(&format!(
        "- Custody: replay {} ; expires before authorization window {}\n\n",
        if readiness.custody_replay_feasible {
            "feasible"
        } else {
            "infeasible"
        },
        if readiness.custody_expires_before_authorization_window {
            "yes"
        } else {
            "no"
        }
    ));

    output.push_str("## Required evidence\n\n");
    output.push_str("| Evidence | Class | Result | Currentness |\n");
    output.push_str("| --- | --- | --- | --- |\n");
    for evidence in &readiness.required_evidence {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            markdown_escape(&evidence.evidence_id),
            markdown_escape(evidence.class.label()),
            markdown_escape(evidence.result.label()),
            markdown_escape(evidence.currentness.label())
        ));
    }

    output.push_str("\n## Rows\n\n");
    if readiness.rows.is_empty() {
        output.push_str("No rows.\n");
    } else {
        output.push_str("| Kind | Evidence | Message | Owner | Next action |\n");
        output.push_str("| --- | --- | --- | --- | --- |\n");
        for row in &readiness.rows {
            output.push_str(&format!(
                "| `{}` | {} | {} | `{}` | {} |\n",
                markdown_escape(row.kind.label()),
                markdown_escape(row.evidence_id.as_deref().unwrap_or("—")),
                markdown_escape(&row.message),
                markdown_escape(&row.owner),
                markdown_escape(&row.next_action)
            ));
        }
    }

    append_word_list(
        &mut output,
        "Remaining reversible work",
        &readiness.remaining_reversible_work,
    );
    append_word_list(
        &mut output,
        "Remaining irreversible operations",
        &readiness.remaining_irreversible_operations,
    );

    output.push_str("\n## Claim boundary\n\n");
    output.push_str(&markdown_escape(&readiness.claim_boundary));
    output.push('\n');
    output
}

fn verdict_rank(verdict: FinalReadinessVerdictV1) -> u8 {
    match verdict {
        FinalReadinessVerdictV1::Mismatch => 0,
        FinalReadinessVerdictV1::ProviderUnavailable => 1,
        FinalReadinessVerdictV1::InstrumentFailure => 2,
        FinalReadinessVerdictV1::Stale => 3,
        FinalReadinessVerdictV1::Unsupported => 4,
        FinalReadinessVerdictV1::NeedsDecision => 5,
        FinalReadinessVerdictV1::Incomplete => 6,
        FinalReadinessVerdictV1::NotProven => 7,
        FinalReadinessVerdictV1::ReadyForFreeze => 8,
    }
}

fn collect_graph_rows(
    evaluation: &FinalEvidenceGraphEvaluationV1,
    inputs: &FinalReadinessDecisionInputsV1,
    rows: &mut Vec<FinalReadinessRowV1>,
) {
    for finding in &evaluation.findings {
        let (kind, message, next_action) = match finding.kind {
            FinalEvidenceFindingKindV1::MissingRequiredNode => (
                FinalReadinessRowKindV1::MissingEvidence,
                "a required evidence node is absent from the graph; issue or CI state cannot supply it",
                "produce the exact evidence node and attach it to the current graph",
            ),
            FinalEvidenceFindingKindV1::InvalidAuthorityUse => (
                FinalReadinessRowKindV1::Mismatch,
                "support-only, historical, or fixture evidence is wired as final authority",
                "rewire the evidence through an explicit support-only edge or remove its final authority",
            ),
            FinalEvidenceFindingKindV1::InvalidSelectedSubject
            | FinalEvidenceFindingKindV1::InvalidPackageGraph
            | FinalEvidenceFindingKindV1::ContradictoryEdge => (
                FinalReadinessRowKindV1::Mismatch,
                "evidence conflicts with the exact selected release subject",
                "reselect or rebuild the conflicting evidence against the exact selected subject",
            ),
            FinalEvidenceFindingKindV1::InvalidProducer => (
                FinalReadinessRowKindV1::Mismatch,
                "producer identity or generation differs from the selected expectation",
                "rerun the producer at the expected generation or reselect the expectation",
            ),
            FinalEvidenceFindingKindV1::InvalidSchema
            | FinalEvidenceFindingKindV1::InvalidDigest
            | FinalEvidenceFindingKindV1::DuplicateNode
            | FinalEvidenceFindingKindV1::DuplicateEdge
            | FinalEvidenceFindingKindV1::UnknownEdgeEndpoint
            | FinalEvidenceFindingKindV1::OrphanRequiredNode
            | FinalEvidenceFindingKindV1::DependencyCycle
            | FinalEvidenceFindingKindV1::InvalidNodeOrigin
            | FinalEvidenceFindingKindV1::MissingRerunOwner => (
                FinalReadinessRowKindV1::MissingEvidence,
                "the supplied evidence graph is structurally unusable",
                "repair the graph input and re-evaluate before freeze",
            ),
            FinalEvidenceFindingKindV1::NonCurrentNode
            | FinalEvidenceFindingKindV1::TransitiveStaleness => continue,
        };
        push_row(
            rows,
            kind,
            finding.evidence_id.clone(),
            message,
            resolve_owner(finding.rerun_owner.as_deref(), &inputs.graph_owner),
            next_action,
        );
    }
}

fn collect_node_rows(
    evaluation: &FinalEvidenceGraphEvaluationV1,
    inputs: &FinalReadinessDecisionInputsV1,
    rows: &mut Vec<FinalReadinessRowV1>,
) {
    for disposition in &evaluation.node_dispositions {
        if !disposition_is_non_current(disposition) {
            continue;
        }
        classify_disposition(disposition, inputs, rows);
    }
}

fn disposition_is_non_current(disposition: &FinalEvidenceNodeDispositionV1) -> bool {
    disposition.direct_non_current || disposition.transitively_stale
}

fn classify_disposition(
    disposition: &FinalEvidenceNodeDispositionV1,
    inputs: &FinalReadinessDecisionInputsV1,
    rows: &mut Vec<FinalReadinessRowV1>,
) {
    let evidence_id = disposition.evidence_id.clone();
    let owner = resolve_owner(disposition.rerun_owner.as_deref(), &inputs.graph_owner);
    let mismatch = disposition.result == FinalEvidenceNodeResultV1::Mismatch
        || disposition.result == FinalEvidenceNodeResultV1::Conflict
        || disposition.result == FinalEvidenceNodeResultV1::Incident
        || disposition.currentness == FinalEvidenceCurrentnessV1::Mismatch;
    let provider = disposition.result == FinalEvidenceNodeResultV1::ProviderUnavailable
        || disposition.currentness == FinalEvidenceCurrentnessV1::ProviderUnavailable;
    let instrument = disposition.result == FinalEvidenceNodeResultV1::InstrumentFailure
        || disposition.currentness == FinalEvidenceCurrentnessV1::InstrumentFailure;
    let stale = disposition.result == FinalEvidenceNodeResultV1::Stale
        || matches!(
            disposition.currentness,
            FinalEvidenceCurrentnessV1::Stale | FinalEvidenceCurrentnessV1::Expired
        )
        || disposition.transitively_stale;

    if mismatch {
        push_row(
            rows,
            FinalReadinessRowKindV1::Mismatch,
            Some(evidence_id),
            "evidence conflicts with, or is excluded from, the selected final subject",
            owner,
            "rebuild the evidence against the exact selected subject",
        );
    } else if provider {
        push_row(
            rows,
            FinalReadinessRowKindV1::ProviderUnavailable,
            Some(evidence_id),
            "the external provider behind this evidence is unavailable",
            owner,
            "restore provider access and re-observe through the exact producer",
        );
    } else if instrument {
        push_row(
            rows,
            FinalReadinessRowKindV1::InstrumentFailure,
            Some(evidence_id),
            "the producing instrument failed for this evidence",
            owner,
            "repair the instrument and rerun on the exact selected subject",
        );
    } else if stale {
        push_row(
            rows,
            FinalReadinessRowKindV1::Stale,
            Some(evidence_id.clone()),
            "evidence is stale, expired, or depends transitively on non-current evidence",
            owner.clone(),
            "rerun this evidence on the exact selected subject",
        );
    } else if disposition.result == FinalEvidenceNodeResultV1::Unsupported {
        push_row(
            rows,
            FinalReadinessRowKindV1::Unsupported,
            Some(evidence_id),
            "the claimed capability is unsupported by the producing stage",
            owner,
            "select a supported support/claim projection or drop the claim",
        );
    } else if disposition.result == FinalEvidenceNodeResultV1::NotProven {
        classify_not_proven(&evidence_id, owner, inputs, rows);
    } else {
        push_row(
            rows,
            FinalReadinessRowKindV1::MissingEvidence,
            Some(evidence_id),
            "required evidence is incomplete or malformed",
            owner,
            "produce the exact evidence result on the selected subject",
        );
    }
}

fn classify_not_proven(
    evidence_id: &str,
    owner: String,
    inputs: &FinalReadinessDecisionInputsV1,
    rows: &mut Vec<FinalReadinessRowV1>,
) {
    let permitted = inputs
        .permitted_claim_narrowings
        .iter()
        .find(|narrowing| narrowing.evidence_id == evidence_id);
    let Some(narrowing) = permitted else {
        push_row(
            rows,
            FinalReadinessRowKindV1::NotProven,
            Some(evidence_id.to_string()),
            "the selected claim is not proven and no support decision permits narrowing it",
            owner,
            "produce exact evidence or record an explicit support narrowing decision",
        );
        return;
    };
    let narrowing_owner = resolve_owner(Some(narrowing.owner.as_str()), &inputs.graph_owner);
    if narrowing_owner.is_empty() || narrowing.permitted_by_decision.trim().is_empty() {
        push_row(
            rows,
            FinalReadinessRowKindV1::NotProven,
            Some(evidence_id.to_string()),
            "the recorded claim narrowing lacks an exact owner or permitting decision id",
            owner,
            "re-record the narrowing with an exact owner and permitting decision",
        );
        return;
    }
    push_row(
        rows,
        FinalReadinessRowKindV1::ClaimNarrowed,
        Some(evidence_id.to_string()),
        "claim narrowed by explicit support decision; NotProven never becomes proof",
        narrowing_owner,
        "keep the narrowed claim out of proof narration and release claims",
    );
}

fn collect_decision_rows(
    inputs: &FinalReadinessDecisionInputsV1,
    rows: &mut Vec<FinalReadinessRowV1>,
) {
    for decision in &inputs.root_decisions {
        if decision.decision_id.trim().is_empty() {
            push_row(
                rows,
                FinalReadinessRowKindV1::DecisionRequired,
                None,
                "a root decision input lacks a stable decision id",
                resolve_owner(Some(decision.owner.as_str()), &inputs.graph_owner),
                "record the decision with a stable id and exact owner",
            );
            continue;
        }
        if decision.required && decision.state == FinalReadinessDecisionStateV1::Missing {
            push_row(
                rows,
                FinalReadinessRowKindV1::DecisionRequired,
                None,
                &format!(
                    "required root decision `{}` is not recorded",
                    decision.decision_id
                ),
                resolve_owner(Some(decision.owner.as_str()), &inputs.graph_owner),
                "record the explicit human decision; it cannot be inferred from implementation or evidence state",
            );
        }
    }

    for limitation in &inputs.supported_limitations {
        if supported_limitation_is_valid(limitation, inputs) {
            continue;
        }
        push_row(
            rows,
            FinalReadinessRowKindV1::Unsupported,
            None,
            &format!(
                "supported limitation `{}` lacks a user-facing projection or exact owner",
                limitation.limitation_id
            ),
            resolve_owner(limitation.owner.as_deref(), &inputs.graph_owner),
            "select the user-facing support/claim projection and exact owner, or withdraw the limitation",
        );
    }
}

fn collect_post_merge_rows(
    inputs: &FinalReadinessDecisionInputsV1,
    rows: &mut Vec<FinalReadinessRowV1>,
) {
    let owner = resolve_owner(Some(inputs.post_merge.owner.as_str()), &inputs.graph_owner);
    if inputs.post_merge.merge_commit.trim().is_empty() {
        push_row(
            rows,
            FinalReadinessRowKindV1::MissingEvidence,
            None,
            "the merged-main subject is missing from the readiness inputs",
            owner.clone(),
            "record the exact merged-main commit subject",
        );
    }
    if !inputs.post_merge.merge_subject_current {
        push_row(
            rows,
            FinalReadinessRowKindV1::Stale,
            None,
            "current main moved past the reviewed release subject",
            owner.clone(),
            "recompute the effective merge base and rerun affected evidence on the merged head",
        );
    }
    if inputs.post_merge.qualification == FinalReadinessQualificationPostureV1::RequiresRerun {
        push_row(
            rows,
            FinalReadinessRowKindV1::Stale,
            None,
            "post-merge qualification requires a rerun; the pre-freeze graph cannot remain accepted",
            owner,
            "rerun the required qualification set on the merged head",
        );
    }
}

fn collect_custody_rows(
    inputs: &FinalReadinessDecisionInputsV1,
    rows: &mut Vec<FinalReadinessRowV1>,
) {
    let owner = resolve_owner(Some(inputs.custody.owner.as_str()), &inputs.graph_owner);
    if !inputs.custody.replay_feasible {
        push_row(
            rows,
            FinalReadinessRowKindV1::MissingEvidence,
            None,
            "custody replay is infeasible for the frozen candidate",
            owner.clone(),
            "restore custody and verify an independent readback before freeze",
        );
    }
    if inputs.custody.expires_before_authorization_window {
        push_row(
            rows,
            FinalReadinessRowKindV1::CustodyExpiring,
            None,
            "custody retention expires before the authorization window",
            owner,
            "extend retention or re-freeze custody before requesting authorization",
        );
    }
}

fn supported_limitation_is_valid(
    limitation: &FinalReadinessSupportedLimitationV1,
    inputs: &FinalReadinessDecisionInputsV1,
) -> bool {
    !limitation.limitation_id.trim().is_empty()
        && limitation
            .user_facing_projection
            .as_deref()
            .is_some_and(|projection| !projection.trim().is_empty())
        && !resolve_owner(limitation.owner.as_deref(), &inputs.graph_owner).is_empty()
}

/// Resolve a row owner: the specific owner when present, otherwise the graph
/// owner. An empty result forces the row to a blocking `MissingEvidence` row
/// so `ReadyForFreeze` can never carry an unnamed row.
fn resolve_owner(specific: Option<&str>, graph_owner: &str) -> String {
    let specific = specific.unwrap_or_default().trim();
    if !specific.is_empty() {
        return specific.to_string();
    }
    graph_owner.trim().to_string()
}

fn push_row(
    rows: &mut Vec<FinalReadinessRowV1>,
    kind: FinalReadinessRowKindV1,
    evidence_id: Option<String>,
    message: &str,
    owner: String,
    next_action: &str,
) {
    if owner.is_empty() {
        rows.push(FinalReadinessRowV1 {
            kind: FinalReadinessRowKindV1::MissingEvidence,
            evidence_id,
            message: format!("{message}; additionally, no exact owner is recorded for this row"),
            owner: "unassigned".to_string(),
            next_action:
                "set the exact owner in the readiness decision inputs, then rerun the aggregate"
                    .to_string(),
        });
        return;
    }
    rows.push(FinalReadinessRowV1 {
        kind,
        evidence_id,
        message: message.to_string(),
        owner,
        next_action: next_action.to_string(),
    });
}

fn canonical_words(words: &[String]) -> Vec<String> {
    let mut canonical = words.to_vec();
    canonical.retain(|word| !word.trim().is_empty());
    canonical.sort();
    canonical.dedup();
    canonical
}

fn qualification_label(posture: FinalReadinessQualificationPostureV1) -> &'static str {
    match posture {
        FinalReadinessQualificationPostureV1::Current => "current",
        FinalReadinessQualificationPostureV1::RequiresRerun => "requires_rerun",
    }
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
    use super::*;
    use crate::artifacts::final_evidence_graph_v1::{
        FINAL_EVIDENCE_EDGE_SCHEMA_ID, FINAL_EVIDENCE_EDGE_SCHEMA_VERSION,
        FINAL_EVIDENCE_GRAPH_SCHEMA_ID, FINAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
        FINAL_EVIDENCE_NODE_SCHEMA_ID, FINAL_EVIDENCE_NODE_SCHEMA_VERSION, FinalEvidenceEdgeKindV1,
        FinalEvidenceEdgeV1, FinalEvidenceGraphModeV1, FinalEvidenceInvalidationDimensionV1,
        FinalEvidenceNodeV1, FinalEvidenceOriginV1, FinalEvidencePackageRoleV1,
        FinalEvidencePackageSubjectV1, FinalEvidenceProducerV1, FinalEvidenceSelectedSubjectV1,
        FinalEvidenceSubjectBindingV1,
    };

    const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";

    fn digest(seed: u64) -> String {
        format!("sha256:v1:{seed:064x}")
    }

    fn package_rows() -> Vec<FinalEvidencePackageSubjectV1> {
        let names = [
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
        let mut rows = names
            .iter()
            .enumerate()
            .map(|(index, name)| FinalEvidencePackageSubjectV1 {
                logical_id: (*name).to_string(),
                package_name: (*name).to_string(),
                version: "0.2.0".to_string(),
                role: FinalEvidencePackageRoleV1::UploadCandidate,
                expected_digest: digest(100 + index as u64),
                observed_digest: Some(digest(100 + index as u64)),
            })
            .collect::<Vec<_>>();
        rows.push(FinalEvidencePackageSubjectV1 {
            logical_id: "repo-edit".to_string(),
            package_name: "effortless-repo-edit".to_string(),
            version: "0.1.0".to_string(),
            role: FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            expected_digest: digest(201),
            observed_digest: Some(digest(201)),
        });
        rows
    }

    fn selected_subject() -> FinalEvidenceSelectedSubjectV1 {
        FinalEvidenceSelectedSubjectV1 {
            repository: REPOSITORY.to_string(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            tree: "fedcba9876543210fedcba9876543210fedcba98".to_string(),
            cargo_lock_digest: digest(1),
            topology_digest: digest(2),
            release_identity: FinalEvidenceReleaseIdentityV1 {
                version: "0.2.0".to_string(),
                tag: "v0.2.0".to_string(),
                github_prerelease: false,
            },
            expected_upload_rows: 10,
            expected_shared_rows: 1,
            package_rows: package_rows(),
        }
    }

    fn binding(subject: &FinalEvidenceSelectedSubjectV1) -> FinalEvidenceSubjectBindingV1 {
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

    fn node(evidence_id: &str, class: FinalEvidenceNodeClassV1) -> FinalEvidenceNodeV1 {
        let subject = selected_subject();
        let origin = match class {
            FinalEvidenceNodeClassV1::PackageArchive => FinalEvidenceOriginV1::CandidateBytes,
            FinalEvidenceNodeClassV1::SupportSelection => FinalEvidenceOriginV1::SourceAuthority,
            _ => FinalEvidenceOriginV1::WorkflowArtifact,
        };
        FinalEvidenceNodeV1 {
            schema_id: FINAL_EVIDENCE_NODE_SCHEMA_ID.to_string(),
            schema_version: FINAL_EVIDENCE_NODE_SCHEMA_VERSION,
            evidence_id: evidence_id.to_string(),
            class,
            origin,
            authority_scope: FinalEvidenceAuthorityScopeV1::FinalExact,
            required: true,
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
            subject: binding(&subject),
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

    fn graph() -> FinalEvidenceGraphV1 {
        let subject = selected_subject();
        let nodes = vec![
            node("package-archive", FinalEvidenceNodeClassV1::PackageArchive),
            node(
                "installed-journey",
                FinalEvidenceNodeClassV1::InstalledJourney,
            ),
            node(
                "support-selection",
                FinalEvidenceNodeClassV1::SupportSelection,
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
                    "support-selection",
                    "installed-journey",
                    FinalEvidenceEdgeKindV1::Projects,
                ),
            ],
            limitations: Vec::new(),
            claim_boundary: "Exact final-release evidence fixture.".to_string(),
        }
    }

    fn inputs() -> FinalReadinessDecisionInputsV1 {
        FinalReadinessDecisionInputsV1 {
            graph_owner: "owner:release-campaign".to_string(),
            root_decisions: vec![FinalReadinessRootDecisionV1 {
                decision_id: "support:windows-tier".to_string(),
                owner: "owner:support".to_string(),
                state: FinalReadinessDecisionStateV1::Decided,
                required: true,
            }],
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

    fn set_node_result(
        graph: &mut FinalEvidenceGraphV1,
        evidence_id: &str,
        result: FinalEvidenceNodeResultV1,
        currentness: FinalEvidenceCurrentnessV1,
    ) -> Result<(), String> {
        let node = graph
            .nodes
            .iter_mut()
            .find(|node| node.evidence_id == evidence_id)
            .ok_or_else(|| format!("missing fixture node {evidence_id}"))?;
        node.result = result;
        node.currentness = currentness;
        Ok(())
    }

    #[test]
    fn complete_inputs_ready_for_freeze_with_only_named_rows() -> Result<(), String> {
        let readiness = aggregate_final_readiness(&graph(), &inputs());
        if readiness.verdict != FinalReadinessVerdictV1::ReadyForFreeze {
            return Err(format!("expected ready, got {:?}", readiness.verdict));
        }
        for row in &readiness.rows {
            if row.owner.trim().is_empty() || row.next_action.trim().is_empty() {
                return Err(format!("unnamed row: {row:?}"));
            }
        }
        if readiness.selected_upload_rows != 10 || readiness.selected_shared_rows != 1 {
            return Err("denominator was not retained".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_required_node_blocks_ready_for_freeze() -> Result<(), String> {
        let mut graph = graph();
        graph.required_node_ids.push("package-docs".to_string());
        let readiness = aggregate_final_readiness(&graph, &inputs());
        if readiness.verdict != FinalReadinessVerdictV1::Incomplete {
            return Err(format!("expected incomplete, got {:?}", readiness.verdict));
        }
        let named = readiness.rows.iter().any(|row| {
            row.kind == FinalReadinessRowKindV1::MissingEvidence && !row.owner.is_empty()
        });
        if !named {
            return Err("missing-evidence row lacked an exact owner".to_string());
        }
        Ok(())
    }

    #[test]
    fn not_proven_blocks_unless_support_permits_an_explicit_narrowing() -> Result<(), String> {
        let mut graph = graph();
        set_node_result(
            &mut graph,
            "installed-journey",
            FinalEvidenceNodeResultV1::NotProven,
            FinalEvidenceCurrentnessV1::Current,
        )?;
        let blocked = aggregate_final_readiness(&graph, &inputs());
        if blocked.verdict != FinalReadinessVerdictV1::NotProven {
            return Err(format!("expected not_proven, got {:?}", blocked.verdict));
        }

        let mut narrowing_inputs = inputs();
        narrowing_inputs.permitted_claim_narrowings = vec![FinalReadinessClaimNarrowingV1 {
            evidence_id: "installed-journey".to_string(),
            permitted_by_decision: "support:journey-tier".to_string(),
            owner: "owner:support".to_string(),
        }];
        let narrowed = aggregate_final_readiness(&graph, &narrowing_inputs);
        if narrowed.verdict != FinalReadinessVerdictV1::ReadyForFreeze {
            return Err(format!(
                "explicitly permitted narrowing should stop blocking, got {:?}",
                narrowed.verdict
            ));
        }
        let narrowed_row = narrowed
            .rows
            .iter()
            .find(|row| row.kind == FinalReadinessRowKindV1::ClaimNarrowed)
            .ok_or_else(|| "claim-narrowed row is missing".to_string())?;
        if narrowed_row.owner != "owner:support"
            || !narrowed_row.next_action.contains("out of proof narration")
        {
            return Err("narrowing row lost its owner or claim boundary".to_string());
        }
        Ok(())
    }

    #[test]
    fn stale_node_forces_stale_and_names_the_smallest_owner() -> Result<(), String> {
        let mut graph = graph();
        set_node_result(
            &mut graph,
            "package-archive",
            FinalEvidenceNodeResultV1::Complete,
            FinalEvidenceCurrentnessV1::Expired,
        )?;
        let readiness = aggregate_final_readiness(&graph, &inputs());
        if readiness.verdict != FinalReadinessVerdictV1::Stale {
            return Err(format!("expected stale, got {:?}", readiness.verdict));
        }
        if !readiness
            .rows
            .iter()
            .any(|row| row.owner == "owner:package-archive")
        {
            return Err("stale row did not name the smallest owner".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_root_decision_stays_distinct_from_implementation_failure() -> Result<(), String> {
        let mut decision_inputs = inputs();
        let first = decision_inputs
            .root_decisions
            .first_mut()
            .ok_or_else(|| "fixture lost its root decision".to_string())?;
        first.state = FinalReadinessDecisionStateV1::Missing;
        let readiness = aggregate_final_readiness(&graph(), &decision_inputs);
        if readiness.verdict != FinalReadinessVerdictV1::NeedsDecision {
            return Err(format!(
                "expected needs_decision, got {:?}",
                readiness.verdict
            ));
        }
        if !readiness.rows.iter().any(|row| {
            row.kind == FinalReadinessRowKindV1::DecisionRequired && row.owner == "owner:support"
        }) {
            return Err("decision row lacked the exact decision owner".to_string());
        }
        Ok(())
    }

    #[test]
    fn invalid_supported_limitation_is_unsupported_and_named() -> Result<(), String> {
        let mut decision_inputs = inputs();
        decision_inputs.supported_limitations = vec![FinalReadinessSupportedLimitationV1 {
            limitation_id: "limitation:windows-symlink".to_string(),
            user_facing_projection: None,
            owner: None,
        }];
        let readiness = aggregate_final_readiness(&graph(), &decision_inputs);
        if readiness.verdict != FinalReadinessVerdictV1::Unsupported {
            return Err(format!("expected unsupported, got {:?}", readiness.verdict));
        }
        if !readiness.rows.iter().any(|row| {
            row.kind == FinalReadinessRowKindV1::Unsupported
                && row.owner == "owner:release-campaign"
        }) {
            return Err("unsupported row fell back to the graph owner".to_string());
        }
        Ok(())
    }

    #[test]
    fn custody_expiring_before_window_blocks_ready_for_freeze() -> Result<(), String> {
        let mut decision_inputs = inputs();
        decision_inputs.custody.expires_before_authorization_window = true;
        let readiness = aggregate_final_readiness(&graph(), &decision_inputs);
        if readiness.verdict != FinalReadinessVerdictV1::Stale {
            return Err(format!("expected stale, got {:?}", readiness.verdict));
        }
        if !readiness
            .rows
            .iter()
            .any(|row| row.kind == FinalReadinessRowKindV1::CustodyExpiring)
        {
            return Err("custody-expiring row is missing".to_string());
        }
        Ok(())
    }

    #[test]
    fn qualification_rerun_rejects_the_old_graph() -> Result<(), String> {
        let mut decision_inputs = inputs();
        decision_inputs.post_merge.qualification =
            FinalReadinessQualificationPostureV1::RequiresRerun;
        let readiness = aggregate_final_readiness(&graph(), &decision_inputs);
        if readiness.verdict != FinalReadinessVerdictV1::Stale {
            return Err(format!("expected stale, got {:?}", readiness.verdict));
        }
        if readiness.post_merge_qualification != FinalReadinessQualificationPostureV1::RequiresRerun
        {
            return Err("qualification posture was not retained".to_string());
        }
        Ok(())
    }

    #[test]
    fn aggregate_is_deterministic_under_input_ordering() -> Result<(), String> {
        let base = graph();
        let mut reordered = graph();
        reordered.nodes.reverse();
        let first = aggregate_final_readiness(&base, &inputs());
        let second = aggregate_final_readiness(&reordered, &inputs());
        if first != second {
            return Err("aggregate depended on input ordering".to_string());
        }
        Ok(())
    }

    #[test]
    fn markdown_and_json_projections_agree_with_the_typed_aggregate() -> Result<(), String> {
        let readiness = aggregate_final_readiness(&graph(), &inputs());
        let markdown = render_final_readiness_markdown(&readiness);
        if !markdown.contains("ready_for_freeze") || !markdown.contains("Claim boundary") {
            return Err("markdown projection lost the verdict or claim boundary".to_string());
        }
        let json = render_final_readiness_json(&readiness).map_err(|error| error.to_string())?;
        if !json.contains(FINAL_READINESS_SCHEMA_ID) {
            return Err("json projection lost the schema id".to_string());
        }
        Ok(())
    }
}
