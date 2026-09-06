//! Exact-head review disposition and readiness transitions (#3843).
//!
//! One typed object binds the actual reviewed source pair (repository,
//! PR number, base/head refs and SHAs, merge base, full-diff digest),
//! the review protocol generation, blocking and advisory findings with
//! stable ids, and the Draft/Ready transition consequence. Currentness
//! law: any load-bearing input movement — head, base, merge base,
//! reviewed diff, protocol generation, reviewer scope, or repository/PR
//! identity — stales the disposition deterministically; reviewer
//! identity or equal prose cannot preserve currentness. Readiness law:
//! a current ReviewClean disposition may permit Draft -> Ready only
//! after the separately declared CI/control conditions are terminal-
//! passed; ReviewBlocked permits (and is the control action for)
//! Ready -> Draft; Stale, Partial, Unsupported, and InstrumentFailure
//! never restore Ready.
//!
//! Finding law: blocking findings require a stable id, an owned seam,
//! an exact source basis, and a repair route; a same-maintainer actor
//! class is retained as process evidence and is never presented as
//! independent-human approval; independent review that was sought but
//! is unavailable or quota-limited is `NotProven` and downgrades a
//! claimed clean verdict; generic comments, approval counts, CI green,
//! and PR-ready labels cannot synthesize ReviewClean.
//!
//! Claim boundary: a read-only typed authority for cargo-allow's
//! solo-maintainer workflow. It does not conduct review, does not
//! publish a GitHub check, does not mutate PR Draft/Ready state, live
//! settings, tags, packages, or release state, and does not require a
//! second human. #3844 owns check/control projection and #2284 owns
//! live effective readback; this contract gives them the semantics to
//! consume without reimplementing review law.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::campaign_issue_closeout_v1::{CampaignCheckOutcomeV1, CampaignEvidenceClassV1};

pub const REVIEW_DISPOSITION_SCHEMA_ID: &str = "cargo-allow.review-disposition.v1";
pub const REVIEW_DISPOSITION_SCHEMA_VERSION: u32 = 1;

/// Structural bounds enforced by the bounded source adapter and by the
/// evaluator itself, so a hostile or runaway record fails closed.
pub const REVIEW_DISPOSITION_MAX_FINDINGS: usize = 512;
pub const REVIEW_DISPOSITION_MAX_THREADS: usize = 512;
pub const REVIEW_DISPOSITION_MAX_TEXT_LEN: usize = 4096;

const CLAIM_BOUNDARY: &str = "A read-only typed authority stating whether the reviewed source pair is clean, blocked, stale, or unavailable and what Draft/Ready transition it permits. It does not conduct review, does not publish a GitHub check, does not mutate PR state, live settings, tags, packages, or release state, and does not require a second human.";

/// Closed currentness vocabulary for one review disposition. The
/// evaluator re-derives it from typed structure; the claimed value is
/// retained metadata and never the authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewCurrentnessV1 {
    ReviewClean,
    ReviewBlocked,
    Stale,
    Partial,
    Unsupported,
    InstrumentFailure,
}

impl ReviewCurrentnessV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReviewClean => "review_clean",
            Self::ReviewBlocked => "review_blocked",
            Self::Stale => "stale",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
            Self::InstrumentFailure => "instrument_failure",
        }
    }

    /// Verdicts that are always derived, never claimable; a disposition
    /// claiming one is incoherent process evidence.
    #[must_use]
    pub const fn is_claimable(self) -> bool {
        !matches!(self, Self::Stale | Self::InstrumentFailure)
    }
}

impl fmt::Display for ReviewCurrentnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.label())
    }
}

/// Severity of one retained finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFindingSeverityV1 {
    Blocking,
    Advisory,
}

impl ReviewFindingSeverityV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::Advisory => "advisory",
        }
    }
}

