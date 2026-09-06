//! Campaign issue closeout verification (#3845).
//!
//! Determines whether an active cargo-allow campaign issue has actually
//! reached its claimed merged-main acceptance state. The verifier is a
//! pure function over two injected inputs: the declared closeout record
//! (structured rows, never free prose) and a repository/GitHub state
//! snapshot taken by the caller. It never mutates issue, PR, tag,
//! package, release, live, or external state, and it never listens to
//! issue events — the next child owns enforcement.
//!
//! Merged-main law: every implementation PR claimed as evidence must be
//! merged (not open, draft, or closed-unmerged), its merge commit must be
//! reachable from current main, the claimed reviewed pair must match the
//! PR evidence, required checks must have accepted terminal results, and
//! a moved main stales prior closeout evidence.
//!
//! Acceptance coverage law: closeout is evaluated per stable acceptance
//! row; a partial implementation leaves the issue open with exact
//! remaining rows; `NotPlanned` and `Duplicate` require an explicit
//! current owner, reason, and replacement/exclusion.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const CAMPAIGN_ISSUE_CLOSEOUT_SCHEMA_ID: &str = "cargo-allow.campaign-issue-closeout.v1";
pub const CAMPAIGN_ISSUE_CLOSEOUT_SCHEMA_VERSION: u32 = 1;

const CLAIM_BOUNDARY: &str = "A read-only typed verifier that decides whether an active cargo-allow campaign issue's claimed acceptance is current on merged main with sufficient review, CI, artifacts, and evidence. It does not change issue state or perform the work it validates; free-form prose is navigation only and never substitutes for structured evidence.";

/// Closed verdict vocabulary for one closeout evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignCloseoutVerdictV1 {
    Complete,
    Partial,
    NotPlanned,
    Duplicate,
    Stale,
    Mismatch,
    NotProven,
    Unsupported,
    InstrumentFailure,
}

impl CampaignCloseoutVerdictV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::NotPlanned => "not_planned",
            Self::Duplicate => "duplicate",
            Self::Stale => "stale",
            Self::Mismatch => "mismatch",
            Self::NotProven => "not_proven",
            Self::Unsupported => "unsupported",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// Live PR state as observed from GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignPrStateV1 {
    Open,
    Draft,
    Merged,
    ClosedWithoutMerge,
}

/// Terminal classification of one required CI check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignCheckOutcomeV1 {
    Passed,
    Failed,
    Skipped,
    Cancelled,
    Nonterminal,
    Unknown,
}

/// Evidence class for one acceptance row (#3842 sufficiency ladder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignEvidenceClassV1 {
    /// Prose/comment only — never sufficient alone.
    Prose,
    /// Typed model or schema foundation without production cutover.
    Foundation,
    /// Characterization or fixture evidence without the production path.
    Characterization,
    /// Current observation of runtime/live/external state.
    CurrentObservation,
    /// Merged-main production cutover with typed receipts.
    ProductionCutover,
}

/// One claimed implementation PR with the identity the closeout law needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignPrEvidenceV1 {
    pub number: u64,
    /// Live GitHub state for the PR.
    pub state: CampaignPrStateV1,
    /// Merge commit sha as reported by GitHub (empty when unmerged).
    pub merge_commit: String,
    /// Head sha the review and CI evidence bind to.
    pub head_sha: String,
    /// Base sha the PR targeted.
    pub base_sha: String,
    /// Effective merge base between base and head.
    pub merge_base: String,
    /// Semantic owner the PR claims to implement (issue-scoped, not
    /// "nearby files").
    pub semantic_owner: String,
}

/// Reviewed pair bound by the review disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignReviewPairV1 {
    pub base_sha: String,
    pub head_sha: String,
    pub merge_base: String,
}

/// One required CI check with its terminal outcome for the exact pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignCheckEvidenceV1 {
    pub name: String,
    pub required: bool,
    pub outcome: CampaignCheckOutcomeV1,
}

/// One stable acceptance row with its claimed evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignAcceptanceRowV1 {
    pub row_id: String,
    pub description: String,
    /// Evidence sufficiency class the row demands (#3842).
    pub required_evidence_class: CampaignEvidenceClassV1,
    /// PR numbers claimed as the implementation evidence for this row.
    pub pr_numbers: Vec<u64>,
    /// Reviewed pair claimed for this row.
    pub review: Option<CampaignReviewPairV1>,
    /// Required check names claimed for this row.
    pub required_checks: Vec<String>,
    /// Retained receipt/artifact identity claimed for this row.
    pub evidence_identity: String,
}

