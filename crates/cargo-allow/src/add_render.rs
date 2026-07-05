use allow_core::{AllowEntry, Finding};
use allow_report::MutationReceipt;
use std::path::Path;

use super::AddContext;

/// Shared mutation-receipt envelope for `add` (GOAL-0004 PR 5A;
/// CARGO-ALLOW-SPEC-0008 "Mutation Receipt Envelope"). Provenance and
/// changed-entry metadata only; does not change `add`'s write behavior.
fn add_mutation_receipt<'a, 'b>(
    entry: &'a AllowEntry,
    context: &'a AddContext<'b>,
    policy_output: Option<&'a str>,
) -> MutationReceipt<'a> {
    let result = if policy_output.is_some() {
        "written"
    } else {
        "stdout"
    };
    MutationReceipt {
        operation: "add",
        tool_version: env!("CARGO_PKG_VERSION"),
        repo_root: context.repo_root.as_deref(),
        config_source: context.config_source.as_deref(),
        ledger_ids: Vec::new(),
        changed_allow_ids: vec![&entry.id],
        before_fingerprints: vec![None],
        after_fingerprints: vec![Some(allow_core::allow_entry_content_fingerprint(entry))],
        result,
        next_commands: vec![
            format!("cargo-allow explain --id {}", entry.id),
            "cargo-allow check --mode no-new".to_string(),
        ],
    }
}

pub(super) fn render_add_summary(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
    context: AddContext<'_>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    let mutation_receipt = add_mutation_receipt(entry, &context, policy_output.as_deref());
    allow_report::render_add_human(allow_report::AddReport::new(
        context.inventory,
        entry,
        finding,
        policy_output.as_deref(),
        false,
        mutation_receipt,
    ))
}

pub(super) fn render_add_summary_json(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
    force: bool,
    context: AddContext<'_>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    let mutation_receipt = add_mutation_receipt(entry, &context, policy_output.as_deref());
    allow_report::render_add_json(allow_report::AddReport::new(
        context.inventory,
        entry,
        finding,
        policy_output.as_deref(),
        force,
        mutation_receipt,
    ))
}
