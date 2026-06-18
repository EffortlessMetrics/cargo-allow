use allow_core::{AllowEntry, Finding, LastSeen};
use allow_report::{
    RefreshModeContext, RefreshReport, render_refresh_human,
    render_refresh_json as render_refresh_artifact_json,
};

use super::RefreshContext;

pub(super) fn render_refresh_result(
    entry: &AllowEntry,
    finding: &Finding,
    previous_last_seen: Option<LastSeen>,
    drift_message: &str,
    dry_run: bool,
    write_requested: bool,
    written_path: Option<&str>,
    context: RefreshContext<'_>,
) -> String {
    let mode = RefreshModeContext {
        explicit_dry_run: dry_run,
        write_requested,
        written_path,
    };
    let report = RefreshReport::new(
        context.inventory,
        entry,
        finding,
        previous_last_seen,
        drift_message,
        mode,
    );
    render_refresh_human(report)
}

pub(super) fn render_refresh_json(
    entry: &AllowEntry,
    finding: &Finding,
    previous_last_seen: Option<LastSeen>,
    drift_message: &str,
    dry_run: bool,
    write_requested: bool,
    written_path: Option<&str>,
    context: RefreshContext<'_>,
) -> String {
    let mode = RefreshModeContext {
        explicit_dry_run: dry_run,
        write_requested,
        written_path,
    };
    let report = RefreshReport::new(
        context.inventory,
        entry,
        finding,
        previous_last_seen,
        drift_message,
        mode,
    );
    render_refresh_artifact_json(report)
}
