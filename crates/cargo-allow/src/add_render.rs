use allow_core::{AllowEntry, Finding};
use allow_report::{MutationReceipt, Style};

use super::AddContext;

/// Shared mutation-receipt envelope for `add` (GOAL-0004 PR 5A;
/// CARGO-ALLOW-SPEC-0008 "Mutation Receipt Envelope"). Provenance and
/// changed-entry metadata only; does not change `add`'s write behavior.
pub(super) fn add_mutation_receipt<'a, 'b>(
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
            format!("cargo-allow explain {}", entry.id),
            "cargo-allow check --mode no-new".to_string(),
        ],
    }
}

#[cfg(test)]
pub(super) fn render_add_summary(
    entry: &AllowEntry,
    finding: &Finding,
    policy_output: Option<&str>,
    context: AddContext<'_>,
) -> String {
    render_add_summary_styled(entry, finding, policy_output, context, Style::PLAIN)
}

pub(super) fn render_add_summary_styled(
    entry: &AllowEntry,
    finding: &Finding,
    policy_output: Option<&str>,
    context: AddContext<'_>,
    style: Style,
) -> String {
    let mutation_receipt = add_mutation_receipt(entry, &context, policy_output);
    allow_report::render_add_human_styled(
        allow_report::AddReport::new(
            context.inventory,
            entry,
            finding,
            policy_output,
            false,
            mutation_receipt,
        ),
        style,
    )
}

pub(super) fn render_add_summary_json(
    entry: &AllowEntry,
    finding: &Finding,
    policy_output: Option<&str>,
    force: bool,
    context: AddContext<'_>,
) -> String {
    let mutation_receipt = add_mutation_receipt(entry, &context, policy_output);
    allow_report::render_add_json(allow_report::AddReport::new(
        context.inventory,
        entry,
        finding,
        policy_output,
        force,
        mutation_receipt,
    ))
}