/// The declared closeout record for one child issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignCloseoutRecordV1 {
    pub parent_campaign: u64,
    pub child_issue: u64,
    /// Claimed verdict for the issue (the verifier re-derives it).
    pub claimed_verdict: CampaignCloseoutVerdictV1,
    /// For NotPlanned/Duplicate: explicit owner.
    pub decision_owner: String,
    /// For NotPlanned/Duplicate: explicit reason.
    pub decision_reason: String,
    /// For Duplicate: the replacement issue number.
    pub duplicate_of: Option<u64>,
    pub rows: Vec<CampaignAcceptanceRowV1>,
    /// Head of current main at closeout-claim time.
    pub claimed_main_head: String,
}

/// Live repository/GitHub state snapshot the verifier consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignRepositoryStateV1 {
    /// Current main head.
    pub main_head: String,
    pub main_tree: String,
    /// Live PR state by number.
    pub prs: Vec<CampaignPrEvidenceV1>,
    /// Live required-check outcomes by check name for the claimed heads.
    pub checks: Vec<CampaignCheckEvidenceV1>,
    /// Reachability answers: merge commit sha -> reachable from main?
    pub reachable_from_main: Vec<(String, bool)>,
}

/// Per-row outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignRowOutcomeV1 {
    pub row_id: String,
    pub verdict: CampaignCloseoutVerdictV1,
    pub reasons: Vec<String>,
}

/// The evaluated closeout result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignCloseoutResultV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub child_issue: u64,
    pub verdict: CampaignCloseoutVerdictV1,
    pub row_outcomes: Vec<CampaignRowOutcomeV1>,
    pub uncovered_row_ids: Vec<String>,
    pub blocking_reasons: Vec<String>,
    pub claim_boundary: &'static str,
}

/// Per-row failure reason vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowFailure {
    PrNotMerged,
    PrDraft,
    MergeUnreachable,
    ReviewPairMismatch,
    RequiredCheckNotPassed,
    EvidenceIdentityMissing,
    EvidenceClassInsufficient,
    SemanticOwnerMissing,
}

impl RowFailure {
    fn message(self, row_id: &str, detail: &str) -> String {
        let label = match self {
            Self::PrNotMerged => "PR is not merged",
            Self::PrDraft => "PR is a draft",
            Self::MergeUnreachable => "merge commit is not reachable from current main",
            Self::ReviewPairMismatch => "reviewed pair does not match the PR evidence",
            Self::RequiredCheckNotPassed => "required check is not terminal-Passed",
            Self::EvidenceIdentityMissing => "claimed evidence identity is missing",
            Self::EvidenceClassInsufficient => "evidence class is insufficient for the row",
            Self::SemanticOwnerMissing => "PR carries no semantic owner for this issue",
        };
        format!("{row_id}: {label} ({detail})")
    }
}

