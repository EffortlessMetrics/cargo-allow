use super::{PruneCandidate, PruneContext};
use crate::source_syntax_inventory_context;
use std::path::Path;

pub(super) fn render_prune_stale_result(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
) -> String {
    let written = written_path.map(|path| path.display().to_string());
    let report_candidates = report_prune_candidates(candidates);
    allow_report::render_prune_human(
        &report_candidates,
        allow_report::PruneModeContext {
            explicit_dry_run,
            write_requested,
            written_path: written.as_deref(),
        },
    )
}

pub(super) fn render_prune_stale_json(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    context: PruneContext<'_>,
) -> String {
    let written = written_path.map(|path| path.display().to_string());
    let report_candidates = report_prune_candidates(candidates);
    allow_report::render_prune_json(
        &report_candidates,
        allow_report::PruneModeContext {
            explicit_dry_run,
            write_requested,
            written_path: written.as_deref(),
        },
        source_syntax_inventory_context(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
    )
}

fn report_prune_candidates(candidates: &[PruneCandidate]) -> Vec<allow_report::PruneCandidate<'_>> {
    candidates
        .iter()
        .map(|candidate| allow_report::PruneCandidate {
            id: &candidate.id,
            kind: candidate.kind.as_str(),
            family: candidate.family.as_deref(),
            owner: &candidate.owner,
            classification: &candidate.classification,
            scope: &candidate.scope,
            reason: &candidate.reason,
        })
        .collect()
}
