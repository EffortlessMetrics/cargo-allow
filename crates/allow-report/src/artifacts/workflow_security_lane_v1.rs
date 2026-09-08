//! Qualified security construction lane over the workflow
//! construction denominator (#3907 PR C): the qualified zizmor
//! release runs offline over every workflow and local action, its raw
//! findings keep their native rule identity, and exact reviewed
//! exceptions reduce the advisory signal without deleting or
//! broadening anything.
//!
//! Advisory: findings — open or exception-accepted — are evidence,
//! not blockers. Enforcement selection is the aggregate lane's
//! authority (#3907 PR D).
//!
//! Laws:
//! - the tool identity must match the inventory's zizmor selection;
//!   version or role drift is an instrument failure;
//! - every raw finding must name an inventoried surface; out-of-scope
//!   findings are instrument failures;
//! - native rule identity (zizmor's `ident`) is preserved alongside
//!   the construction-family mapping; unmapped idents land in the
//!   unsupported family with a named reason;
//! - an exception applies only to its exact (path, rule) pair and
//!   records who reviewed it; a broad suppression cannot exist;
//! - a completed offline run is `clean` only with zero findings;
//!   findings — open or exception-accepted — keep the `findings`
//!   result so the signal stays visible until the corpus is accepted.

use serde::{Deserialize, Serialize};

use crate::artifacts::workflow_construction_v1::{
    WorkflowConstructionFamilyV1, WorkflowConstructionInventoryV1,
    WorkflowConstructionSurfaceKindV1,
};

pub const WORKFLOW_SECURITY_LANE_SCHEMA_ID: &str = "cargo-allow.workflow-security-lane.v1";
pub const WORKFLOW_SECURITY_LANE_SCHEMA_VERSION: u32 = 1;

const WORKFLOW_SECURITY_CLAIM_BOUNDARY: &str = "Qualified security construction lane over the workflow construction denominator: the qualified zizmor release runs offline with native rule identity preserved, exact reviewed exceptions reduce the advisory signal without broadening anything, and findings stay visible as evidence. Advisory until the aggregate lane selects enforcement; no workflow is mutated and no network lookup enters ordinary cargo-allow scans.";

/// One raw finding flattened from the tool's JSON output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSecurityRawFindingV1 {
    pub ident: String,
    pub desc: String,
    pub confidence: String,
    pub severity: String,
    pub persona: String,
    pub path: String,
    /// The tool's own location annotation (job/step identity).
    pub annotation: String,
    /// One-based source line of the finding's primary location.
    pub line: u32,
}

/// The tool run the lane grades.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSecurityToolRunV1 {
    pub tool: String,
    pub version: String,
    /// The offline-audits posture the findings were produced under.
    pub offline_mode: bool,
    pub covered: Vec<String>,
    pub raw_findings: Vec<WorkflowSecurityRawFindingV1>,
}

/// One exact, reviewed exception: (path, rule) granularity with the
/// review identity attached. A broad suppression cannot be expressed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSecurityExceptionV1 {
    pub path: String,
    /// The tool's native rule identity (e.g. zizmor's `artipacked`).
    pub rule: String,
    pub owner: String,
    pub reason: String,
    pub evidence: Vec<String>,
    /// ISO date after which the exception must be re-reviewed.
    pub review_after: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSecurityDispositionV1 {
    /// No exception applies; the finding is open advisory signal.
    Open,
    /// An exact reviewed exception covers this finding.
    ExceptionAccepted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSecurityFindingV1 {
    pub path: String,
    /// The tool's native rule identity, preserved verbatim.
    pub rule: String,
    pub desc: String,
    pub severity: String,
    pub confidence: String,
    pub family: WorkflowConstructionFamilyV1,
    pub annotation: String,
    pub line: u32,
    pub disposition: WorkflowSecurityDispositionV1,
    pub exception_owner: Option<String>,
}

impl WorkflowSecurityDispositionV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::ExceptionAccepted => "exception_accepted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSecurityLaneResultV1 {
    Clean,
    Findings,
    InstrumentFailure,
}

