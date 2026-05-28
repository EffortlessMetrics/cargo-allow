use allow_core::AllowEntry;

use crate::converter_workflow_action_entries::workflow_action_entry;
use crate::converter_workflow_file_entries::workflow_file_entry;
use crate::types::LegacyWorkflowRule;

pub(crate) fn entries_from_workflow_rule(rule: &LegacyWorkflowRule) -> Vec<AllowEntry> {
    let mut entries = Vec::with_capacity(rule.external_actions.len() + 1);
    entries.push(workflow_file_entry(rule));
    entries.extend(
        rule.external_actions
            .iter()
            .map(|action| workflow_action_entry(rule, action)),
    );
    entries
}
