use super::{PruneCandidate, PruneContext};
use crate::{markdown_cell, source_syntax_inventory_context};
use std::path::Path;

pub(super) fn render_prune_stale_result(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow prune\n\n");
    if write_requested {
        out.push_str("mode: write\n");
    } else {
        out.push_str("mode: dry-run\n");
    }
    if explicit_dry_run {
        out.push_str("requested: --dry-run\n");
    }
    out.push_str(&format!("stale entries: {}\n\n", candidates.len()));
    if candidates.is_empty() {
        out.push_str("No stale allow entries found.\n");
        return out;
    }
    out.push_str("| Allow ID | Kind | Family | Owner | Classification | Scope | Reason |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for candidate in candidates {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(&candidate.id),
            candidate.kind,
            markdown_cell(candidate.family.as_deref().unwrap_or("-")),
            markdown_cell(&candidate.owner),
            markdown_cell(&candidate.classification),
            markdown_cell(&candidate.scope),
            markdown_cell(&candidate.reason)
        ));
    }
    if let Some(path) = written_path {
        out.push_str(&format!(
            "\nRemoved stale entries from `{}`.\n",
            markdown_cell(&path.display().to_string())
        ));
    } else {
        out.push_str(
            "\nNo files were changed. Remove these entries only after confirming the exception is gone.\n",
        );
    }
    out
}

pub(super) fn render_prune_stale_json(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    context: PruneContext<'_>,
) -> String {
    let written = written_path.map(|path| path.display().to_string());
    let report_candidates = candidates
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
        .collect::<Vec<_>>();
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
