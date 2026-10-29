use allow_report::{
    RefreshModeContext, RefreshReport, Style, render_refresh_human_styled,
    render_refresh_json as render_refresh_artifact_json,
};

use super::RefreshRenderInput;

pub(super) fn render_refresh_result_styled(input: RefreshRenderInput<'_>, style: Style) -> String {
    let mode = RefreshModeContext {
        explicit_dry_run: input.dry_run,
        write_requested: input.write_requested,
        written_path: input.written_path,
    };
    let report = RefreshReport::new(
        input.context.inventory,
        input.entry,
        input.finding,
        input.previous_last_seen,
        input.drift_message,
        mode,
        input.mutation_receipt,
    );
    render_refresh_human_styled(report, style)
}

pub(super) fn render_refresh_json(input: RefreshRenderInput<'_>) -> String {
    let mode = RefreshModeContext {
        explicit_dry_run: input.dry_run,
        write_requested: input.write_requested,
        written_path: input.written_path,
    };
    let report = RefreshReport::new(
        input.context.inventory,
        input.entry,
        input.finding,
        input.previous_last_seen,
        input.drift_message,
        mode,
        input.mutation_receipt,
    );
    render_refresh_artifact_json(report)
}

/// Join the per-entry refresh JSON artifacts of a bulk `--all` run into one
/// parseable JSON array (#4651). Each element is the documented
/// `cargo-allow.refresh.v1` document, re-indented one level; no artifact
/// fields change, so single-entry and per-element output stay identical.
pub(super) fn render_bulk_refresh_json(documents: &[String]) -> String {
    let mut out = String::from("[\n");
    for (index, document) in documents.iter().enumerate() {
        let last_document = index + 1 == documents.len();
        let mut lines = document.lines().peekable();
        while let Some(line) = lines.next() {
            out.push_str("  ");
            out.push_str(line.trim_end());
            if lines.peek().is_none() && !last_document {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("]\n");
    out
}
