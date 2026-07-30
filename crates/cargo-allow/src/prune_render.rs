use super::{PruneCandidate, PruneContext, PruneRenderMode};
use allow_report::Style;
use std::path::Path;

#[cfg(test)]
pub(super) fn render_prune_stale_result(
    candidates: &[PruneCandidate],
    removed_toml_blocks: &[String],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    total_entries: usize,
    context: PruneContext<'_>,
) -> String {
    render_prune_stale_result_with_options(
        candidates,
        removed_toml_blocks,
        PruneRenderOptions {
            explicit_dry_run,
            write_requested,
            written_path,
            total_entries,
            style: Style::PLAIN,
        },
        context,
    )
}

pub(super) struct PruneRenderOptions<'a> {
    pub(super) explicit_dry_run: bool,
    pub(super) write_requested: bool,
    pub(super) written_path: Option<&'a Path>,
    pub(super) total_entries: usize,
    pub(super) style: Style,
}

pub(super) fn render_prune_stale_result_with_options(
    candidates: &[PruneCandidate],
    removed_toml_blocks: &[String],
    options: PruneRenderOptions<'_>,
    context: PruneContext<'_>,
) -> String {
    let mode = PruneRenderMode::new(
        options.explicit_dry_run,
        options.write_requested,
        options.written_path,
        options.total_entries,
    );
    let report_candidates = report_prune_candidates(candidates);
    let text = allow_report::render_prune_human_with_context_styled(
        &report_candidates,
        mode.context(),
        context.inventory,
        options.style,
    );
    insert_removed_toml_preview(text, removed_toml_blocks)
}

pub(super) fn render_prune_stale_json(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    total_entries: usize,
    context: PruneContext<'_>,
) -> String {
    let mode = PruneRenderMode::new(
        explicit_dry_run,
        write_requested,
        written_path,
        total_entries,
    );
    let report_candidates = report_prune_candidates(candidates);
    allow_report::render_prune_json(
        &report_candidates,
        mode.context(),
        context.inventory,
        &context.mutation_receipt,
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

fn insert_removed_toml_preview(mut text: String, removed_toml_blocks: &[String]) -> String {
    if removed_toml_blocks.is_empty() {
        return text;
    }
    let mut preview = String::new();
    preview.push_str("\nTOML removal preview:\n\n");
    preview.push_str("```diff\n");
    for block in removed_toml_blocks {
        for line in block.lines() {
            preview.push_str("- ");
            preview.push_str(line);
            preview.push('\n');
        }
    }
    preview.push_str("```\n");

    if let Some(index) = text.find("\nClaim boundary:") {
        text.insert_str(index, &preview);
    } else {
        text.push_str(&preview);
    }
    text
}