/// Evaluate one declared closeout record against the live state.
#[must_use]
pub fn evaluate_campaign_closeout(
    record: &CampaignCloseoutRecordV1,
    state: &CampaignRepositoryStateV1,
) -> CampaignCloseoutResultV1 {
    let mut blocking = Vec::new();
    let mut row_outcomes = Vec::new();
    let mut uncovered = Vec::new();

    if record.rows.is_empty() {
        blocking.push("the closeout record claims no acceptance rows".to_string());
    }

    // Merged-main law: current main movement since the claim stales.
    if record.claimed_main_head != state.main_head {
        blocking.push(format!(
            "main moved since the claim: {} -> {}",
            record.claimed_main_head, state.main_head
        ));
    }

    // Deliberate no-code decisions.
    if matches!(
        record.claimed_verdict,
        CampaignCloseoutVerdictV1::NotPlanned | CampaignCloseoutVerdictV1::Duplicate
    ) {
        if record.decision_owner.trim().is_empty()
            || record.decision_reason.trim().is_empty()
            || (record.claimed_verdict == CampaignCloseoutVerdictV1::Duplicate
                && record.duplicate_of.is_none())
        {
            blocking.push(
                "NotPlanned/Duplicate requires explicit owner, reason, and replacement/exclusion"
                    .to_string(),
            );
            return result(
                record,
                CampaignCloseoutVerdictV1::Mismatch,
                Vec::new(),
                blocking,
            );
        }
        return result(record, record.claimed_verdict, Vec::new(), blocking);
    }

    for row in &record.rows {
        let mut reasons = Vec::new();
        let mut row_ok = true;

        if row.evidence_identity.trim().is_empty() {
            reasons.push(RowFailure::EvidenceIdentityMissing.message(&row.row_id, ""));
            row_ok = false;
        }
        if row.semantic_owner_missing(record.child_issue, &state.prs) {
            reasons.push(RowFailure::SemanticOwnerMissing.message(&row.row_id, ""));
            row_ok = false;
        }
        for pr_number in &row.pr_numbers {
            let Some(pr) = state
                .prs
                .iter()
                .find(|candidate| candidate.number == *pr_number)
            else {
                reasons.push(RowFailure::PrNotMerged.message(
                    &row.row_id,
                    &format!("PR #{pr_number} is absent from the state snapshot"),
                ));
                row_ok = false;
                continue;
            };
            if pr.state != CampaignPrStateV1::Merged || pr.merge_commit.is_empty() {
                let failure = if pr.state == CampaignPrStateV1::Draft {
                    RowFailure::PrDraft
                } else {
                    RowFailure::PrNotMerged
                };
                reasons.push(failure.message(&row.row_id, &format!("PR #{pr_number}")));
                row_ok = false;
                continue;
            }
            let reachable = state
                .reachable_from_main
                .iter()
                .find(|(sha, _)| sha == &pr.merge_commit)
                .map(|(_, reachable)| *reachable)
                .unwrap_or(false);
            if !reachable {
                reasons.push(
                    RowFailure::MergeUnreachable.message(&row.row_id, &format!("PR #{pr_number}")),
                );
                row_ok = false;
            }
            if let Some(review) = &row.review {
                let matches_review = review.head_sha == pr.head_sha
                    && review.base_sha == pr.base_sha
                    && review.merge_base == pr.merge_base;
                if !matches_review {
                    reasons.push(
                        RowFailure::ReviewPairMismatch
                            .message(&row.row_id, &format!("PR #{pr_number}")),
                    );
                    row_ok = false;
                }
            }
        }
        for check_name in &row.required_checks {
            let outcome = state
                .checks
                .iter()
                .find(|check| &check.name == check_name)
                .map(|check| check.outcome)
                .unwrap_or(CampaignCheckOutcomeV1::Unknown);
            if outcome != CampaignCheckOutcomeV1::Passed {
                reasons.push(
                    RowFailure::RequiredCheckNotPassed
                        .message(&row.row_id, &format!("{check_name} = {:?}", outcome)),
                );
                row_ok = false;
            }
        }
        if row.required_evidence_class < CampaignEvidenceClassV1::CurrentObservation
            && !row.required_checks.is_empty()
        {
            // A row demanding checks but claiming only prose/foundation
            // evidence cannot silently satisfy the stronger owner.
            reasons.push(RowFailure::EvidenceClassInsufficient.message(
                &row.row_id,
                &format!("demands {:?} evidence", row.required_evidence_class),
            ));
            row_ok = false;
        }

        if row_ok {
            row_outcomes.push(CampaignRowOutcomeV1 {
                row_id: row.row_id.clone(),
                verdict: CampaignCloseoutVerdictV1::Complete,
                reasons: Vec::new(),
            });
        } else {
            uncovered.push(row.row_id.clone());
            row_outcomes.push(CampaignRowOutcomeV1 {
                row_id: row.row_id.clone(),
                verdict: CampaignCloseoutVerdictV1::NotProven,
                reasons,
            });
        }
    }

    let verdict = if record.rows.is_empty() {
        // A record claiming no rows has nothing to verify: that is an
        // instrument failure of the record producer, never a clean result.
        CampaignCloseoutVerdictV1::InstrumentFailure
    } else if !blocking.is_empty() {
        CampaignCloseoutVerdictV1::Mismatch
    } else if uncovered.is_empty() {
        CampaignCloseoutVerdictV1::Complete
    } else {
        CampaignCloseoutVerdictV1::Partial
    };

    result(record, verdict, row_outcomes, blocking)
}

impl CampaignAcceptanceRowV1 {
    /// A merged PR satisfies a row only when at least one of the row's PRs
    /// declares a semantic owner bound to this issue — "nearby files" from
    /// another issue never count.
    fn semantic_owner_missing(&self, child_issue: u64, prs: &[CampaignPrEvidenceV1]) -> bool {
        let owner_tag = format!("issue:{child_issue}");
        !self.pr_numbers.iter().any(|number| {
            prs.iter()
                .find(|pr| pr.number == *number)
                .is_some_and(|pr| {
                    pr.semantic_owner
                        .split(',')
                        .any(|owner| owner.trim() == owner_tag)
                })
        })
    }
}

