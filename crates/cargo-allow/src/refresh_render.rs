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