/// One retained review finding. A blocking finding must carry a stable
/// id, an owned seam, an exact source basis, and a repair route; the
/// evaluator fails a disposition that claims a blocking finding without
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFindingV1 {
    pub id: String,
    pub severity: ReviewFindingSeverityV1,
    /// The owned seam the finding names (module, surface, or owner).
    pub owned_seam: String,
    /// Exact source basis of the finding.
    pub source_path: String,
    pub source_line: Option<u32>,
    /// The typed repair route (who/what closes the finding).
    pub repair_route: String,
    pub claim_boundary: String,
}

/// Identity class of the review actor. Same-maintainer review is valid
/// process evidence for the solo-maintainer workflow; it is never
/// presented as independent-human approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewActorClassV1 {
    SameMaintainer,
    IndependentHuman,
    IndependentBot,
    IndependentModel,
    /// The review was sought but not performed (quota-limited or
    /// unavailable). An unavailable review cannot be clean.
    Unavailable,
}

impl ReviewActorClassV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SameMaintainer => "same_maintainer",
            Self::IndependentHuman => "independent_human",
            Self::IndependentBot => "independent_bot",
            Self::IndependentModel => "independent_model",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Posture of separately retained independent review. Sought-but-
/// unavailable review is `NotProven`, never clean; review that was not
/// sought is `NotRetained` and leaves the solo-maintainer process
/// review as the retained evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndependentReviewPostureV1 {
    NotRetained,
    NotProven { reason: String },
    Proven { reference: String },
}

/// Required-CI ownership retained by the disposition. The disposition
/// itself never reads CI; it either names the separate owner that
/// decides readiness from CI or retains an observation reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequiredCiV1 {
    /// Explicit separate owner (e.g. the #3844 check projection).
    pub owner: String,
    /// Retained observation reference, when one exists.
    pub observation_ref: String,
}

/// The retained review disposition for one exact source pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDispositionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    pub pr_number: u64,
    pub base_ref: String,
    pub base_sha: String,
    pub head_ref: String,
    pub head_sha: String,
    /// Effective merge base (the effective comparison pair).
    pub merge_base: String,
    /// Full-diff digest over the reviewed changed set.
    pub reviewed_diff_digest: String,
    /// Review protocol/skill generation the review was executed under.
    pub review_protocol: String,
    pub actor_class: ReviewActorClassV1,
    pub reviewer_identity: String,
    pub independent_review: IndependentReviewPostureV1,
    /// Claimed verdict; the evaluator re-derives and structure wins.
    pub claimed_verdict: ReviewCurrentnessV1,
    pub findings: Vec<ReviewFindingV1>,
    /// Threads/dispositions inspected (process metadata, not semantic
    /// identity).
    pub threads_inspected: Vec<String>,
    pub required_ci: ReviewRequiredCiV1,
    /// Evidence class from #3810, kept explicit.
    pub evidence_class: CampaignEvidenceClassV1,
    /// Selected issue/claim boundary that defines reviewer scope.
    pub scope_claim_boundary: String,
    /// Envelope metadata only; excluded from semantic identity.
    pub reviewed_at_utc: String,
}

/// Live source snapshot the currentness check consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewLiveSourceV1 {
    pub repository: String,
    pub pr_number: u64,
    pub base_ref: String,
    pub base_sha: String,
    pub head_ref: String,
    pub head_sha: String,
    pub merge_base: String,
    pub diff_digest: String,
    pub review_protocol: String,
    /// The reviewer scope the consumer needs for this decision.
    pub scope_claim_boundary: String,
}

/// Draft/Ready readiness state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReadinessStateV1 {
    Ready,
    Draft,
}

impl ReviewReadinessStateV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Draft => "draft",
        }
    }
}

/// One declared required check with its observed outcome. Terminal and
/// passed is the only readiness-permitting outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewCheckObservationV1 {
    pub name: String,
    pub outcome: CampaignCheckOutcomeV1,
}