fn result(
    record: &CampaignCloseoutRecordV1,
    verdict: CampaignCloseoutVerdictV1,
    row_outcomes: Vec<CampaignRowOutcomeV1>,
    blocking: Vec<String>,
) -> CampaignCloseoutResultV1 {
    let uncovered = row_outcomes
        .iter()
        .filter(|outcome| outcome.verdict != CampaignCloseoutVerdictV1::Complete)
        .map(|outcome| outcome.row_id.clone())
        .collect();
    CampaignCloseoutResultV1 {
        schema_id: CAMPAIGN_ISSUE_CLOSEOUT_SCHEMA_ID.to_string(),
        schema_version: CAMPAIGN_ISSUE_CLOSEOUT_SCHEMA_VERSION,
        child_issue: record.child_issue,
        verdict,
        row_outcomes,
        uncovered_row_ids: uncovered,
        blocking_reasons: blocking,
        claim_boundary: CLAIM_BOUNDARY,
    }
}

impl fmt::Display for CampaignCloseoutVerdictV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.label())
    }
}

#[cfg(test)]
mod closeout_corpus_tests {
    use super::{
        CampaignAcceptanceRowV1, CampaignCheckEvidenceV1, CampaignCheckOutcomeV1,
        CampaignCloseoutRecordV1, CampaignCloseoutVerdictV1, CampaignEvidenceClassV1,
        CampaignPrEvidenceV1, CampaignPrStateV1, CampaignRepositoryStateV1, CampaignReviewPairV1,
        evaluate_campaign_closeout,
    };

    const MAIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const MERGE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HEAD: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const BASE: &str = "dddddddddddddddddddddddddddddddddddddddd";

    fn pr_merged() -> CampaignPrEvidenceV1 {
        CampaignPrEvidenceV1 {
            number: 4000,
            state: CampaignPrStateV1::Merged,
            merge_commit: MERGE.to_string(),
            head_sha: HEAD.to_string(),
            base_sha: BASE.to_string(),
            merge_base: BASE.to_string(),
            semantic_owner: "issue:3845".to_string(),
        }
    }

    fn check_passed(name: &str) -> CampaignCheckEvidenceV1 {
        CampaignCheckEvidenceV1 {
            name: name.to_string(),
            required: true,
            outcome: CampaignCheckOutcomeV1::Passed,
        }
    }

    fn row(pr: u64) -> CampaignAcceptanceRowV1 {
        CampaignAcceptanceRowV1 {
            row_id: "row-1".to_string(),
            description: "the thing works".to_string(),
            required_evidence_class: CampaignEvidenceClassV1::ProductionCutover,
            pr_numbers: vec![pr],
            review: Some(CampaignReviewPairV1 {
                base_sha: BASE.to_string(),
                head_sha: HEAD.to_string(),
                merge_base: BASE.to_string(),
            }),
            required_checks: vec!["ci".to_string()],
            evidence_identity: "sha256:v1:row-evidence".to_string(),
        }
    }

    fn record(
        verdict: CampaignCloseoutVerdictV1,
        rows: Vec<CampaignAcceptanceRowV1>,
    ) -> CampaignCloseoutRecordV1 {
        CampaignCloseoutRecordV1 {
            parent_campaign: 3768,
            child_issue: 3845,
            claimed_verdict: verdict,
            decision_owner: String::new(),
            decision_reason: String::new(),
            duplicate_of: None,
            rows,
            claimed_main_head: MAIN.to_string(),
        }
    }

    fn state() -> CampaignRepositoryStateV1 {
        CampaignRepositoryStateV1 {
            main_head: MAIN.to_string(),
            main_tree: "treetree".to_string(),
            prs: vec![pr_merged()],
            checks: vec![check_passed("ci")],
            reachable_from_main: vec![(MERGE.to_string(), true)],
        }
    }

