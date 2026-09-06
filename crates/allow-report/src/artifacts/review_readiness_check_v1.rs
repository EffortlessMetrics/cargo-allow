//! Review-readiness check projection (#3844).
//!
//! Projects the typed exact-head review state (#3843) into one stable
//! machine-visible GitHub check context, `review-readiness`, for an
//! exact current PR pair. The projection consumes only structured
//! inputs — a retained `ReviewDispositionV1`, a live source snapshot,
//! the PR Draft/Ready posture, and the triggering event class — and
//! never parses comments, review summaries, labels, approval counts,
//! or CI aggregate state as review semantics. CI remains a separate
//! required input and can never clear a blocked review.
//!
//! Conclusion law:
//! - a present, current, ReviewClean disposition → Success;
//! - ReviewBlocked, Stale, Partial, or Unsupported → Failure (a Ready
//!   PR must become/remain Draft);
//! - malformed disposition or snapshot (InstrumentFailure) → Failure;
//! - a missing disposition → Neutral with required posture Draft: an
//!   unreviewed PR has no proven readiness, and the live required-
//!   control posture for the missing case is owned by #2284.
//!
//! Freshness law: every event capable of moving head, base, merge
//! base, reviewed diff, or readiness posture requires recompute; a
//! retained green observation whose binding (repository, PR, base/head
//! refs and SHAs, merge base, diff digest, disposition identity) does
//! not equal the live pair is a stale green and is invalidated. The
//! projection binds its own result to the exact live pair.
//!
//! Claim boundary: a read-only projection over typed review state. It
//! does not perform review, does not merge, tag, publish, change
//! rulesets, mint authorization, or mutate release/external state, and
//! cannot make itself a required check; #2283 names the check and
//! #2284 applies and reads back the live control.

use serde::{Deserialize, Serialize};

use super::review_disposition_v1::{
    ReviewCurrentnessV1, ReviewDispositionParseFailureV1, ReviewDispositionV1, ReviewLiveSourceV1,
    ReviewReadinessStateV1, ReviewTransitionRequestV1, evaluate_review_disposition,
    parse_review_live_source_bytes, review_semantic_identity,
};

pub const REVIEW_READINESS_CHECK_SCHEMA_ID: &str = "cargo-allow.review-readiness-check.v1";
pub const REVIEW_READINESS_CHECK_SCHEMA_VERSION: u32 = 1;

/// The one stable GitHub check context this projection publishes.
pub const REVIEW_READINESS_CHECK_CONTEXT: &str = "review-readiness";

const CLAIM_BOUNDARY: &str = "A read-only projection of typed review state into one stable review-readiness check for an exact PR pair. It makes blocking and stale review machine-visible with a Draft/Ready control posture; it does not perform review, does not parse review prose, does not merge, tag, publish, change rulesets, or mutate release/external state, and cannot make itself a required check.";

/// The check conclusions. Success requires a current clean structured
/// review; Neutral is reserved for the missing-disposition case; every
/// blocked, stale, partial, unsupported, or malformed state fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReadinessConclusionV1 {
    Success,
    Neutral,
    Failure,
}

impl ReviewReadinessConclusionV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Neutral => "neutral",
            Self::Failure => "failure",
        }
    }
}

/// PR Draft/Ready posture as observed for the exact pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReadinessDraftStateV1 {
    Draft,
    Ready,
}

impl ReviewReadinessDraftStateV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
        }
    }
}

impl From<ReviewReadinessDraftStateV1> for ReviewReadinessStateV1 {
    fn from(state: ReviewReadinessDraftStateV1) -> Self {
        match state {
            ReviewReadinessDraftStateV1::Draft => Self::Draft,
            ReviewReadinessDraftStateV1::Ready => Self::Ready,
        }
    }
}

