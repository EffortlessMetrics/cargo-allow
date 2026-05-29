use super::{PruneCandidate, PruneContext, PruneRenderMode};
use std::path::Path;

pub(super) fn render_prune_stale_result(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
) -> String {
    let mode = PruneRenderMode::new(explicit_dry_run, write_requested, written_path);
    let report_candidates = report_prune_candidates(candidates);
    allow_report::render_prune_human(&report_candidates, mode.context())
}

pub(super) fn render_prune_stale_json(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    context: PruneContext<'_>,
) -> String {
    let mode = PruneRenderMode::new(explicit_dry_run, write_requested, written_path);
    let report_candidates = report_prune_candidates(candidates);
    allow_report::render_prune_json(&report_candidates, mode.context(), context.inventory)
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