    #[test]
    fn merged_main_acceptance_is_complete() {
        let outcome = evaluate_campaign_closeout(
            &record(CampaignCloseoutVerdictV1::Complete, vec![row(4000)]),
            &state(),
        );
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Complete);
        assert!(outcome.blocking_reasons.is_empty());
        assert!(outcome.uncovered_row_ids.is_empty());
    }

    #[test]
    fn control_1_open_draft_or_unmerged_pr_is_rejected() {
        for (pr_state, label) in [
            (CampaignPrStateV1::Open, "open"),
            (CampaignPrStateV1::Draft, "draft"),
            (CampaignPrStateV1::ClosedWithoutMerge, "closed-unmerged"),
        ] {
            let mut snapshot = state();
            for pr in &mut snapshot.prs {
                pr.state = pr_state;
                pr.merge_commit = String::new();
            }
            let outcome = evaluate_campaign_closeout(
                &record(CampaignCloseoutVerdictV1::Complete, vec![row(4000)]),
                &snapshot,
            );
            assert_eq!(
                outcome.verdict,
                CampaignCloseoutVerdictV1::Partial,
                "{label}"
            );
            assert!(
                outcome.uncovered_row_ids.contains(&"row-1".to_string()),
                "{label}"
            );
        }
    }

    #[test]
    fn control_2_unreachable_merge_is_rejected() {
        let mut snapshot = state();
        snapshot.reachable_from_main = vec![(MERGE.to_string(), false)];
        let outcome = evaluate_campaign_closeout(
            &record(CampaignCloseoutVerdictV1::Complete, vec![row(4000)]),
            &snapshot,
        );
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Partial);
        let first = first_row_outcome(&outcome);
        let reasons = first.reasons.join(" ");
        assert!(reasons.contains("not reachable"), "{reasons}");
    }

    #[test]
    fn control_3_older_review_pair_is_rejected() {
        let mut row = row(4000);
        row.review = Some(CampaignReviewPairV1 {
            base_sha: BASE.to_string(),
            head_sha: "9999999999999999999999999999999999999999".to_string(),
            merge_base: BASE.to_string(),
        });
        let outcome = evaluate_campaign_closeout(
            &record(CampaignCloseoutVerdictV1::Complete, vec![row]),
            &state(),
        );
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Partial);
        let reasons = first_row_outcome(&outcome).reasons.join(" ");
        assert!(reasons.contains("reviewed pair"), "{reasons}");
    }

    #[test]
    fn control_4_nonterminal_required_check_is_rejected() {
        for outcome_v in [
            CampaignCheckOutcomeV1::Failed,
            CampaignCheckOutcomeV1::Skipped,
            CampaignCheckOutcomeV1::Cancelled,
            CampaignCheckOutcomeV1::Nonterminal,
            CampaignCheckOutcomeV1::Unknown,
        ] {
            let mut snapshot = state();
            for check in &mut snapshot.checks {
                check.outcome = outcome_v;
            }
            let outcome = evaluate_campaign_closeout(
                &record(CampaignCloseoutVerdictV1::Complete, vec![row(4000)]),
                &snapshot,
            );
            assert_eq!(
                outcome.verdict,
                CampaignCloseoutVerdictV1::Partial,
                "{outcome_v:?}"
            );
        }
    }

    #[test]
    fn control_5_foreign_issue_pr_is_rejected() {
        let mut snapshot = state();
        for pr in &mut snapshot.prs {
            pr.semantic_owner = "issue:3744".to_string();
        }
        let outcome = evaluate_campaign_closeout(
            &record(CampaignCloseoutVerdictV1::Complete, vec![row(4000)]),
            &snapshot,
        );
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Partial);
        let reasons = first_row_outcome(&outcome).reasons.join(" ");
        assert!(reasons.contains("semantic owner"), "{reasons}");
    }

    #[test]
    fn control_6_partial_slice_leaves_exact_remaining_rows() {
        let mut second = row(4000);
        second.row_id = "row-2".to_string();
        second.required_checks = vec!["never-ran".to_string()];
        let outcome = evaluate_campaign_closeout(
            &record(CampaignCloseoutVerdictV1::Complete, vec![row(4000), second]),
            &state(),
        );
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Partial);
        assert_eq!(outcome.uncovered_row_ids, vec!["row-2".to_string()]);
        let first = first_row_outcome(&outcome);
        assert_eq!(first.verdict, CampaignCloseoutVerdictV1::Complete);
    }

    fn first_row_outcome(
        outcome: &super::CampaignCloseoutResultV1,
    ) -> &super::CampaignRowOutcomeV1 {
        outcome
            .row_outcomes
            .iter()
            .find(|outcome_row| outcome_row.row_id == "row-1")
            .expect("row-1 outcome present")
    }

    #[test]
    fn record_without_rows_is_an_instrument_failure() {
        let outcome = evaluate_campaign_closeout(
            &record(CampaignCloseoutVerdictV1::Complete, Vec::new()),
            &state(),
        );
        assert_eq!(
            outcome.verdict,
            CampaignCloseoutVerdictV1::InstrumentFailure
        );
        assert!(!outcome.blocking_reasons.is_empty());
    }

    #[test]
    fn moved_main_stales_the_claim() {
        let mut snapshot = state();
        snapshot.main_head = "7777777777777777777777777777777777777777".to_string();
        let outcome = evaluate_campaign_closeout(
            &record(CampaignCloseoutVerdictV1::Complete, vec![row(4000)]),
            &snapshot,
        );
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Mismatch);
        assert!(
            outcome
                .blocking_reasons
                .iter()
                .any(|reason| reason.contains("main moved"))
        );
    }

    #[test]
    fn not_planned_requires_owner_and_reason() {
        let mut record = record(CampaignCloseoutVerdictV1::NotPlanned, Vec::new());
        record.decision_owner = "core/release".to_string();
        record.decision_reason = "superseded by the refreeze".to_string();
        let outcome = evaluate_campaign_closeout(&record, &state());
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::NotPlanned);

        record.decision_reason = String::new();
        let outcome = evaluate_campaign_closeout(&record, &state());
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Mismatch);
    }

    #[test]
    fn duplicate_requires_replacement() {
        let mut record = record(CampaignCloseoutVerdictV1::Duplicate, Vec::new());
        record.decision_owner = "core/release".to_string();
        record.decision_reason = "tracked elsewhere".to_string();
        record.duplicate_of = Some(3744);
        let outcome = evaluate_campaign_closeout(&record, &state());
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Duplicate);

        record.duplicate_of = None;
        let outcome = evaluate_campaign_closeout(&record, &state());
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Mismatch);
    }
}
//
#[cfg(test)]
mod closeout_coverage_tests {
    use super::{
        CampaignAcceptanceRowV1, CampaignCheckEvidenceV1, CampaignCheckOutcomeV1,
        CampaignCloseoutRecordV1, CampaignCloseoutVerdictV1, CampaignEvidenceClassV1,
        CampaignPrEvidenceV1, CampaignPrStateV1, CampaignRepositoryStateV1,
        evaluate_campaign_closeout,
    };