/// The readiness question the consumer asks: from which state, to which
/// state, under which separately declared required checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewTransitionRequestV1 {
    pub current_state: ReviewReadinessStateV1,
    pub target_state: ReviewReadinessStateV1,
    pub required_checks: Vec<ReviewCheckObservationV1>,
}

/// The evaluated transition. Inert data: no GitHub or live state is
/// touched by producing or observing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReadinessTransitionV1 {
    pub from_state: ReviewReadinessStateV1,
    pub to_state: ReviewReadinessStateV1,
    pub permitted: bool,
    pub reasons: Vec<String>,
}

/// The evaluated disposition outcome. The human and JSON views both
/// derive from this one ordered semantic result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDispositionOutcomeV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    pub pr_number: u64,
    /// Actor class carried through so consumers never present a
    /// same-maintainer process review as independent-human approval.
    pub actor_class: ReviewActorClassV1,
    pub claimed_verdict: ReviewCurrentnessV1,
    pub currentness: ReviewCurrentnessV1,
    pub currentness_reasons: Vec<String>,
    pub stale_dimensions: Vec<String>,
    pub blocking_finding_ids: Vec<String>,
    pub semantic_identity: String,
    pub transition: ReviewReadinessTransitionV1,
    /// Owned so consumers can parse the JSON view back with serde.
    pub claim_boundary: String,
}