impl WorkflowSecurityLaneResultV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Findings => "findings",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSecurityLaneReportV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub tool: String,
    pub version: String,
    /// The denominator content digest the lane graded against.
    pub denominator_digest: String,
    pub covered: Vec<String>,
    pub findings: Vec<WorkflowSecurityFindingV1>,
    pub result: WorkflowSecurityLaneResultV1,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Map a native rule identity to its construction family. Unknown
/// idents land in the unsupported family so nothing is silently
/// dropped.
#[must_use]
pub fn workflow_security_rule_family(ident: &str) -> WorkflowConstructionFamilyV1 {
    match ident {
        "template-injection" => WorkflowConstructionFamilyV1::UntrustedContextShellInterpolation,
        "dangerous-triggers" => WorkflowConstructionFamilyV1::PrivilegedUntrustedEvent,
        "artipacked" => WorkflowConstructionFamilyV1::CredentialPersistence,
        "unpinned-uses" => WorkflowConstructionFamilyV1::MutableOrUnresolvedActionRef,
        "excessive-permissions" => WorkflowConstructionFamilyV1::PermissionExcessOrAmbiguity,
        "self-repository" => WorkflowConstructionFamilyV1::UnsafeCheckoutOrRefSelection,
        "superfluous-actions" => WorkflowConstructionFamilyV1::NestedLocalActionUninspected,
        _ => WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure,
    }
}

/// Does an exact exception cover this finding? Granularity is exactly
/// (path, rule); nothing else participates.
fn exception_applies<'a>(
    finding_path: &str,
    finding_rule: &str,
    exceptions: &'a [WorkflowSecurityExceptionV1],
    today: &str,
) -> Option<&'a WorkflowSecurityExceptionV1> {
    // An expired exception stops applying: the finding returns to open
    // advisory signal and must be re-reviewed to be suppressed again.
    // Dates are ISO (YYYY-MM-DD), so lexicographic order is date order.
    exceptions.iter().find(|exception| {
        exception.path == finding_path
            && exception.rule == finding_rule
            && exception.review_after.as_str() >= today
    })
}

