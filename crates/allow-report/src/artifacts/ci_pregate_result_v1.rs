//! Stage 1 pre-gate result (#3836).
//!
//! One typed aggregate for the fast deterministic prerequisite tier
//! that rejects cheap source-controlled defects before expensive proof
//! jobs are provisioned. Result law: only `Complete` — or an explicitly
//! valid `NotApplicable` for a selected check that does not apply to
//! the change — permits heavy jobs to start; missing, skipped,
//! cancelled, stale, empty-selection, or instrument-failure states are
//! never green; a diagnostic artifact upload cannot strengthen the
//! result; an empty selection is not `Complete`; stale prior-head
//! results are `Stale`; provider failures are `InstrumentFailure`.
//!
//! Routing law: the pre-gate changes workflow reachability only. It
//! never decides which heavy semantic lanes apply (#2569 stays the
//! routing authority), never reads release secrets, never performs
//! external mutation, and never moves Clippy into Stage 1 without
//! measurement.
//!
//! Claim boundary: a typed reachability gate for the CI workflow
//! graph. It protects expensive-lane start order and first-failure
//! time; it does not select the proof denominator, decide impact
//! classes, or prove repository correctness.

use serde::{Deserialize, Serialize};

pub const CI_PRE_GATE_SCHEMA_ID: &str = "cargo-allow.ci-pregate-result.v1";
pub const CI_PRE_GATE_SCHEMA_VERSION: u32 = 1;

const CLAIM_BOUNDARY: &str = "A typed reachability gate for the CI workflow graph: deterministic cheap checks decide whether expensive proof jobs may start. It does not select the proof denominator, does not decide impact classes, does not read release secrets, does not mutate external state, and is not product or release correctness evidence.";

/// The aggregate states. Only `Complete` and an explicitly valid
/// `NotApplicable` permit heavy jobs to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiPreGateStateV1 {
    Complete,
    Findings,
    NotApplicable,
    Stale,
    Cancelled,
    InstrumentFailure,
}

impl CiPreGateStateV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Findings => "findings",
            Self::NotApplicable => "not_applicable",
            Self::Stale => "stale",
            Self::Cancelled => "cancelled",
            Self::InstrumentFailure => "instrument_failure",
        }
    }

    /// Whether this state permits the selected heavy proof jobs to
    /// start. Missing, skipped, stale, and failed states never do.
    #[must_use]
    pub const fn permits_heavy_jobs(self) -> bool {
        matches!(self, Self::Complete | Self::NotApplicable)
    }
}

/// Terminal state of one selected check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiPreGateCheckStateV1 {
    Passed,
    Failed,
    NotApplicable,
    Skipped,
    Cancelled,
    TimedOut,
    InstrumentFailure,
}

impl CiPreGateCheckStateV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::NotApplicable => "not_applicable",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::InstrumentFailure => "instrument_failure",
        }
    }

    #[must_use]
    pub const fn is_passing_or_not_applicable(self) -> bool {
        matches!(self, Self::Passed | Self::NotApplicable)
    }
}

/// One selected check with its outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiPreGateCheckResultV1 {
    /// Stable check name (the workflow step identity).
    pub name: String,
    pub state: CiPreGateCheckStateV1,
    /// Explicit NotApplicable requires the reason the check cannot
    /// apply; an unexplained NotApplicable is not valid.
    #[serde(default)]
    pub not_applicable_reason: Option<String>,
}

/// The aggregate result for one exact source pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiPreGateResultV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// The exact head the result was computed for; any movement makes
    /// a prior result `Stale`.
    pub head_sha: String,
    /// The base the pair was compared against.
    pub base_sha: String,
    #[serde(default)]
    pub checks: Vec<CiPreGateCheckResultV1>,
    /// Artifact-only diagnostics may upload under failure; recording
    /// one never strengthens the aggregate.
    #[serde(default)]
    pub diagnostics_uploaded: Vec<String>,
    /// Observation limits retained with the result.
    #[serde(default)]
    pub limits: Vec<String>,
    pub claim_boundary: String,
}