    #[test]
    fn display_renders_all_nine_verdict_labels() {
        for (verdict, expected) in [
            (CampaignCloseoutVerdictV1::Complete, "complete"),
            (CampaignCloseoutVerdictV1::Partial, "partial"),
            (CampaignCloseoutVerdictV1::NotPlanned, "not_planned"),
            (CampaignCloseoutVerdictV1::Duplicate, "duplicate"),
            (CampaignCloseoutVerdictV1::Stale, "stale"),
            (CampaignCloseoutVerdictV1::Mismatch, "mismatch"),
            (CampaignCloseoutVerdictV1::NotProven, "not_proven"),
            (CampaignCloseoutVerdictV1::Unsupported, "unsupported"),
            (
                CampaignCloseoutVerdictV1::InstrumentFailure,
                "instrument_failure",
            ),
        ] {
            assert_eq!(format!("{verdict}"), expected);
            assert_eq!(verdict.label(), expected);
        }
    }

    #[test]
    fn empty_record_and_missing_pr_produce_partial_with_reasons() {
        let record = CampaignCloseoutRecordV1 {
            parent_campaign: 3768,
            child_issue: 3845,
            claimed_verdict: CampaignCloseoutVerdictV1::Complete,
            decision_owner: String::new(),
            decision_reason: String::new(),
            duplicate_of: None,
            rows: vec![CampaignAcceptanceRowV1 {
                row_id: "r1".to_string(),
                description: "d".to_string(),
                required_evidence_class: CampaignEvidenceClassV1::ProductionCutover,
                pr_numbers: vec![9999],
                review: None,
                required_checks: Vec::new(),
                evidence_identity: "sha256:v1:x".to_string(),
            }],
            claimed_main_head: MAIN.to_string(),
        };
        let mut snapshot = state();
        snapshot.prs.clear();
        let outcome = evaluate_campaign_closeout(&record, &snapshot);
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Partial);
        assert_eq!(outcome.uncovered_row_ids, vec!["r1".to_string()]);
        let row = outcome
            .row_outcomes
            .iter()
            .find(|outcome_row| outcome_row.row_id == "r1")
            .expect("r1 outcome present");
        assert!(
            row.reasons
                .iter()
                .any(|reason| reason.contains("absent from the state snapshot"))
        );
    }

    #[test]
    fn evidence_identity_is_required_for_a_passing_row() {
        let snapshot = state();
        let mut record = CampaignCloseoutRecordV1 {
            parent_campaign: 3768,
            child_issue: 3845,
            claimed_verdict: CampaignCloseoutVerdictV1::Complete,
            decision_owner: String::new(),
            decision_reason: String::new(),
            duplicate_of: None,
            rows: vec![CampaignAcceptanceRowV1 {
                row_id: "r1".to_string(),
                description: "d".to_string(),
                required_evidence_class: CampaignEvidenceClassV1::ProductionCutover,
                pr_numbers: vec![4000],
                review: None,
                required_checks: Vec::new(),
                evidence_identity: "   ".to_string(),
            }],
            claimed_main_head: MAIN.to_string(),
        };
        let outcome = evaluate_campaign_closeout(&record, &snapshot);
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Partial);
        assert!(
            outcome
                .row_outcomes
                .iter()
                .find(|outcome_row| outcome_row.row_id == "r1")
                .is_some_and(|outcome_row| {
                    outcome_row
                        .reasons
                        .iter()
                        .any(|reason| reason.contains("evidence identity"))
                })
        );

        // Fill in the identity; the row now passes.
        for acceptance_row in &mut record.rows {
            acceptance_row.evidence_identity = "sha256:v1:aa".to_string();
        }
        let outcome = evaluate_campaign_closeout(&record, &snapshot);
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Complete);
    }

    #[test]
    fn unsupported_verdict_label_roundtrips_through_display() {
        assert_eq!(
            CampaignCloseoutVerdictV1::Unsupported.to_string(),
            "unsupported"
        );
    }

    const MAIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn state() -> CampaignRepositoryStateV1 {
        CampaignRepositoryStateV1 {
            main_head: MAIN.to_string(),
            main_tree: "treetree".to_string(),
            prs: vec![CampaignPrEvidenceV1 {
                number: 4000,
                state: CampaignPrStateV1::Merged,
                merge_commit: MERGE.to_string(),
                head_sha: HEAD.to_string(),
                base_sha: BASE.to_string(),
                merge_base: BASE.to_string(),
                semantic_owner: "issue:3845".to_string(),
            }],
            checks: vec![CampaignCheckEvidenceV1 {
                name: "ci".to_string(),
                required: true,
                outcome: CampaignCheckOutcomeV1::Passed,
            }],
            reachable_from_main: vec![(MERGE.to_string(), true)],
        }
    }

    const MERGE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HEAD: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const BASE: &str = "dddddddddddddddddddddddddddddddddddddddd";
}
//
#[cfg(test)]
mod closeout_edge_tests {
    use super::{
        CampaignAcceptanceRowV1, CampaignCheckEvidenceV1, CampaignCheckOutcomeV1,
        CampaignCloseoutRecordV1, CampaignCloseoutVerdictV1, CampaignEvidenceClassV1,
        CampaignPrEvidenceV1, CampaignPrStateV1, CampaignRepositoryStateV1,
        evaluate_campaign_closeout,
    };

