//! Pinned syntax/expression lane over the workflow construction
//! denominator (#3907 PR B): the pinned actionlint release runs over
//! every current workflow and checked example, and its raw findings
//! are validated, family-mapped, and graded into one typed lane
//! report.
//!
//! Laws:
//! - the tool identity (name, version, pin digest) must match the
//!   inventory's selected syntax analyzer; a drifted pin is an
//!   instrument failure, not a pass;
//! - every covered surface must be an inventoried workflow or checked
//!   example, and every inventoried action must be accounted for in
//!   the lane's uncovered set with its reason;
//! - every raw finding must name a covered surface; an out-of-scope
//!   finding is an instrument failure;
//! - raw kinds map to construction families — `expression` and
//!   `syntax-check` are syntax findings, and any other kind lands in
//!   `unsupported_or_instrument_failure` so nothing is silently
//!   dropped;
//! - a clean run is `clean` only when the tool ran over the whole
//!   covered set; missing, malformed, or partial analyses stay
//!   `instrument_failure`.

use serde::{Deserialize, Serialize};

use crate::artifacts::workflow_construction_v1::{
    WorkflowConstructionFamilyV1, WorkflowConstructionInventoryV1,
    WorkflowConstructionSurfaceKindV1,
};

pub const WORKFLOW_SYNTAX_LANE_SCHEMA_ID: &str = "cargo-allow.workflow-syntax-lane.v1";
pub const WORKFLOW_SYNTAX_LANE_SCHEMA_VERSION: u32 = 1;

const WORKFLOW_SYNTAX_CLAIM_BOUNDARY: &str = "Pinned syntax/expression lane over the workflow construction denominator: the pinned actionlint release grades every current workflow and checked example with its exact pin identity, findings keep their native kind and source location mapped into construction families, and missing or partial analyses are instrument failures. Local composite action internals are outside this lane's pinned configuration and stay with the security lane; enforcement selection arrives with the aggregate lane.";

/// One raw finding exactly as the pinned tool emitted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSyntaxRawFindingV1 {
    pub message: String,
    pub filepath: String,
    pub line: u32,
    pub column: u32,
    pub kind: String,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub end_column: Option<u32>,
}