#[must_use]
fn fnv1a64(parts: &[String]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // Field separator so "ab"|"c" and "a"|"bc" differ.
        hash ^= 0x1f;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Semantic identity of one disposition: an ordered digest over every
/// source dimension that makes a prior review stale (repository, PR,
/// base/head refs and SHAs, merge base, full-diff digest, protocol
/// generation, reviewer scope, and the ordered finding set with its
/// sources and severities). Volatile presentation fields — review
/// timestamp, reviewer display identity, inspected threads, CI
/// ownership metadata — are excluded, so they cannot change the
/// identity of what was reviewed.
#[must_use]
pub fn review_semantic_identity(disposition: &ReviewDispositionV1) -> String {
    let mut parts: Vec<String> = vec![
        disposition.schema_id.clone(),
        disposition.repository.clone(),
        disposition.pr_number.to_string(),
        disposition.base_ref.clone(),
        disposition.base_sha.clone(),
        disposition.head_ref.clone(),
        disposition.head_sha.clone(),
        disposition.merge_base.clone(),
        disposition.reviewed_diff_digest.clone(),
        disposition.review_protocol.clone(),
        disposition.scope_claim_boundary.clone(),
    ];
    for finding in &disposition.findings {
        parts.push(finding.id.clone());
        parts.push(finding.severity.label().to_string());
        parts.push(finding.owned_seam.clone());
        parts.push(finding.source_path.clone());
        parts.push(
            finding
                .source_line
                .map_or_else(String::new, |line| line.to_string()),
        );
        parts.push(finding.repair_route.clone());
    }
    fnv1a64(&parts)
}

fn text_too_long(field: &str, value: &str) -> Option<String> {
    if value.chars().count() > REVIEW_DISPOSITION_MAX_TEXT_LEN {
        Some(format!(
            "field '{field}' exceeds the {REVIEW_DISPOSITION_MAX_TEXT_LEN}-character bound"
        ))
    } else {
        None
    }
}

/// Structural validation of one disposition. Malformed or incoherent
/// records are instrument failures, never silently evaluated.
fn disposition_instrument_failures(disposition: &ReviewDispositionV1) -> Vec<String> {
    let mut failures = Vec::new();
    if disposition.schema_id != REVIEW_DISPOSITION_SCHEMA_ID {
        failures.push(format!(
            "schema_id is not {REVIEW_DISPOSITION_SCHEMA_ID}: {}",
            disposition.schema_id
        ));
    }
    if disposition.schema_version != REVIEW_DISPOSITION_SCHEMA_VERSION {
        failures.push(format!(
            "unsupported schema_version: {}",
            disposition.schema_version
        ));
    }
    if disposition.repository.trim().is_empty() {
        failures.push("repository is empty".to_string());
    }
    if disposition.pr_number == 0 {
        failures.push("pr_number is zero".to_string());
    }
    for (field, value) in [
        ("base_ref", &disposition.base_ref),
        ("base_sha", &disposition.base_sha),
        ("head_ref", &disposition.head_ref),
        ("head_sha", &disposition.head_sha),
        ("merge_base", &disposition.merge_base),
        ("reviewed_diff_digest", &disposition.reviewed_diff_digest),
        ("review_protocol", &disposition.review_protocol),
        ("scope_claim_boundary", &disposition.scope_claim_boundary),
    ] {
        if value.trim().is_empty() {
            failures.push(format!("{field} is empty"));
        }
    }
    if disposition.required_ci.owner.trim().is_empty()
        && disposition.required_ci.observation_ref.trim().is_empty()
    {
        failures.push(
            "required_ci retains neither an observation reference nor an explicit separate owner"
                .to_string(),
        );
    }
    if !disposition.claimed_verdict.is_claimable() {
        failures.push(format!(
            "claimed_verdict is a derived-only verdict: {}",
            disposition.claimed_verdict.label()
        ));
    }
    if disposition.findings.len() > REVIEW_DISPOSITION_MAX_FINDINGS {
        failures.push(format!(
            "findings exceed the {REVIEW_DISPOSITION_MAX_FINDINGS}-entry bound"
        ));
    }
    if disposition.threads_inspected.len() > REVIEW_DISPOSITION_MAX_THREADS {
        failures.push(format!(
            "threads_inspected exceeds the {REVIEW_DISPOSITION_MAX_THREADS}-entry bound"
        ));
    }
    let mut seen_ids: Vec<&str> = Vec::new();
    for finding in &disposition.findings {
        if matches!(finding.severity, ReviewFindingSeverityV1::Blocking) {
            for (field, value) in [
                ("id", &finding.id),
                ("owned_seam", &finding.owned_seam),
                ("source_path", &finding.source_path),
                ("repair_route", &finding.repair_route),
            ] {
                if value.trim().is_empty() {
                    failures.push(format!("blocking finding is missing its {field}"));
                }
            }
        }
        if !seen_ids.contains(&finding.id.as_str()) {
            seen_ids.push(&finding.id);
        } else {
            failures.push(format!("duplicate finding id: {}", finding.id));
        }
        for (field, value) in [
            ("findings.id", &finding.id),
            ("findings.owned_seam", &finding.owned_seam),
            ("findings.source_path", &finding.source_path),
            ("findings.repair_route", &finding.repair_route),
            ("findings.claim_boundary", &finding.claim_boundary),
        ] {
            if let Some(too_long) = text_too_long(field, value) {
                failures.push(too_long);
            }
        }
    }
    if matches!(disposition.actor_class, ReviewActorClassV1::Unavailable)
        && matches!(
            disposition.independent_review,
            IndependentReviewPostureV1::Proven { .. }
        )
    {
        failures.push(
            "actor class is unavailable but independent review claims a proven record".to_string(),
        );
    }
    if disposition.claimed_verdict == ReviewCurrentnessV1::ReviewBlocked
        && !disposition
            .findings
            .iter()
            .any(|finding| matches!(finding.severity, ReviewFindingSeverityV1::Blocking))
    {
        failures.push(
            "claimed_verdict is review_blocked but no blocking finding is retained".to_string(),
        );
    }
    failures
}

fn live_instrument_failures(live: &ReviewLiveSourceV1) -> Vec<String> {
    let mut failures = Vec::new();
    if live.repository.trim().is_empty() {
        failures.push("live repository is empty".to_string());
    }
    for (field, value) in [
        ("base_ref", &live.base_ref),
        ("base_sha", &live.base_sha),
        ("head_ref", &live.head_ref),
        ("head_sha", &live.head_sha),
        ("merge_base", &live.merge_base),
        ("diff_digest", &live.diff_digest),
        ("review_protocol", &live.review_protocol),
        ("scope_claim_boundary", &live.scope_claim_boundary),
    ] {
        if value.trim().is_empty() {
            failures.push(format!("live {field} is empty"));
        }
    }
    failures
}

fn request_instrument_failures(request: &ReviewTransitionRequestV1) -> Vec<String> {
    request
        .required_checks
        .iter()
        .filter(|check| check.name.trim().is_empty())
        .map(|_| "a required check carries an empty name".to_string())
        .collect()
}

/// Load-bearing input dimensions whose movement stales a disposition,
/// in fixed order.
fn stale_dimensions(disposition: &ReviewDispositionV1, live: &ReviewLiveSourceV1) -> Vec<String> {
    let mut stale = Vec::new();
    if disposition.repository != live.repository {
        stale.push("repository".to_string());
    }
    if disposition.pr_number != live.pr_number {
        stale.push("pr_number".to_string());
    }
    if disposition.base_sha != live.base_sha {
        stale.push("base_sha".to_string());
    }
    if disposition.head_sha != live.head_sha {
        stale.push("head_sha".to_string());
    }
    if disposition.merge_base != live.merge_base {
        stale.push("merge_base".to_string());
    }
    if disposition.reviewed_diff_digest != live.diff_digest {
        stale.push("reviewed_diff_digest".to_string());
    }
    if disposition.review_protocol != live.review_protocol {
        stale.push("review_protocol".to_string());
    }
    if disposition.scope_claim_boundary != live.scope_claim_boundary {
        stale.push("review_scope".to_string());
    }
    stale
}

/// Derive the currentness verdict from typed structure on a non-stale,
/// instrument-clean disposition. Structure wins over the claim.
fn derive_currentness(
    disposition: &ReviewDispositionV1,
    reasons: &mut Vec<String>,
) -> ReviewCurrentnessV1 {
    let has_blocking = disposition
        .findings
        .iter()
        .any(|finding| matches!(finding.severity, ReviewFindingSeverityV1::Blocking));
    if has_blocking {
        return ReviewCurrentnessV1::ReviewBlocked;
    }
    if disposition.claimed_verdict == ReviewCurrentnessV1::ReviewClean {
        if matches!(disposition.actor_class, ReviewActorClassV1::Unavailable) {
            reasons.push(
                "actor class is unavailable: an unavailable review is not proven clean".to_string(),
            );
            return ReviewCurrentnessV1::Partial;
        }
        if let IndependentReviewPostureV1::NotProven { reason } = &disposition.independent_review {
            reasons.push(format!(
                "independent review is not proven and cannot back a clean claim: {reason}"
            ));
            return ReviewCurrentnessV1::Partial;
        }
    }
    disposition.claimed_verdict
}

fn check_outcome_label(outcome: CampaignCheckOutcomeV1) -> &'static str {
    match outcome {
        CampaignCheckOutcomeV1::Passed => "terminal-passed",
        CampaignCheckOutcomeV1::Failed => "failed",
        CampaignCheckOutcomeV1::Skipped => "skipped",
        CampaignCheckOutcomeV1::Cancelled => "cancelled",
        CampaignCheckOutcomeV1::Nonterminal => "non-terminal",
        CampaignCheckOutcomeV1::Unknown => "unknown",
    }
}