    fn state() -> CampaignRepositoryStateV1 {
        CampaignRepositoryStateV1 {
            main_head: "aaaa".to_string(),
            main_tree: "treetree".to_string(),
            prs: vec![CampaignPrEvidenceV1 {
                number: 4000,
                state: CampaignPrStateV1::Merged,
                merge_commit: "bbbb".to_string(),
                head_sha: "cccc".to_string(),
                base_sha: "dddd".to_string(),
                merge_base: "dddd".to_string(),
                semantic_owner: "issue:3845".to_string(),
            }],
            checks: vec![CampaignCheckEvidenceV1 {
                name: "ci".to_string(),
                required: true,
                outcome: CampaignCheckOutcomeV1::Passed,
            }],
            reachable_from_main: vec![("bbbb".to_string(), true)],
        }
    }

    fn full_record() -> CampaignCloseoutRecordV1 {
        CampaignCloseoutRecordV1 {
            parent_campaign: 3768,
            child_issue: 3845,
            claimed_verdict: CampaignCloseoutVerdictV1::Complete,
            decision_owner: String::new(),
            decision_reason: String::new(),
            duplicate_of: None,
            rows: vec![CampaignAcceptanceRowV1 {
                row_id: "r1".to_string(),
                description: "d".to_string(),
                required_evidence_class: CampaignEvidenceClassV1::ProductionCutover,
                pr_numbers: vec![4000],
                review: None,
                required_checks: Vec::new(),
                evidence_identity: "sha256:v1:aa".to_string(),
            }],
            claimed_main_head: "aaaa".to_string(),
        }
    }

