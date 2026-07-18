use serde_json::{Map, Value, json};

use crate::artifacts::AddPlanApplicationV1;
use crate::contracts::ADD_PLAN_APPLICATION_ARTIFACT;
use crate::{claim_boundary_for_schema_id, scanner_limitations_for_schema_id};

/// Render the `cargo-allow.add-plan-application.v1` receipt as pretty JSON.
///
/// The receipt is emitted after `add --from-plan` has already validated the
/// full policy and atomically replaced the ledger; it records provenance, not a
/// pending intent. Unlike the add-finding plan it does not claim
/// `policy_not_mutated` — this artifact exists precisely because policy *was*
/// mutated — but it does honestly disclaim that any recheck was executed.
pub fn render_add_plan_application_json(receipt: &AddPlanApplicationV1<'_>) -> String {
    let inventory = receipt.inventory;
    let mut inventory_json = Map::from_iter([
        ("scope".to_string(), json!(inventory.scope)),
        ("scanner".to_string(), json!(inventory.scanner)),
        ("source".to_string(), json!(inventory.source)),
    ]);
    if let Some(root) = inventory.root {
        inventory_json.insert("root".to_string(), json!(root));
    }
    if let Some(files_scanned) = inventory.files_scanned {
        inventory_json.insert("files_scanned".to_string(), json!(files_scanned));
    }
    if inventory.empty_git_tracked {
        inventory_json.insert("empty_git_tracked".to_string(), json!(true));
    }
    if let Some(completeness) = inventory.completeness {
        inventory_json.insert("completeness".to_string(), json!(completeness));
    }

    let value = json!({
        "schema_version": ADD_PLAN_APPLICATION_ARTIFACT.schema_version,
        "schema_id": ADD_PLAN_APPLICATION_ARTIFACT.schema_id,
        "tool": "cargo-allow",
        "tool_version": receipt.tool_version,
        "command": "add",
        "claim_boundary": claim_boundary_for_schema_id(ADD_PLAN_APPLICATION_ARTIFACT.schema_id),
        "scanner_limitations": scanner_limitations_for_schema_id(ADD_PLAN_APPLICATION_ARTIFACT.schema_id),
        "inventory": Value::Object(inventory_json),
        "plan_digest": receipt.plan_digest,
        "repository_identity": receipt.repository_identity,
        "finding_digest": receipt.finding_digest,
        "target_ledger": receipt.target_ledger,
        "policy_before_digest": receipt.policy_before_digest,
        "policy_after_digest": receipt.policy_after_digest,
        "added_allow_id": receipt.added_allow_id,
        "targeted_recheck": receipt.targeted_recheck,
        "full_check_argv": receipt.full_check_argv,
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|err| format!("{{\"serialization_error\":\"{err}\"}}"));
    rendered.push('\n');
    rendered
}