/// Trigger/event classes the freshness law recognizes. Every class
/// recomputes the projection; the event is retained as evidence that
/// the recompute happened on a readiness-relevant change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReadinessEventV1 {
    Opened,
    Reopened,
    Synchronize,
    ForcePush,
    ReadyForReview,
    ConvertedToDraft,
    BaseMoved,
    MergeBaseMoved,
    DispositionUpdated,
    WorkflowConfigMoved,
}

impl ReviewReadinessEventV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Opened => "opened",
            Self::Reopened => "reopened",
            Self::Synchronize => "synchronize",
            Self::ForcePush => "force_push",
            Self::ReadyForReview => "ready_for_review",
            Self::ConvertedToDraft => "converted_to_draft",
            Self::BaseMoved => "base_moved",
            Self::MergeBaseMoved => "merge_base_moved",
            Self::DispositionUpdated => "disposition_updated",
            Self::WorkflowConfigMoved => "workflow_config_moved",
        }
    }

    /// Events that move the reviewed source pair. A retained green
    /// observation from before any of these is a stale green.
    #[must_use]
    pub const fn moves_source_pair(self) -> bool {
        matches!(
            self,
            Self::Synchronize
                | Self::ForcePush
                | Self::BaseMoved
                | Self::MergeBaseMoved
                | Self::DispositionUpdated
                | Self::WorkflowConfigMoved
        )
    }
}

/// The structured disposition input for one projection. The adapter
/// may deliver a retained record, an explicit missing state, or a
/// parse failure; free prose is not an input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReadinessDispositionInputV1 {
    Present(Box<ReviewDispositionV1>),
    Missing,
    Malformed { reason: String },
}

/// A retained check observation bound to the exact pair it was
/// computed for. Used to detect stale greens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReadinessObservationV1 {
    pub conclusion: ReviewReadinessConclusionV1,
    pub binding: ReviewReadinessBindingV1,
}

/// The exact pair + disposition identity a projection or observation
/// is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReadinessBindingV1 {
    pub repository: String,
    pub pr_number: u64,
    pub base_ref: String,
    pub base_sha: String,
    pub head_ref: String,
    pub head_sha: String,
    pub merge_base: String,
    pub diff_digest: String,
    /// Semantic identity of the disposition consumed, empty when the
    /// input was missing.
    pub disposition_identity: String,
}

impl ReviewReadinessBindingV1 {
    /// Equality of the source-pair dimensions (repository, PR, refs,
    /// SHAs, merge base, diff digest). A disposition-identity change
    /// over the same pair supersedes a retained green; it does not
    /// stale the pair.
    #[must_use]
    pub fn same_source_pair(&self, other: &Self) -> bool {
        self.repository == other.repository
            && self.pr_number == other.pr_number
            && self.base_ref == other.base_ref
            && self.base_sha == other.base_sha
            && self.head_ref == other.head_ref
            && self.head_sha == other.head_sha
            && self.merge_base == other.merge_base
            && self.diff_digest == other.diff_digest
    }
}

/// Projection input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReadinessProjectionInputV1 {
    pub disposition: ReviewReadinessDispositionInputV1,
    pub live: ReviewLiveSourceV1,
    pub draft_state: ReviewReadinessDraftStateV1,
    pub event: ReviewReadinessEventV1,
    /// The prior retained observation, when one exists.
    pub prior_observation: Option<ReviewReadinessObservationV1>,
    /// Paths changed between the disposition's bound head and the live
    /// head, computed by the adapter from git. Empty when the heads are
    /// equal or no disposition is present.
    #[serde(default)]
    pub head_delta_paths: Vec<String>,
}

/// The retained-review-ledger escape: a disposition committed inside
/// the PR it covers necessarily moves the head it binds. The law
/// admits that movement only when the entire head delta is
/// review-disposition records under the declared ledger directory;
/// anything else is ordinary staleness.
pub const REVIEW_DISPOSITION_LEDGER_DIR: &str = ".allow/review-dispositions/";

#[must_use]
pub fn is_review_ledger_bootstrap(head_delta_paths: &[String]) -> bool {
    !head_delta_paths.is_empty()
        && head_delta_paths
            .iter()
            .all(|path| path.starts_with(REVIEW_DISPOSITION_LEDGER_DIR))
}

