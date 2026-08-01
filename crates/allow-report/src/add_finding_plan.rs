use serde_json::{Map, Value, json};

use crate::artifacts::AddFindingPlanV1;
use crate::contracts::ADD_FINDING_PLAN_ARTIFACT;
use crate::{claim_boundary_for_schema_id, scanner_limitations_for_schema_id};

pub fn render_add_finding_plan_json(plan: &AddFindingPlanV1<'_>) -> String {
    let result_class = plan.evaluation.result_class(plan.inventory);
    render_add_finding_plan_json_with_result_class(plan, result_class)
}

/// Render an add-finding plan with caller-supplied scanner evidence while
/// retaining the existing public plan data shape.
pub fn render_add_finding_plan_json_with_result_class(
    plan: &AddFindingPlanV1<'_>,
    result_class: Option<&str>,
) -> String {
    let inventory = plan.inventory;
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

    let mut evaluation_json = Map::from_iter([
        ("scope".to_string(), json!(plan.evaluation.scope)),
        ("locality".to_string(), json!(plan.evaluation.locality)),
        ("reasons".to_string(), json!(plan.evaluation.reasons)),
    ]);
    if let Some(result_class) = result_class {
        evaluation_json.insert("result_class".to_string(), json!(result_class));
    }

    let value = json!({
        "schema_version": ADD_FINDING_PLAN_ARTIFACT.schema_version,
        "schema_id": ADD_FINDING_PLAN_ARTIFACT.schema_id,
        "tool": "cargo-allow",
        "tool_version": plan.tool_version,
        "command": "why",
        "claim_boundary": claim_boundary_for_schema_id(ADD_FINDING_PLAN_ARTIFACT.schema_id),
        "scanner_limitations": scanner_limitations_for_schema_id(ADD_FINDING_PLAN_ARTIFACT.schema_id),
        "repository": { "identity": plan.repository.identity, "root": plan.repository.root },
        "inventory": Value::Object(inventory_json),
        "evaluation": Value::Object(evaluation_json),
        "inventory_basis_identity": plan.inventory_basis_identity,
        "policy": { "path": plan.policy.path, "digest": plan.policy.digest },
        "finding": {
            "kind": plan.finding.kind, "family": plan.finding.family,
            "path": plan.finding.path, "line": plan.finding.line, "column": plan.finding.column,
            "identity": plan.finding.identity, "digest": plan.finding.digest,
            "source_file_digest": plan.finding.source_file_digest, "selector": plan.finding.selector,
        },
        "outcome": { "status": plan.outcome.status, "allow_id": plan.outcome.allow_id, "message": plan.outcome.message },
        "candidates": plan.candidates.iter().map(|candidate| json!({
            "allow_id": candidate.allow_id,
            "mismatch_reasons": candidate.mismatch_reasons,
        })).collect::<Vec<_>>(),
        "required_fields": plan.required_fields,
        "proof_plans": plan.proof_plans.iter().map(|proof| json!({
            "program": proof.program, "args": proof.args,
        })).collect::<Vec<_>>(),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|err| format!("{{\"serialization_error\":\"{err}\"}}"));
    rendered.push('\n');
    rendered
}
