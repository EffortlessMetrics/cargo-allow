use allow_report::{
    RefreshModeContext, RefreshReport, render_refresh_human,
    render_refresh_json as render_refresh_artifact_json,
};

use super::RefreshRenderInput;

pub(super) fn render_refresh_result(input: RefreshRenderInput<'_>) -> String {
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
    );
    render_refresh_human(report)
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
    );
    render_refresh_artifact_json(report)
}