/// The evaluated aggregate. `state` is the only permit decision input;
/// heavy jobs read it through the workflow `needs` edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiPreGateEvaluationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub head_sha: String,
    pub state: CiPreGateStateV1,
    /// Ordered, bounded reasons for a non-permitting state.
    pub reasons: Vec<String>,
    pub claim_boundary: String,
}

/// Evaluate the aggregate result against the current head/base.
///
/// - a result bound to a different head is `Stale` (negative control
///   8: an older green never stays current);
/// - an empty selection is never `Complete` (negative control 11:
///   exit zero with no selected checks is not green);
/// - a per-check `NotApplicable` is valid only with an explicit
///   reason; unexplained or contradictory states fail closed;
/// - any failed, skipped, cancelled, timed-out, or instrument-failed
///   check makes the aggregate non-green (negative control 2);
/// - diagnostics never strengthen the result (negative control 3).
#[must_use]
pub fn evaluate_ci_pre_gate(
    result: &CiPreGateResultV1,
    current_head: &str,
) -> CiPreGateEvaluationV1 {
    let mut reasons = Vec::new();
    let state = if result.head_sha != current_head {
        reasons.push(format!(
            "stale: the result was computed for head {} and the current head is {}",
            result.head_sha, current_head
        ));
        CiPreGateStateV1::Stale
    } else if result.checks.is_empty() {
        reasons
            .push("empty selection: no check ran, so the aggregate cannot be complete".to_string());
        CiPreGateStateV1::InstrumentFailure
    } else {
        let mut findings = Vec::new();
        let mut non_passing = Vec::new();
        for check in &result.checks {
            match check.state {
                CiPreGateCheckStateV1::Passed => {}
                CiPreGateCheckStateV1::NotApplicable => {
                    if check
                        .not_applicable_reason
                        .as_deref()
                        .is_none_or(|reason| reason.trim().is_empty())
                    {
                        reasons.push(format!("not_applicable without a reason: {}", check.name));
                    }
                }
                CiPreGateCheckStateV1::Failed => findings.push(check.name.clone()),
                CiPreGateCheckStateV1::Skipped
                | CiPreGateCheckStateV1::Cancelled
                | CiPreGateCheckStateV1::TimedOut
                | CiPreGateCheckStateV1::InstrumentFailure => {
                    non_passing.push(format!("{}: {}", check.name, check.state.label()));
                }
            }
        }
        if !findings.is_empty() {
            reasons.push(format!("findings: {}", findings.join(", ")));
            CiPreGateStateV1::Findings
        } else if !non_passing.is_empty() {
            reasons.push(format!("non-passing checks: {}", non_passing.join(", ")));
            CiPreGateStateV1::InstrumentFailure
        } else {
            let unexplained = result
                .checks
                .iter()
                .filter(|check| check.state == CiPreGateCheckStateV1::NotApplicable)
                .any(|check| {
                    check
                        .not_applicable_reason
                        .as_deref()
                        .is_none_or(|reason| reason.trim().is_empty())
                });
            if unexplained {
                CiPreGateStateV1::InstrumentFailure
            } else {
                let all_not_applicable = result
                    .checks
                    .iter()
                    .all(|check| check.state == CiPreGateCheckStateV1::NotApplicable);
                if all_not_applicable {
                    // An explicitly reasoned all-not-applicable
                    // selection is the one valid non-green permit.
                    CiPreGateStateV1::NotApplicable
                } else {
                    CiPreGateStateV1::Complete
                }
            }
        }
    };
    CiPreGateEvaluationV1 {
        schema_id: CI_PRE_GATE_SCHEMA_ID.to_string(),
        schema_version: CI_PRE_GATE_SCHEMA_VERSION,
        head_sha: current_head.to_string(),
        state,
        reasons,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of the evaluation.
#[must_use]
pub fn render_ci_pre_gate_human(evaluation: &CiPreGateEvaluationV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "ci-pregate: head={} state={}",
        evaluation.head_sha,
        evaluation.state.label()
    ));
    for reason in &evaluation.reasons {
        lines.push(format!("  reason: {reason}"));
    }
    lines.push(format!("  claim boundary: {}", evaluation.claim_boundary));
    lines.join("\n")
}

/// JSON view of the evaluation.
pub fn render_ci_pre_gate_json(
    evaluation: &CiPreGateEvaluationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(evaluation)
}
