use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;

#[test]
fn add_finding_plan_json_preserves_bound_identity_and_structured_argv() {
    let digest = "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
    let plan = AddFindingPlanV1 {
        tool_version: "0.1.10".to_string(),
        repository: AddFindingPlanRepository {
            identity: digest.to_string(),
            root: "H:/repo".to_string(),
        },
        inventory: InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(12))
            .with_completeness("complete"),
        evaluation: EvaluationContext {
            scope: "scoped",
            locality: "proven",
            reasons: &[],
        },
        inventory_basis_identity: digest.to_string(),
        policy: AddFindingPlanPolicy {
            path: "policy/allow.toml".to_string(),
            digest: digest.to_string(),
        },
        finding: AddFindingPlanFinding {
            kind: "panic".to_string(),
            family: Some("unwrap".to_string()),
            path: "src/lib.rs".to_string(),
            line: Some(10),
            column: Some(5),
            identity: BTreeMap::from([
                ("language".to_string(), json!("rust")),
                ("callee".to_string(), json!("unwrap")),
            ]),
            digest: digest.to_string(),
            source_file_digest: digest.to_string(),
            selector: BTreeMap::from([("callee".to_string(), json!("unwrap"))]),
        },
        outcome: AddFindingPlanOutcome {
            status: "new".to_string(),
            allow_id: None,
            message: "unreceipted panic.unwrap".to_string(),
        },
        candidates: vec![AddFindingPlanCandidate {
            allow_id: "allow-near".to_string(),
            mismatch_reasons: vec!["path mismatch".to_string()],
        }],
        required_fields: vec![
            "owner".to_string(),
            "reason".to_string(),
            "evidence".to_string(),
        ],
        proof_plans: vec![AddFindingPlanProofPlan {
            program: "cargo-allow".to_string(),
            args: vec![
                "add".to_string(),
                "--path".to_string(),
                "src/lib.rs".to_string(),
            ],
        }],
    };

    let rendered = render_add_finding_plan_json(&plan);
    let value: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("plan JSON: {err}")));
    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some(ADD_FINDING_PLAN_SCHEMA_ID)
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some("why")
    );
    assert_eq!(
        value
            .pointer("/evaluation/result_class")
            .and_then(Value::as_str),
        Some("exact_scoped")
    );
    assert_eq!(
        value.pointer("/tool_version").and_then(Value::as_str),
        Some("0.1.10")
    );
    assert_eq!(
        value
            .pointer("/inventory_basis_identity")
            .and_then(Value::as_str),
        Some(digest)
    );
    assert_eq!(
        value
            .pointer("/finding/source_file_digest")
            .and_then(Value::as_str),
        Some(digest)
    );
    assert_eq!(
        value
            .pointer("/proof_plans/0/args/2")
            .and_then(Value::as_str),
        Some("src/lib.rs")
    );
    assert!(
        value
            .pointer("/claim_boundary")
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
    );
}
