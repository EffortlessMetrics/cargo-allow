//! Spec-system render adapter (#3522 slice C).
//!
//! Rendering now lives in `allow_report::spec_system_render` over the
//! allow-report spec-system report DTOs. This file keeps the cargo-internal call
//! shape (CLI `OutputFormat` in, rendered text out) and maps onto the
//! neutral renderer format; output bytes are unchanged.

use super::*;
use allow_report::SpecSystemRenderFormat;

fn map_output_format(format: OutputFormat) -> SpecSystemRenderFormat {
    match format {
        OutputFormat::Json => SpecSystemRenderFormat::Json,
        OutputFormat::Html => SpecSystemRenderFormat::Html,
        OutputFormat::Sarif => SpecSystemRenderFormat::Sarif,
        OutputFormat::Human => SpecSystemRenderFormat::Human,
        OutputFormat::Markdown => SpecSystemRenderFormat::Markdown,
    }
}

pub(super) fn render_spec_system_report(report: &SpecSystemReport, format: OutputFormat) -> String {
    allow_report::render_spec_system_report(report, map_output_format(format))
}

pub(super) fn filter_spec_system_report_for_artifact(
    report: &SpecSystemReport,
    artifact_id: &str,
) -> CargoAllowResult<SpecSystemReport> {
    allow_report::filter_spec_system_report_for_artifact(report, artifact_id)
}

pub(super) fn render_spec_system_explain_markdown(report: &SpecSystemReport) -> String {
    allow_report::render_spec_system_explain_markdown(report)
}

pub(super) fn render_spec_system_markdown(report: &SpecSystemReport) -> String {
    allow_report::render_spec_system_markdown(report)
}

pub(super) fn render_spec_system_json(report: &SpecSystemReport) -> String {
    allow_report::render_spec_system_json(report)
}
