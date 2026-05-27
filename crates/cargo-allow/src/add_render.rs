use allow_core::{AllowEntry, Finding};
use allow_match::finding_location;
use std::path::Path;

use crate::source_syntax_inventory_context;

pub(super) fn render_add_summary(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow add summary\n");
    out.push_str(&format!("id: {}\n", entry.id));
    out.push_str(&format!("kind: {}\n", entry.kind));
    if let Some(family) = &entry.family {
        out.push_str(&format!("family: {family}\n"));
    }
    out.push_str(&format!("scope: {}\n", entry.path_or_glob()));
    out.push_str(&format!("owner: {}\n", entry.owner));
    out.push_str(&format!("classification: {}\n", entry.classification));
    out.push_str(&format!("matched finding: {}\n", finding_location(finding)));
    if let Some(output) = output {
        out.push_str(&format!("output: {}\n", output.display()));
    } else {
        out.push_str("output: stdout\n");
    }
    out.push_str("claim boundary: generated policy entry requires human review before merge.\n");
    out
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AddContext<'a> {
    pub(super) inventory_source: &'a str,
    pub(super) source_tree_root: Option<&'a str>,
    pub(super) inventory_files: Option<usize>,
}

impl Default for AddContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

pub(super) fn render_add_summary_json(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
    force: bool,
    context: AddContext<'_>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    allow_report::render_add_json(allow_report::AddReport {
        inventory: source_syntax_inventory_context(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
        entry,
        selected_finding: finding,
        policy_output: policy_output.as_deref(),
        force,
    })
}
