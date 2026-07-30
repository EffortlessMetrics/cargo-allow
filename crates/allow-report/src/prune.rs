use crate::Style;
use crate::allow_entry_json::push_optional_string_field;
use crate::contracts::PRUNE_ARTIFACT;
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::text::markdown_cell;
use crate::{
    CLAIM_BOUNDARY_TEXT, InventoryContext, MutationReceipt, PruneCandidate, PruneModeContext,
};
use allow_core::json_escape;

pub fn render_prune_human(candidates: &[PruneCandidate<'_>], mode: PruneModeContext<'_>) -> String {
    render_prune_human_with_context(candidates, mode, InventoryContext::unknown_source_syntax())
}

pub fn render_prune_human_with_context(
    candidates: &[PruneCandidate<'_>],
    mode: PruneModeContext<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    render_prune_human_with_context_styled(candidates, mode, inventory, Style::PLAIN)
}

pub fn render_prune_human_with_context_styled(
    candidates: &[PruneCandidate<'_>],
    mode: PruneModeContext<'_>,
    inventory: InventoryContext<'_>,
    style: Style,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow prune\n\n");
    out.push_str(&format!(
        "Inventory: {}/{} via {}{}\n",
        inventory.scope,
        inventory.scanner,
        inventory.source,
        inventory.files_scanned_suffix()
    ));
    if let Some(root) = inventory.root {
        out.push_str(&format!("Source tree root: {root}\n"));
    }
    if mode.write_requested {
        out.push_str("mode: write\n");
    } else {
        out.push_str("mode: dry-run\n");
    }
    if mode.explicit_dry_run {
        out.push_str("requested: --dry-run\n");
    }
    out.push_str(&style.status("stale", "stale"));
    out.push_str(&format!(" entries: {}\n\n", candidates.len()));
    if candidates.is_empty() {
        out.push_str("No ");
        out.push_str(&style.status("stale", "stale"));
        out.push_str(" allow entries found.\n");
        out.push('\n');
        out.push_str(CLAIM_BOUNDARY_TEXT);
        out.push('\n');
        return out;
    }
    out.push_str("| Allow ID | Kind | Family | Owner | Classification | Scope | Reason |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for candidate in candidates {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(candidate.id),
            candidate.kind,
            markdown_cell(candidate.family.unwrap_or("-")),
            markdown_cell(candidate.owner),
            markdown_cell(candidate.classification),
            markdown_cell(candidate.scope),
            markdown_cell(candidate.reason)
        ));
    }
    if let Some(path) = mode.written_path {
        let remaining = mode.total_entries.saturating_sub(candidates.len());
        out.push_str(&format!("\nRemoved {} ", candidates.len()));
        out.push_str(&style.status("stale", "stale"));
        out.push_str(&format!(
            " entr{} from `{}`. {} entr{} remain.\n",
            if candidates.len() == 1 { "y" } else { "ies" },
            markdown_cell(path),
            remaining,
            if remaining == 1 { "y" } else { "ies" },
        ));
    } else {
        out.push_str(
            "\nNo files were changed. Remove these entries only after confirming the exception is gone.\n",
        );
    }
    out.push('\n');
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

pub fn render_prune_json(
    candidates: &[PruneCandidate<'_>],
    mode: PruneModeContext<'_>,
    inventory: InventoryContext<'_>,
    mutation_receipt: &MutationReceipt<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, PRUNE_ARTIFACT, inventory);
    out.push_str("  \"mode\": {\n");
    out.push_str(&format!(
        "    \"dry_run\": {},\n",
        bool_json(!mode.write_requested)
    ));
    out.push_str(&format!(
        "    \"write_requested\": {},\n",
        bool_json(mode.write_requested)
    ));
    out.push_str(&format!(
        "    \"explicit_dry_run\": {},\n",
        bool_json(mode.explicit_dry_run)
    ));
    out.push_str(&format!(
        "    \"written_path\": {}\n",
        option_json(mode.written_path)
    ));
    out.push_str("  },\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"stale_entries\": {}\n  }},\n",
        candidates.len()
    ));
    out.push_str("  \"stale_entries\": [\n");
    for (index, candidate) in candidates.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_prune_candidate_json(candidate, "  "));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"mutation_receipt\": ");
    out.push_str(&crate::render_mutation_receipt_json(mutation_receipt, "  "));
    out.push('\n');
    out.push_str("}\n");
    out
}

fn render_prune_candidate_json(candidate: &PruneCandidate<'_>, indent: &str) -> String {
    let field_indent = format!("{indent}  ");
    let mut fields = vec![
        format!("{field_indent}\"id\": \"{}\"", json_escape(candidate.id)),
        format!(
            "{field_indent}\"kind\": \"{}\"",
            json_escape(candidate.kind)
        ),
    ];
    push_optional_string_field(&mut fields, &field_indent, "family", candidate.family);
    fields.extend([
        format!(
            "{field_indent}\"owner\": \"{}\"",
            json_escape(candidate.owner)
        ),
        format!(
            "{field_indent}\"classification\": \"{}\"",
            json_escape(candidate.classification)
        ),
        format!(
            "{field_indent}\"scope\": \"{}\"",
            json_escape(candidate.scope)
        ),
        format!(
            "{field_indent}\"reason\": \"{}\"",
            json_escape(candidate.reason)
        ),
    ]);
    format!("{indent}  {{\n{}\n{indent}  }}", fields.join(",\n"))
}
