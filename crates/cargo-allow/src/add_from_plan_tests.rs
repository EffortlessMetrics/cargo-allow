use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;

const DIGEST_A: &str = "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
const DIGEST_B: &str = "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111";

/// A single-field perturbation applied to a valid plan, used to prove each
/// binding/generation check rejects independently.
type PlanMutation = fn(&mut LoadedPlan);

fn matching_plan_and_bindings() -> (LoadedPlan, PlanFindingBindings) {
    let identity: BTreeMap<String, Value> = BTreeMap::from([
        ("language".to_string(), json!("rust")),
        ("callee".to_string(), json!("unwrap")),
    ]);
    let selector: BTreeMap<String, Value> =
        BTreeMap::from([("callee".to_string(), json!("unwrap"))]);
    let plan = LoadedPlan {
        schema_version: 1,
        schema_id: allow_report::ADD_FINDING_PLAN_SCHEMA_ID.to_string(),
        tool: "cargo-allow".to_string(),
        tool_version: "0.1.11".to_string(),
        command: "why".to_string(),
        claim_boundary: vec!["source_syntax_only".to_string()],
        scanner_limitations: vec!["rustc_not_invoked".to_string()],
        repository: LoadedRepository {
            identity: DIGEST_A.to_string(),
            root: "/repo".to_string(),
        },
        inventory: json!({"scope": "source_tree"}),
        inventory_basis_identity: DIGEST_A.to_string(),
        policy: LoadedPolicy {
            path: "policy/allow.toml".to_string(),
            digest: DIGEST_B.to_string(),
        },
        finding: LoadedFinding {
            kind: "panic".to_string(),
            family: Some("unwrap".to_string()),
            path: "src/lib.rs".to_string(),
            line: Some(1),
            column: Some(20),
            identity: identity.clone(),
            digest: DIGEST_A.to_string(),
            source_file_digest: DIGEST_B.to_string(),
            selector: selector.clone(),
        },
        outcome: LoadedOutcome {
            status: "new".to_string(),
            allow_id: None,
            message: "unreceipted panic.unwrap".to_string(),
        },
        candidates: Vec::new(),
        required_fields: vec!["owner".to_string()],
        proof_plans: Vec::new(),
    };
    let bindings = PlanFindingBindings {
        repository_identity: DIGEST_A.to_string(),
        inventory_basis_identity: DIGEST_A.to_string(),
        policy_path: "policy/allow.toml".to_string(),
        policy_digest: DIGEST_B.to_string(),
        finding_kind: "panic".to_string(),
        finding_family: Some("unwrap".to_string()),
        finding_path: "src/lib.rs".to_string(),
        finding_line: Some(1),
        finding_column: Some(20),
        finding_identity: identity,
        finding_digest: DIGEST_A.to_string(),
        source_file_digest: DIGEST_B.to_string(),
        selector,
    };
    (plan, bindings)
}

#[test]
fn matching_plan_and_bindings_verify() {
    let (plan, bindings) = matching_plan_and_bindings();
    assert!(verify_bindings(&plan, &bindings, "/repo").is_ok());
    assert!(validate_plan_generation(&plan).is_ok());
}

#[test]
fn each_binding_drift_is_rejected_without_ok() {
    // Every load-bearing binding, when perturbed, must flip verify_bindings to
    // Err. This is the core no-silent-drift guarantee.
    let mutate: Vec<(&str, PlanMutation)> = vec![
        ("repository root", |p| {
            p.repository.root = "/other".to_string()
        }),
        ("policy path", |p| {
            p.policy.path = "policy/other.toml".to_string()
        }),
        ("policy digest", |p| p.policy.digest = DIGEST_A.to_string()),
        ("inventory basis", |p| {
            p.inventory_basis_identity = DIGEST_B.to_string()
        }),
        ("repository identity", |p| {
            p.repository.identity = DIGEST_B.to_string()
        }),
        ("finding kind", |p| p.finding.kind = "unsafe".to_string()),
        ("finding family", |p| p.finding.family = None),
        ("finding path", |p| {
            p.finding.path = "src/other.rs".to_string()
        }),
        ("finding digest", |p| {
            p.finding.digest = DIGEST_B.to_string()
        }),
        ("source file digest", |p| {
            p.finding.source_file_digest = DIGEST_A.to_string()
        }),
        ("finding identity", |p| {
            p.finding
                .identity
                .insert("callee".to_string(), json!("expect"));
        }),
        ("selector", |p| {
            p.finding
                .selector
                .insert("callee".to_string(), json!("expect"));
        }),
    ];
    for (label, mutation) in mutate {
        let (mut plan, bindings) = matching_plan_and_bindings();
        mutation(&mut plan);
        assert!(
            verify_bindings(&plan, &bindings, "/repo").is_err(),
            "{label} drift must be rejected"
        );
    }
}