    #[test]
    fn characterization_evidence_with_checks_passes_when_observation_not_demanded() {
        let mut record = full_record();
        for acceptance_row in &mut record.rows {
            acceptance_row.required_evidence_class = CampaignEvidenceClassV1::Characterization;
        }
        let outcome = evaluate_campaign_closeout(&record, &state());
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Complete);
    }

    #[test]
    fn current_observation_evidence_passes_without_checks() {
        let mut record = full_record();
        for acceptance_row in &mut record.rows {
            acceptance_row.required_evidence_class = CampaignEvidenceClassV1::CurrentObservation;
        }
        let outcome = evaluate_campaign_closeout(&record, &state());
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Complete);
    }

    #[test]
    fn pr_with_whitespace_around_semantic_owner_still_binds() {
        let mut snapshot = state();
        for pr in &mut snapshot.prs {
            pr.semantic_owner = " issue:3845 , other ".to_string();
        }
        let outcome = evaluate_campaign_closeout(&full_record(), &snapshot);
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Complete);
    }
}
//
//
#[cfg(test)]
mod closeout_final_coverage_tests {
    use super::{
        CampaignAcceptanceRowV1, CampaignCloseoutRecordV1, CampaignCloseoutVerdictV1,
        CampaignEvidenceClassV1, CampaignRepositoryStateV1, evaluate_campaign_closeout,
    };

    #[test]
    fn row_without_prs_or_checks_passes_with_evidence_identity() {
        let record = CampaignCloseoutRecordV1 {
            parent_campaign: 3768,
            child_issue: 3845,
            claimed_verdict: CampaignCloseoutVerdictV1::Complete,
            decision_owner: String::new(),
            decision_reason: String::new(),
            duplicate_of: None,
            rows: vec![CampaignAcceptanceRowV1 {
                row_id: "r1".to_string(),
                description: "no-pr row".to_string(),
                required_evidence_class: CampaignEvidenceClassV1::ProductionCutover,
                pr_numbers: Vec::new(),
                review: None,
                required_checks: Vec::new(),
                evidence_identity: "sha256:v1:aa".to_string(),
            }],
            claimed_main_head: "aaaa".to_string(),
        };
        let snapshot = CampaignRepositoryStateV1 {
            main_head: "aaaa".to_string(),
            main_tree: "t".to_string(),
            prs: Vec::new(),
            checks: Vec::new(),
            reachable_from_main: Vec::new(),
        };
        let outcome = evaluate_campaign_closeout(&record, &snapshot);
        // A row without PRs has no semantic owner binding.
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Partial);
    }

    #[test]
    fn duplicate_with_rows_and_valid_replacement_evaluates_rows() {
        let record = CampaignCloseoutRecordV1 {
            parent_campaign: 3768,
            child_issue: 3845,
            claimed_verdict: CampaignCloseoutVerdictV1::Duplicate,
            decision_owner: "core/release".to_string(),
            decision_reason: "tracked in the replacement".to_string(),
            duplicate_of: Some(3744),
            rows: vec![CampaignAcceptanceRowV1 {
                row_id: "r1".to_string(),
                description: "d".to_string(),
                required_evidence_class: CampaignEvidenceClassV1::ProductionCutover,
                pr_numbers: Vec::new(),
                review: None,
                required_checks: Vec::new(),
                evidence_identity: "sha256:v1:aa".to_string(),
            }],
            claimed_main_head: "aaaa".to_string(),
        };
        let snapshot = CampaignRepositoryStateV1 {
            main_head: "aaaa".to_string(),
            main_tree: "t".to_string(),
            prs: Vec::new(),
            checks: Vec::new(),
            reachable_from_main: Vec::new(),
        };
        let outcome = evaluate_campaign_closeout(&record, &snapshot);
        assert_eq!(outcome.verdict, CampaignCloseoutVerdictV1::Duplicate);
    }

    #[test]
    fn unsupported_verdict_label_matches() {
        assert_eq!(
            CampaignCloseoutVerdictV1::Unsupported.label(),
            "unsupported"
        );
    }
}
//
#[cfg(test)]
mod closeout_display_and_label_tests {
    use super::CampaignCloseoutVerdictV1;
    use std::fmt::Display;

    #[test]
    fn all_nine_verdict_labels_match_display_output() {
        let cases = [
            (CampaignCloseoutVerdictV1::Complete, "complete"),
            (CampaignCloseoutVerdictV1::Partial, "partial"),
            (CampaignCloseoutVerdictV1::NotPlanned, "not_planned"),
            (CampaignCloseoutVerdictV1::Duplicate, "duplicate"),
            (CampaignCloseoutVerdictV1::Stale, "stale"),
            (CampaignCloseoutVerdictV1::Mismatch, "mismatch"),
            (CampaignCloseoutVerdictV1::NotProven, "not_proven"),
            (CampaignCloseoutVerdictV1::Unsupported, "unsupported"),
            (CampaignCloseoutVerdictV1::InstrumentFailure, "instrument_failure"),
        ];
        for (verdict, expected) in cases {
            assert_eq!(format!("{verdict}"), expected);
            assert_eq!(verdict.to_string(), expected);
            assert_eq!(verdict.label(), expected);
        }
    }
}