/// Grade one validated tool run against the workflow construction
/// inventory with the reviewed exception set. Pure and fail-closed.
#[must_use]
pub fn evaluate_workflow_security_run(
    tool_run: &WorkflowSecurityToolRunV1,
    inventory: &WorkflowConstructionInventoryV1,
    exceptions: &[WorkflowSecurityExceptionV1],
    today: &str,
) -> WorkflowSecurityLaneReportV1 {
    let mut instrument: Vec<String> = Vec::new();

    if tool_run.tool != "zizmor" {
        // The qualified security analyzer for this lane is zizmor; a
        // run from any other tool is not this lane's evidence even if
        // some inventory entry shares its version.
        instrument.push(format!(
            "tool {} is not the qualified security analyzer (zizmor)",
            tool_run.tool
        ));
    }
    if !tool_run.offline_mode {
        // An online run's evidence includes network-sourced audits the
        // lane cannot account for; it is never this lane's offline
        // evidence.
        instrument.push(
            "tool run was not produced in offline mode; online audits are outside the lane's evidence".to_string(),
        );
    }
    let selection = inventory
        .tool_selections
        .iter()
        .find(|tool| tool.tool == tool_run.tool);
    match selection {
        None => instrument.push(format!(
            "tool {} is not in the inventory's tool selections",
            tool_run.tool
        )),
        Some(selection) => {
            if selection.status
                == crate::artifacts::workflow_construction_v1::WorkflowSecurityToolStatusV1::CandidatePendingQualification
            {
                instrument.push(format!(
                    "tool {} is still a candidate pending fixture qualification",
                    tool_run.tool
                ));
            }
            if selection.version.as_deref() != Some(tool_run.version.as_str()) {
                instrument.push(format!(
                    "tool version drifted: run {} vs selected {}",
                    tool_run.version,
                    selection.version.as_deref().unwrap_or("<none>")
                ));
            }
        }
    }

    // Denominator law: the run's covered set must be exactly the
    // inventoried workflows and local actions (this lane inspects both;
    // checked examples stay with the syntax lane).
    let mut expected: Vec<String> = inventory
        .surfaces
        .iter()
        .filter(|surface| {
            matches!(
                surface.kind,
                WorkflowConstructionSurfaceKindV1::Workflow
                    | WorkflowConstructionSurfaceKindV1::LocalAction
            )
        })
        .map(|surface| surface.path.clone())
        .collect();
    expected.sort();
    let mut run_covered = tool_run.covered.clone();
    run_covered.sort();
    if run_covered != expected {
        instrument.push(format!(
            "covered surfaces do not match the inventoried denominator: run {run_covered:?} vs inventory {expected:?}"
        ));
    }

    let covered_set: std::collections::BTreeSet<&str> =
        tool_run.covered.iter().map(String::as_str).collect();
    let mut findings = Vec::new();
    for raw in &tool_run.raw_findings {
        if !covered_set.contains(raw.path.as_str()) {
            instrument.push(format!("raw finding names out-of-scope path {}", raw.path));
            continue;
        }
        let family = workflow_security_rule_family(&raw.ident);
        let applying = exception_applies(&raw.path, &raw.ident, exceptions, today);
        let (disposition, exception_owner) = if let Some(exception) = applying {
            (
                WorkflowSecurityDispositionV1::ExceptionAccepted,
                Some(exception.owner.clone()),
            )
        } else {
            (WorkflowSecurityDispositionV1::Open, None)
        };
        findings.push(WorkflowSecurityFindingV1 {
            path: raw.path.clone(),
            rule: raw.ident.clone(),
            desc: raw.desc.clone(),
            severity: raw.severity.clone(),
            confidence: raw.confidence.clone(),
            family,
            annotation: raw.annotation.clone(),
            line: raw.line,
            disposition,
            exception_owner,
        });
    }

    let result = if !instrument.is_empty() {
        WorkflowSecurityLaneResultV1::InstrumentFailure
    } else if findings.is_empty() {
        WorkflowSecurityLaneResultV1::Clean
    } else {
        WorkflowSecurityLaneResultV1::Findings
    };

    let mut limitations = vec![
        "offline mode: known-vulnerability audits requiring network access are not part of this evidence".to_string(),
        "the exception granularity is exactly (path, rule); step-level narrowing arrives with corpus acceptance".to_string(),
    ];
    if result == WorkflowSecurityLaneResultV1::InstrumentFailure {
        limitations.push(format!("instrument failures: {}", instrument.join("; ")));
    }

    WorkflowSecurityLaneReportV1 {
        schema_id: WORKFLOW_SECURITY_LANE_SCHEMA_ID.to_string(),
        schema_version: WORKFLOW_SECURITY_LANE_SCHEMA_VERSION,
        tool: tool_run.tool.clone(),
        version: tool_run.version.clone(),
        denominator_digest: inventory.surfaces_digest.clone(),
        covered: run_covered,
        findings,
        result,
        limitations,
        claim_boundary: WORKFLOW_SECURITY_CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of one security lane report.
#[must_use]
pub fn render_workflow_security_human(report: &WorkflowSecurityLaneReportV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "workflow-security: tool={} version={} result={} findings={} open={}",
        report.tool,
        report.version,
        report.result.label(),
        report.findings.len(),
        report
            .findings
            .iter()
            .filter(|finding| finding.disposition == WorkflowSecurityDispositionV1::Open)
            .count()
    ));
    for finding in &report.findings {
        lines.push(format!(
            "  {}:{} [{}] {} ({}): {}",
            finding.path,
            finding.line,
            finding.rule,
            finding.family.as_str(),
            finding.disposition.label(),
            finding.annotation
        ));
    }
    for limitation in &report.limitations {
        lines.push(format!("  limitation: {limitation}"));
    }
    lines.push(format!("  claim boundary: {}", report.claim_boundary));
    lines.join("\n")
}

/// JSON view of one security lane report.
///
/// # Errors
///
/// Returns the serialization error when the report cannot be rendered.
pub fn render_workflow_security_json(
    report: &WorkflowSecurityLaneReportV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}
