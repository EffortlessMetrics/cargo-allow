//! The workflow construction aggregate (#3907 PR D): one stable
//! semantic report combining the pinned syntax lane and the qualified
//! security lane over the same denominator, with per-lane results
//! preserved verbatim so aggregation never masks evidence.
//!
//! Laws:
//! - the aggregate result is `clean` only when both lane reports are
//!   clean; `findings` when either lane has findings; and
//!   `instrument_failure` when either lane is an instrument failure
//!   (an instrument failure dominates findings — a dead tool must not
//!   look like a passing one);
//! - both lane reports must grade against the same denominator digest;
//!   a mismatch is an aggregate instrument failure;
//! - findings stay visible in the aggregate: exception-accepted
//!   security findings keep their disposition, and advisory examples
//!   or candidates never block unrelated source changes (enforcement
//!   selection is the aggregate lane's authority under #2283/#2284);
//! - the aggregate carries the retained artifact identity so the
//!   report can be stored under failure without masking the result.

use serde::{Deserialize, Serialize};

use crate::artifacts::workflow_construction_v1::WorkflowConstructionInventoryV1;
use crate::artifacts::workflow_security_lane_v1::{
    WorkflowSecurityDispositionV1, WorkflowSecurityLaneReportV1, WorkflowSecurityLaneResultV1,
};
use crate::artifacts::workflow_syntax_lane_v1::{
    WorkflowSyntaxLaneReportV1, WorkflowSyntaxLaneResultV1,
};

pub const WORKFLOW_CONSTRUCTION_AGGREGATE_SCHEMA_ID: &str =
    "cargo-allow.workflow-construction-aggregate.v1";
pub const WORKFLOW_CONSTRUCTION_AGGREGATE_SCHEMA_VERSION: u32 = 1;

const AGGREGATE_CLAIM_BOUNDARY: &str = "Stable semantic aggregate over the pinned syntax lane and the qualified security lane: per-lane results are preserved verbatim, an instrument failure dominates findings, and the advisory posture holds until the aggregate lane's authority (#2283/#2284) selects enforcement. No workflow is mutated and no network lookup enters ordinary cargo-allow scans.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConstructionAggregateResultV1 {
    Clean,
    Findings,
    InstrumentFailure,
}

impl WorkflowConstructionAggregateResultV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Findings => "findings",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// One lane's verbatim result inside the aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConstructionAggregateLaneV1 {
    pub lane: String,
    pub result: String,
    pub finding_count: u32,
}

/// The stable semantic aggregate over both construction lanes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConstructionAggregateV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// The inventory content digest both lanes graded against.
    pub denominator_digest: String,
    pub syntax_lane: WorkflowConstructionAggregateLaneV1,
    pub security_lane: WorkflowConstructionAggregateLaneV1,
    pub result: WorkflowConstructionAggregateResultV1,
    pub open_security_findings: u32,
    pub accepted_security_findings: u32,
    pub syntax_findings: u32,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Combine the two lane reports into the stable aggregate. Pure.