#[test]
fn unsupported_generations_are_rejected() {
    let cases: Vec<(&str, PlanMutation)> = vec![
        ("schema id", |p| {
            p.schema_id = "cargo-allow.other.v1".to_string()
        }),
        ("schema version", |p| p.schema_version = 2),
        ("tool", |p| p.tool = "other-tool".to_string()),
        ("command", |p| p.command = "add".to_string()),
        ("non-new outcome", |p| {
            p.outcome.status = "matched".to_string()
        }),
    ];
    for (label, mutation) in cases {
        let (mut plan, _bindings) = matching_plan_and_bindings();
        mutation(&mut plan);
        assert!(
            validate_plan_generation(&plan).is_err(),
            "{label} must be rejected"
        );
    }
}

#[test]
fn strict_parse_rejects_unknown_top_level_fields() {
    let (plan, _bindings) = matching_plan_and_bindings();
    // Serialize a valid-shaped object, then inject an unexpected top-level key.
    let mut object = json!({
        "schema_version": plan.schema_version,
        "schema_id": plan.schema_id,
        "tool": plan.tool,
        "tool_version": plan.tool_version,
        "command": plan.command,
        "claim_boundary": plan.claim_boundary,
        "scanner_limitations": plan.scanner_limitations,
        "repository": {"identity": plan.repository.identity, "root": plan.repository.root},
        "inventory": plan.inventory,
        "inventory_basis_identity": plan.inventory_basis_identity,
        "policy": {"path": plan.policy.path, "digest": plan.policy.digest},
        "finding": {
            "kind": plan.finding.kind, "family": plan.finding.family,
            "path": plan.finding.path, "line": plan.finding.line, "column": plan.finding.column,
            "identity": plan.finding.identity, "digest": plan.finding.digest,
            "source_file_digest": plan.finding.source_file_digest, "selector": plan.finding.selector,
        },
        "outcome": {"status": plan.outcome.status, "allow_id": plan.outcome.allow_id, "message": plan.outcome.message},
        "candidates": plan.candidates,
        "required_fields": plan.required_fields,
        "proof_plans": plan.proof_plans,
    });
    // Baseline: the well-formed object parses.
    assert!(parse_plan_strict(object.to_string().as_bytes()).is_ok());
    // Injected unknown field is rejected by deny_unknown_fields.
    object
        .as_object_mut()
        .expect("object")
        .insert("unexpected".to_string(), json!("value"));
    assert!(parse_plan_strict(object.to_string().as_bytes()).is_err());
}

#[test]
fn full_check_argv_carries_root_config_and_optional_untracked() {
    let argv = full_check_argv("/repo", "policy/allow.toml", false);
    assert_eq!(&argv[0..3], &["check", "--mode", "no-new"]);
    assert!(argv.iter().any(|arg| arg == "--root"));
    assert!(argv.iter().any(|arg| arg == "policy/allow.toml"));
    assert!(!argv.iter().any(|arg| arg == "--include-untracked"));

    let with_untracked = full_check_argv("/repo", "policy/allow.toml", true);
    assert!(
        with_untracked
            .iter()
            .any(|arg| arg == "--include-untracked")
    );
}
