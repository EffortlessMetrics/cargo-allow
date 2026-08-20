//! Dry-run command wired to proof-engine (#2589-B).

use std::path::Path;

use proof_engine::{DRY_RUN_PLAN_REPORT_SCHEMA_ID, dry_run_plan_artifact};

use crate::render::{DryRunFrameV1, OutputFormat, emit_frame};

pub const DRY_RUN_FRAME_SCHEMA_ID: &str = "cargo-proof.dry-run-frame.v1";
pub const DRY_RUN_CLAIM_BOUNDARY: &str =
    "Structured argv projection only; issue/spec prose must never become executable shell.";

pub fn dry_run_from_plan_path(path: &Path) -> Result<proof_engine::DryRunPlanReportV1, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    dry_run_plan_artifact(&text)
}

pub fn render_dry_run_frame(
    report: &proof_engine::DryRunPlanReportV1,
    format: OutputFormat,
) -> Result<String, String> {
    let structured_lines: Vec<String> = report
        .lines
        .iter()
        .map(|line| line.structured_argv.clone())
        .collect();
    let frame = DryRunFrameV1 {
        schema_id: DRY_RUN_FRAME_SCHEMA_ID.to_string(),
        plan_id: report.plan_id.clone(),
        line_count: report.lines.len(),
        structured_lines,
        claim_boundary: DRY_RUN_CLAIM_BOUNDARY.to_string(),
    };
    let rendered = emit_frame(&frame, format)?;
    if format == OutputFormat::Json {
        return Ok(rendered);
    }
    let mut output = rendered;
    output.push_str(&format!("engine_schema: {DRY_RUN_PLAN_REPORT_SCHEMA_ID}\n"));
    for line in &report.lines {
        output.push_str(&line.structured_argv);
        output.push('\n');
    }
    Ok(output)
}