/// The tool run the lane grades: exact identity, arguments, and the
/// denominator split the tool actually executed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSyntaxToolRunV1 {
    pub tool: String,
    pub version: String,
    pub pin_identity: String,
    pub arguments: Vec<String>,
    /// Surfaces the tool executed over.
    pub covered: Vec<String>,
    /// Surfaces in the denominator this lane's pinned configuration
    /// does not inspect, with the reason recorded per path.
    pub uncovered: Vec<WorkflowSyntaxUncoveredSurfaceV1>,
    pub raw_findings: Vec<WorkflowSyntaxRawFindingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSyntaxUncoveredSurfaceV1 {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSyntaxLaneResultV1 {
    Clean,
    Findings,
    InstrumentFailure,
}

impl WorkflowSyntaxLaneResultV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Findings => "findings",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// One source-located syntax finding mapped into the construction
/// vocabulary. The native tool kind is preserved alongside the family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSyntaxFindingV1 {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub kind: String,
    pub family: WorkflowConstructionFamilyV1,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSyntaxLaneReportV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub tool: String,
    pub version: String,
    pub pin_identity: String,
    /// The inventory content digest the lane graded against.
    pub denominator_digest: String,
    pub covered: Vec<String>,
    pub uncovered: Vec<WorkflowSyntaxUncoveredSurfaceV1>,
    pub findings: Vec<WorkflowSyntaxFindingV1>,
    pub result: WorkflowSyntaxLaneResultV1,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Denominator law: covered paths are exactly the inventoried
/// workflows and checked examples; uncovered paths are exactly the
/// inventoried local actions.
fn check_denominator_laws(
    tool_run: &WorkflowSyntaxToolRunV1,
    inventory: &WorkflowConstructionInventoryV1,
    run_covered: &[String],
    instrument: &mut Vec<String>,
) {
    let mut expected_covered: Vec<String> = inventory
        .surfaces
        .iter()
        .filter(|surface| {
            matches!(
                surface.kind,
                WorkflowConstructionSurfaceKindV1::Workflow
                    | WorkflowConstructionSurfaceKindV1::CheckedExample
            )
        })
        .map(|surface| surface.path.clone())
        .collect();
    expected_covered.sort();
    if run_covered != expected_covered {
        instrument.push(format!(
            "covered surfaces do not match the inventoried denominator: run {run_covered:?} vs inventory {expected_covered:?}"
        ));
    }
    let action_paths: std::collections::BTreeSet<&str> = inventory
        .surfaces
        .iter()
        .filter(|surface| surface.kind == WorkflowConstructionSurfaceKindV1::LocalAction)
        .map(|surface| surface.path.as_str())
        .collect();
    let uncovered_paths: std::collections::BTreeSet<&str> = tool_run
        .uncovered
        .iter()
        .map(|surface| surface.path.as_str())
        .collect();
    if uncovered_paths != action_paths {
        instrument.push(format!(
            "uncovered surfaces do not match the inventoried local actions: run {uncovered_paths:?} vs inventory {action_paths:?}"
        ));
    }
}

/// Family mapping with kind preservation; unknown kinds land in the
/// unsupported family so nothing is silently dropped, and findings
/// outside the covered set are instrument failures.
fn map_raw_findings(
    tool_run: &WorkflowSyntaxToolRunV1,
    covered_set: &std::collections::BTreeSet<&str>,
    instrument: &mut Vec<String>,
) -> Vec<WorkflowSyntaxFindingV1> {
    let mut findings = Vec::new();
    for raw in &tool_run.raw_findings {
        if !covered_set.contains(raw.filepath.as_str()) {
            instrument.push(format!(
                "raw finding names out-of-scope path {}",
                raw.filepath
            ));
            continue;
        }
        let family = match raw.kind.as_str() {
            "expression" | "syntax-check" => {
                WorkflowConstructionFamilyV1::SyntaxOrExpressionInvalid
            }
            other => {
                instrument.push(format!(
                    "raw finding kind {other} has no family mapping and lands in the unsupported family"
                ));
                WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure
            }
        };
        findings.push(WorkflowSyntaxFindingV1 {
            path: raw.filepath.clone(),
            line: raw.line,
            column: raw.column,
            kind: raw.kind.clone(),
            family,
            message: raw.message.clone(),
        });
    }
    findings
}

/// Grade one validated tool run against the workflow construction
/// inventory. Pure and fail-closed: identity drift, denominator
/// mismatch, out-of-scope findings, and unknown raw kinds are all
/// instrument failures or explicit findings — never silence.
#[must_use]
pub fn evaluate_workflow_syntax_run(
    tool_run: &WorkflowSyntaxToolRunV1,
    inventory: &WorkflowConstructionInventoryV1,
) -> WorkflowSyntaxLaneReportV1 {
    let mut instrument: Vec<String> = Vec::new();

    // The lane's pin must match the inventory's selected syntax
    // analyzer: version and digest move together.
    let selection = inventory
        .tool_selections
        .iter()
        .find(|tool| tool.tool == tool_run.tool);
    match selection {
        None => instrument.push(format!(
            "tool {} is not the inventory's selected syntax analyzer",
            tool_run.tool
        )),
        Some(selection) => {
            if selection.version.as_deref() != Some(tool_run.version.as_str()) {
                instrument.push(format!(
                    "tool version drifted: run {} vs selected {}",
                    tool_run.version,
                    selection.version.as_deref().unwrap_or("<none>")
                ));
            }
            if selection.pin_identity.as_deref() != Some(tool_run.pin_identity.as_str()) {
                instrument.push(format!(
                    "tool pin drifted: run {} vs selected {}",
                    tool_run.pin_identity,
                    selection.pin_identity.as_deref().unwrap_or("<none>")
                ));
            }
        }
    }

    let mut run_covered = tool_run.covered.clone();
    run_covered.sort();
    check_denominator_laws(tool_run, inventory, &run_covered, &mut instrument);
    let covered_set: std::collections::BTreeSet<&str> =
        tool_run.covered.iter().map(String::as_str).collect();

    let findings = map_raw_findings(tool_run, &covered_set, &mut instrument);

    let result = if !instrument.is_empty() {
        WorkflowSyntaxLaneResultV1::InstrumentFailure
    } else if findings.is_empty() {
        WorkflowSyntaxLaneResultV1::Clean
    } else {
        WorkflowSyntaxLaneResultV1::Findings
    };

    let mut limitations = vec![
        "the pinned configuration runs with the shellcheck layer disabled, so run-script construction inside workflows is outside this lane's evidence (shell_or_command_construction stays with the security lane and review)".to_string(),
        "local composite action manifests are inventoried but not inspected by this lane's pinned configuration (nested_local_action_uninspected); their construction evidence arrives with the security lane".to_string(),
        "the precise runner-label ignore carried by the pregate ('label \"macos-15-intel\" is unknown') applies to this lane as well".to_string(),
    ];
    if result == WorkflowSyntaxLaneResultV1::InstrumentFailure {
        limitations.push(format!("instrument failures: {}", instrument.join("; ")));
    }

    WorkflowSyntaxLaneReportV1 {
        schema_id: WORKFLOW_SYNTAX_LANE_SCHEMA_ID.to_string(),
        schema_version: WORKFLOW_SYNTAX_LANE_SCHEMA_VERSION,
        tool: tool_run.tool.clone(),
        version: tool_run.version.clone(),
        pin_identity: tool_run.pin_identity.clone(),
        denominator_digest: inventory.surfaces_digest.clone(),
        covered: run_covered,
        uncovered: tool_run.uncovered.clone(),
        findings,
        result,
        limitations,
        claim_boundary: WORKFLOW_SYNTAX_CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of one syntax lane report.
#[must_use]
pub fn render_workflow_syntax_human(report: &WorkflowSyntaxLaneReportV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "workflow-syntax: tool={} version={} result={} findings={}",
        report.tool,
        report.version,
        report.result.label(),
        report.findings.len()
    ));
    for finding in &report.findings {
        lines.push(format!(
            "  {}:{}:{} [{}] {}: {}",
            finding.path,
            finding.line,
            finding.column,
            finding.kind,
            finding.family.as_str(),
            finding.message
        ));
    }
    for limitation in &report.limitations {
        lines.push(format!("  limitation: {limitation}"));
    }
    lines.push(format!("  claim boundary: {}", report.claim_boundary));
    lines.join("\n")
}

/// JSON view of one syntax lane report.
///
/// # Errors
///
/// Returns the serialization error when the report cannot be rendered.
pub fn render_workflow_syntax_json(
    report: &WorkflowSyntaxLaneReportV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}