/// Evaluate the readiness transition against the derived currentness.
/// The result is inert data; no GitHub state changes.
#[must_use]
pub fn evaluate_review_readiness_transition(
    currentness: ReviewCurrentnessV1,
    stale_dimensions: &[String],
    blocking_count: usize,
    request: &ReviewTransitionRequestV1,
) -> ReviewReadinessTransitionV1 {
    let from = request.current_state;
    let to = request.target_state;
    if from == to {
        return ReviewReadinessTransitionV1 {
            from_state: from,
            to_state: to,
            permitted: true,
            reasons: vec!["already in the requested state".to_string()],
        };
    }
    if to == ReviewReadinessStateV1::Draft {
        return ReviewReadinessTransitionV1 {
            from_state: from,
            to_state: to,
            permitted: true,
            reasons: vec![
                "demotion to draft is always permitted; it is the control action for a blocked, stale, or unprovable review".to_string(),
            ],
        };
    }
    let mut reasons = Vec::new();
    let permitted = match currentness {
        ReviewCurrentnessV1::ReviewClean => {
            let failing: Vec<&ReviewCheckObservationV1> = request
                .required_checks
                .iter()
                .filter(|check| check.outcome != CampaignCheckOutcomeV1::Passed)
                .collect();
            for check in &failing {
                reasons.push(format!(
                    "required check '{}' is {}",
                    check.name,
                    check_outcome_label(check.outcome)
                ));
            }
            failing.is_empty()
        }
        ReviewCurrentnessV1::ReviewBlocked => {
            reasons.push(format!(
                "review is blocked by {blocking_count} blocking finding(s); CI state never overrides a blocked review"
            ));
            false
        }
        ReviewCurrentnessV1::Stale => {
            reasons.push(format!(
                "review is stale: load-bearing dimensions moved ({})",
                stale_dimensions.join(", ")
            ));
            reasons.push(
                "a fresh disposition on the current pair is required before ready restoration"
                    .to_string(),
            );
            false
        }
        ReviewCurrentnessV1::Partial => {
            reasons.push("review is partial and cannot restore ready".to_string());
            false
        }
        ReviewCurrentnessV1::Unsupported => {
            reasons.push("review is unsupported and cannot restore ready".to_string());
            false
        }
        ReviewCurrentnessV1::InstrumentFailure => {
            reasons.push(
                "review disposition failed instrument validation and cannot restore ready"
                    .to_string(),
            );
            false
        }
    };
    if permitted {
        reasons.push("review is clean and current".to_string());
        if !request.required_checks.is_empty() {
            reasons.push("every required check is terminal-passed".to_string());
        }
    }
    ReviewReadinessTransitionV1 {
        from_state: from,
        to_state: to,
        permitted,
        reasons,
    }
}

