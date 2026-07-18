use serde_json::Value;

use super::*;

fn sample_receipt() -> AddPlanApplicationV1<'static> {
    let digest = "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111";
    let after = "sha256:v1:2222222222222222222222222222222222222222222222222222222222222222";
    AddPlanApplicationV1 {
        tool_version: "0.1.10".to_string(),
        inventory: InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(3)),
        plan_digest: digest.to_string(),
        repository_identity: digest.to_string(),
        finding_digest: digest.to_string(),
        target_ledger: "policy/allow.toml".to_string(),
        policy_before_digest: digest.to_string(),
        policy_after_digest: after.to_string(),
        added_allow_id: "allow-0007".to_string(),
        targeted_recheck: "not_executed".to_string(),
        full_check_argv: vec![
            "check".to_string(),
            "--mode".to_string(),
            "no-new".to_string(),
        ],
    }
}

#[test]
fn add_plan_application_json_binds_plan_and_policy_states() {
    let rendered = render_add_plan_application_json(&sample_receipt());
    let value: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("receipt JSON: {err}")));

    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some(ADD_PLAN_APPLICATION_SCHEMA_ID)
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some("add")
    );
    assert_eq!(
        value.pointer("/added_allow_id").and_then(Value::as_str),
        Some("allow-0007")
    );
    assert_eq!(
        value.pointer("/target_ledger").and_then(Value::as_str),
        Some("policy/allow.toml")
    );
    assert_eq!(
        value.pointer("/targeted_recheck").and_then(Value::as_str),
        Some("not_executed")
    );
    assert_ne!(
        value
            .pointer("/policy_before_digest")
            .and_then(Value::as_str),
        value
            .pointer("/policy_after_digest")
            .and_then(Value::as_str),
        "before/after policy digests must be distinct in the sample"
    );
    assert_eq!(
        value.pointer("/full_check_argv/1").and_then(Value::as_str),
        Some("--mode")
    );
}

#[test]
fn add_plan_application_claim_boundary_admits_the_mutation() {
    let rendered = render_add_plan_application_json(&sample_receipt());
    let value: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("receipt JSON: {err}")));
    let claims = value
        .pointer("/claim_boundary")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("claim_boundary should be an array"));
    let claims: Vec<&str> = claims.iter().filter_map(Value::as_str).collect();
    // This artifact exists because policy WAS mutated: it must not launder an
    // honest write behind a `policy_not_mutated` claim.
    assert!(
        !claims.contains(&"policy_not_mutated"),
        "add-plan-application must not claim policy_not_mutated"
    );
    assert!(claims.contains(&"targeted_recheck_not_executed"));
    assert!(claims.contains(&"full_repository_check_not_executed"));
}