/// The projected check result, bound to the exact live pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReadinessProjectionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// The one stable check context. Owned so consumers can parse the
    /// JSON view back with serde.
    pub check_context: String,
    pub repository: String,
    pub pr_number: u64,
    pub event: ReviewReadinessEventV1,
    pub conclusion: ReviewReadinessConclusionV1,
    pub conclusion_reasons: Vec<String>,
    /// The posture the control path requires after this projection.
    pub required_posture: ReviewReadinessStateV1,
    /// True when a retained green observation was bound to a different
    /// pair than the live one: the old green is stale and must not be
    /// reused by any consumer.
    pub stale_green_invalidated: bool,
    /// True when the disposition's bound head differs from the live
    /// head and the entire delta is proven review-disposition records
    /// (the retained-review-ledger bootstrap).
    pub head_ledger_bootstrap: bool,
    pub binding: ReviewReadinessBindingV1,
    /// Owned so consumers can parse the JSON view back with serde.
    pub claim_boundary: String,
}

/// Parse a live source snapshot for the projection.
pub fn parse_review_readiness_live_bytes(
    bytes: &[u8],
) -> Result<ReviewLiveSourceV1, ReviewDispositionParseFailureV1> {
    parse_review_live_source_bytes(bytes)
}

/// Project the structured review state onto the stable check context.
/// Pure and timestamp-free.
#[must_use]
pub fn evaluate_review_readiness_projection(
    input: &ReviewReadinessProjectionInputV1,
) -> ReviewReadinessProjectionV1 {
    let mut conclusion_reasons = Vec::new();
    let disposition_identity = match &input.disposition {
        ReviewReadinessDispositionInputV1::Present(disposition) => {
            review_semantic_identity(disposition)
        }
        _ => String::new(),
    };

    let (conclusion, currentness_reasons) = match &input.disposition {
        ReviewReadinessDispositionInputV1::Malformed { reason } => (
            ReviewReadinessConclusionV1::Failure,
            vec![format!("malformed disposition: {reason}")],
        ),
        ReviewReadinessDispositionInputV1::Missing => {
            // An unreviewed PR has no proven readiness. The check stays
            // neutral (never clean); the control posture is Draft until
            // a current clean disposition exists.
            conclusion_reasons.push(
                "no retained review disposition for this exact pair; readiness is not proven"
                    .to_string(),
            );
            (ReviewReadinessConclusionV1::Neutral, Vec::new())
        }
        ReviewReadinessDispositionInputV1::Present(disposition) => {
            // Reuse the #3843 evaluator: the readiness question is the
            // Draft/Ready -> Ready transition with no embedded checks,
            // because CI stays a separate required input.
            let request = ReviewTransitionRequestV1 {
                current_state: input.draft_state.into(),
                target_state: ReviewReadinessStateV1::Ready,
                required_checks: Vec::new(),
            };
            // The retained-review-ledger bootstrap: when the only head
            // delta since the reviewed head is disposition records, the
            // record's own commit does not stale the review. Base and
            // merge-base movement still stale normally.
            let bootstrap = disposition.head_sha != input.live.head_sha
                && is_review_ledger_bootstrap(&input.head_delta_paths);
            let effective_live;
            let evaluation_live = if bootstrap {
                conclusion_reasons.push(
                    "review-ledger bootstrap: the head delta since the reviewed pair is review-disposition records only".to_string(),
                );
                let mut adjusted = input.live.clone();
                adjusted.head_sha = disposition.head_sha.clone();
                adjusted.diff_digest = disposition.reviewed_diff_digest.clone();
                effective_live = adjusted;
                &effective_live
            } else {
                &input.live
            };
            let outcome = evaluate_review_disposition(disposition, evaluation_live, &request);
            conclusion_reasons.extend(outcome.transition.reasons.iter().cloned());
            conclusion_reasons.extend(outcome.currentness_reasons.iter().cloned());
            let conclusion = match outcome.currentness {
                ReviewCurrentnessV1::ReviewClean => ReviewReadinessConclusionV1::Success,
                ReviewCurrentnessV1::ReviewBlocked
                | ReviewCurrentnessV1::Stale
                | ReviewCurrentnessV1::Partial
                | ReviewCurrentnessV1::Unsupported
                | ReviewCurrentnessV1::InstrumentFailure => ReviewReadinessConclusionV1::Failure,
            };
            (conclusion, Vec::new())
        }
    };
    conclusion_reasons.extend(currentness_reasons);

    let current_binding = binding(input, &disposition_identity);
    let stale_green_invalidated = input.prior_observation.as_ref().is_some_and(|prior| {
        prior.conclusion == ReviewReadinessConclusionV1::Success
            && (input.event.moves_source_pair()
                || !prior.binding.same_source_pair(&current_binding))
    });
    if stale_green_invalidated {
        conclusion_reasons.push(
            "a retained green observation was bound to a different pair or predates a source-pair movement; the old green is stale".to_string(),
        );
    }

    let required_posture = if conclusion == ReviewReadinessConclusionV1::Success {
        ReviewReadinessStateV1::Ready
    } else {
        ReviewReadinessStateV1::Draft
    };

    ReviewReadinessProjectionV1 {
        schema_id: REVIEW_READINESS_CHECK_SCHEMA_ID.to_string(),
        schema_version: REVIEW_READINESS_CHECK_SCHEMA_VERSION,
        check_context: REVIEW_READINESS_CHECK_CONTEXT.to_string(),
        repository: input.live.repository.clone(),
        pr_number: input.live.pr_number,
        event: input.event,
        conclusion,
        conclusion_reasons,
        required_posture,
        stale_green_invalidated,
        head_ledger_bootstrap: matches!(
            &input.disposition,
            ReviewReadinessDispositionInputV1::Present(disposition) if disposition.head_sha != input.live.head_sha
        ) && is_review_ledger_bootstrap(&input.head_delta_paths),
        binding: binding(input, &disposition_identity),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

fn binding(
    input: &ReviewReadinessProjectionInputV1,
    disposition_identity: &str,
) -> ReviewReadinessBindingV1 {
    ReviewReadinessBindingV1 {
        repository: input.live.repository.clone(),
        pr_number: input.live.pr_number,
        base_ref: input.live.base_ref.clone(),
        base_sha: input.live.base_sha.clone(),
        head_ref: input.live.head_ref.clone(),
        head_sha: input.live.head_sha.clone(),
        merge_base: input.live.merge_base.clone(),
        diff_digest: input.live.diff_digest.clone(),
        disposition_identity: disposition_identity.to_string(),
    }
}

/// Human view of the projection.
#[must_use]
pub fn render_review_readiness_human(projection: &ReviewReadinessProjectionV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "{}: repository={} pr={} event={}",
        projection.check_context,
        projection.repository,
        projection.pr_number,
        projection.event.label()
    ));
    lines.push(format!("  conclusion: {}", projection.conclusion.label()));
    lines.push(format!(
        "  required posture: {}",
        projection.required_posture.label()
    ));
    if projection.stale_green_invalidated {
        lines.push("  stale green: invalidated".to_string());
    }
    if projection.head_ledger_bootstrap {
        lines.push("  review-ledger bootstrap: applied".to_string());
    }
    for reason in &projection.conclusion_reasons {
        lines.push(format!("  reason: {reason}"));
    }
    lines.push(format!(
        "  binding: {}#{}/{}:{}",
        projection.binding.repository,
        projection.binding.pr_number,
        projection.binding.head_ref,
        projection.binding.head_sha
    ));
    lines.push(format!("  claim boundary: {}", projection.claim_boundary));
    lines.join("\n")
}

/// JSON view of the projection.
pub fn render_review_readiness_json(
    projection: &ReviewReadinessProjectionV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(projection)
}