/// Evaluate one disposition against the live source and the readiness
/// request. Pure and timestamp-free.
#[must_use]
pub fn evaluate_review_disposition(
    disposition: &ReviewDispositionV1,
    live: &ReviewLiveSourceV1,
    request: &ReviewTransitionRequestV1,
) -> ReviewDispositionOutcomeV1 {
    let mut instrument = disposition_instrument_failures(disposition);
    instrument.extend(live_instrument_failures(live));
    instrument.extend(request_instrument_failures(request));

    let (stale, currentness, currentness_reasons) = if !instrument.is_empty() {
        (
            Vec::new(),
            ReviewCurrentnessV1::InstrumentFailure,
            instrument,
        )
    } else {
        let stale = stale_dimensions(disposition, live);
        if stale.is_empty() {
            let mut reasons = Vec::new();
            let currentness = derive_currentness(disposition, &mut reasons);
            (stale, currentness, reasons)
        } else {
            (stale, ReviewCurrentnessV1::Stale, Vec::new())
        }
    };

    let blocking_finding_ids: Vec<String> = disposition
        .findings
        .iter()
        .filter(|finding| matches!(finding.severity, ReviewFindingSeverityV1::Blocking))
        .map(|finding| finding.id.clone())
        .collect();

    let transition = evaluate_review_readiness_transition(
        currentness,
        &stale,
        blocking_finding_ids.len(),
        request,
    );

    ReviewDispositionOutcomeV1 {
        schema_id: REVIEW_DISPOSITION_SCHEMA_ID.to_string(),
        schema_version: REVIEW_DISPOSITION_SCHEMA_VERSION,
        repository: disposition.repository.clone(),
        pr_number: disposition.pr_number,
        actor_class: disposition.actor_class,
        claimed_verdict: disposition.claimed_verdict,
        currentness,
        currentness_reasons,
        stale_dimensions: stale,
        blocking_finding_ids,
        semantic_identity: review_semantic_identity(disposition),
        transition,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

/// Parse failure of the bounded source adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDispositionParseFailureV1 {
    pub reason: String,
}

/// Bounded source adapter: parse and structurally validate a
/// disposition from bytes. Unknown fields, bound violations, and
/// incoherent records fail closed.
pub fn parse_review_disposition_bytes(
    bytes: &[u8],
) -> Result<ReviewDispositionV1, ReviewDispositionParseFailureV1> {
    let disposition: ReviewDispositionV1 =
        serde_json::from_slice(bytes).map_err(|error| ReviewDispositionParseFailureV1 {
            reason: format!("disposition parse: {error}"),
        })?;
    let failures = disposition_instrument_failures(&disposition);
    if failures.is_empty() {
        Ok(disposition)
    } else {
        Err(ReviewDispositionParseFailureV1 {
            reason: failures.join("; "),
        })
    }
}

/// Parse a live source snapshot. Unknown fields fail closed.
pub fn parse_review_live_source_bytes(
    bytes: &[u8],
) -> Result<ReviewLiveSourceV1, ReviewDispositionParseFailureV1> {
    serde_json::from_slice(bytes).map_err(|error| ReviewDispositionParseFailureV1 {
        reason: format!("live source parse: {error}"),
    })
}

/// Parse a transition request. Unknown fields fail closed.
pub fn parse_review_transition_request_bytes(
    bytes: &[u8],
) -> Result<ReviewTransitionRequestV1, ReviewDispositionParseFailureV1> {
    serde_json::from_slice(bytes).map_err(|error| ReviewDispositionParseFailureV1 {
        reason: format!("transition request parse: {error}"),
    })
}

/// Human view of the outcome. Derives from the same ordered semantic
/// result as the JSON view.
#[must_use]
pub fn render_review_disposition_human(outcome: &ReviewDispositionOutcomeV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "review-disposition: repository={} pr={}",
        outcome.repository, outcome.pr_number
    ));
    lines.push(format!("  actor: {}", outcome.actor_class.label()));
    lines.push(format!("  claimed: {}", outcome.claimed_verdict.label()));
    lines.push(format!("  currentness: {}", outcome.currentness.label()));
    if outcome.stale_dimensions.is_empty() {
        lines.push("  stale dimensions: none".to_string());
    } else {
        lines.push(format!(
            "  stale dimensions: {}",
            outcome.stale_dimensions.join(", ")
        ));
    }
    if outcome.blocking_finding_ids.is_empty() {
        lines.push("  blocking findings: none".to_string());
    } else {
        lines.push(format!(
            "  blocking findings: {}",
            outcome.blocking_finding_ids.join(", ")
        ));
    }
    for reason in &outcome.currentness_reasons {
        lines.push(format!("  reason: {reason}"));
    }
    lines.push(format!(
        "  semantic identity: {}",
        outcome.semantic_identity
    ));
    lines.push(format!(
        "  transition: {} -> {}: {}",
        outcome.transition.from_state.label(),
        outcome.transition.to_state.label(),
        if outcome.transition.permitted {
            "permitted"
        } else {
            "not_permitted"
        }
    ));
    for reason in &outcome.transition.reasons {
        lines.push(format!("    reason: {reason}"));
    }
    lines.push(format!("  claim boundary: {}", outcome.claim_boundary));
    lines.join("\n")
}

/// JSON view of the outcome. Derives from the same ordered semantic
/// result as the human view.
pub fn render_review_disposition_json(
    outcome: &ReviewDispositionOutcomeV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(outcome)
}