#[must_use]
pub fn aggregate_workflow_construction(
    inventory: &WorkflowConstructionInventoryV1,
    syntax: &WorkflowSyntaxLaneReportV1,
    security: &WorkflowSecurityLaneReportV1,
) -> WorkflowConstructionAggregateV1 {
    let mut instrument: Vec<String> = Vec::new();

    // Both lanes must have graded the same denominator: a digest
    // mismatch means the reports came from different tree states.
    if syntax.denominator_digest != security.denominator_digest {
        instrument.push(
            "lane denominator digests disagree; the reports came from different tree states"
                .to_string(),
        );
    }

    // Schema identity per lane.
    if syntax.schema_id != crate::artifacts::workflow_syntax_lane_v1::WORKFLOW_SYNTAX_LANE_SCHEMA_ID
    {
        instrument.push("syntax lane schema mismatch".to_string());
    }
    if security.schema_id
        != crate::artifacts::workflow_security_lane_v1::WORKFLOW_SECURITY_LANE_SCHEMA_ID
    {
        instrument.push("security lane schema mismatch".to_string());
    }

    // Aggregate result: aggregate instrument failures dominate; then a
    // lane-level instrument failure dominates findings; findings stay
    // visible (open or exception-accepted) rather than collapsing to
    // clean; only two clean lanes are clean.
    let open_security_findings = security
        .findings
        .iter()
        .filter(|finding| finding.disposition == WorkflowSecurityDispositionV1::Open)
        .count() as u32;
    let accepted_security_findings = (security.findings.len() as u32) - open_security_findings;
    let lane_failure = syntax.result == WorkflowSyntaxLaneResultV1::InstrumentFailure
        || security.result == WorkflowSecurityLaneResultV1::InstrumentFailure;
    let lane_findings = syntax.result == WorkflowSyntaxLaneResultV1::Findings
        || security.result == WorkflowSecurityLaneResultV1::Findings;
    let result = if !instrument.is_empty() || lane_failure {
        WorkflowConstructionAggregateResultV1::InstrumentFailure
    } else if lane_findings {
        WorkflowConstructionAggregateResultV1::Findings
    } else {
        WorkflowConstructionAggregateResultV1::Clean
    };

    let mut limitations = Vec::new();
    if result == WorkflowConstructionAggregateResultV1::InstrumentFailure {
        limitations.extend(instrument.iter().cloned());
        // Propagate the failing lane's own reasons: the aggregate names
        // why each lane instrument-failed, not just that it did.
        if syntax.result == WorkflowSyntaxLaneResultV1::InstrumentFailure {
            limitations.extend(syntax.limitations.iter().cloned());
        }
        if security.result == WorkflowSecurityLaneResultV1::InstrumentFailure {
            limitations.extend(security.limitations.iter().cloned());
        }
        limitations.push(
            "aggregate instrument failure: the per-lane reports are retained above and in the lane artifacts".to_string(),
        );
    }
    limitations.push(
        "advisory: this aggregate does not gate; enforcement selection is the #2283/#2284 authority".to_string(),
    );

    WorkflowConstructionAggregateV1 {
        schema_id: WORKFLOW_CONSTRUCTION_AGGREGATE_SCHEMA_ID.to_string(),
        schema_version: WORKFLOW_CONSTRUCTION_AGGREGATE_SCHEMA_VERSION,
        denominator_digest: inventory.surfaces_digest.clone(),
        syntax_lane: WorkflowConstructionAggregateLaneV1 {
            lane: "syntax".to_string(),
            result: syntax.result.label().to_string(),
            finding_count: syntax.findings.len() as u32,
        },
        security_lane: WorkflowConstructionAggregateLaneV1 {
            lane: "security".to_string(),
            result: security.result.label().to_string(),
            finding_count: security.findings.len() as u32,
        },
        result,
        open_security_findings,
        accepted_security_findings,
        syntax_findings: syntax.findings.len() as u32,
        limitations,
        claim_boundary: AGGREGATE_CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of the aggregate.
#[must_use]
pub fn render_workflow_construction_aggregate_human(
    aggregate: &WorkflowConstructionAggregateV1,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "workflow-construction: result={} syntax={}/{} security={}/{} open-security={}",
        aggregate.result.label(),
        aggregate.syntax_lane.result,
        aggregate.syntax_findings,
        aggregate.security_lane.result,
        aggregate.security_lane.finding_count,
        aggregate.open_security_findings
    ));
    for limitation in &aggregate.limitations {
        lines.push(format!("  limitation: {limitation}"));
    }
    lines.push(format!("  claim boundary: {}", aggregate.claim_boundary));
    lines.join("\n")
}

/// JSON view of the aggregate.
///
/// # Errors
///
/// Returns the serialization error when the aggregate cannot be
/// rendered.
pub fn render_workflow_construction_aggregate_json(
    aggregate: &WorkflowConstructionAggregateV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(aggregate)
}
