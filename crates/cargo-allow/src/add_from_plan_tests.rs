use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;

const DIGEST_A: &str = "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
const DIGEST_B: &str = "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111";
type FlagCase = (&'static str, fn(&mut AddArgs), &'static str);

fn base_from_plan_args() -> AddArgs {
    AddArgs {
        root: crate::RootArgs { root: None },
        config: None,
        kind: None,
        path: None,
        line: None,
        glob: None,
        family: None,
        callee: None,
        owner: "owner".to_string(),
        reason: "reason".to_string(),
        classification: "reviewed_exception".to_string(),
        review_after: None,
        expires: None,
        evidence: Vec::new(),
        id: None,
        include_untracked: false,
        write: None,
        force: false,
        update: false,
        from_plan: Some(PathBuf::from("plan.json")),
        summary_format: crate::HumanJsonFormat::Human,
        summary_output: None,
    }
}

#[test]
fn from_plan_flag_contract_errors_are_usage() {
    let cases: [FlagCase; 5] = [
        ("requires update", |_| {}, "requires --update"),
        (
            "write conflict",
            |args| {
                args.update = true;
                args.write = Some(PathBuf::from("out.toml"));
            },
            "cannot be combined with --write",
        ),
        (
            "force conflict",
            |args| {
                args.update = true;
                args.force = true;
            },
            "cannot be combined with --force",
        ),
        (
            "kind conflict",
            |args| {
                args.update = true;
                args.kind = Some("panic".to_string());
            },
            "--kind cannot be combined",
        ),
        (
            "manual selector conflict",
            |args| {
                args.update = true;
                args.path = Some(PathBuf::from("src/lib.rs"));
            },
            "manual target selectors",
        ),
    ];

    for (label, mutate, message) in cases {
        let mut args = base_from_plan_args();
        mutate(&mut args);
        let error = reject_conflicting_from_plan_flags(&args)
            .expect_err("conflicting from-plan invocation should fail");
        assert_eq!(
            error.kind(),
            allow_core::CargoAllowErrorKind::Usage,
            "{label} should be a usage error"
        );
        assert!(
            error.to_string().contains(message),
            "{label} should preserve its guidance: {error}"
        );
    }
}

#[test]
fn from_plan_duplicate_allow_id_is_usage() {
    let error = ensure_unique_allow_id(["allow-0001"], "allow-0001")
        .expect_err("from-plan should reject a duplicate allow ID");

    assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(error.to_string().contains(
        "allow entry id `allow-0001` already exists; pass a unique --id or omit --id to auto-assign"
    ));
    assert!(ensure_unique_allow_id(["allow-0001"], "allow-0002").is_ok());
}

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
        command: "why".to_string(),
        repository: LoadedRepository {
            identity: DIGEST_A.to_string(),
            root: "/repo".to_string(),
        },
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
        },
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
fn strict_parse_accepts_a_full_plan_and_rejects_malformed_input() {
    // A full v1 plan (with fields the transaction ignores, such as proof_plans
    // and candidates) parses cleanly.
    let full_plan = json!({
        "schema_version": 1,
        "schema_id": allow_report::ADD_FINDING_PLAN_SCHEMA_ID,
        "tool": "cargo-allow",
        "tool_version": "0.1.11",
        "command": "why",
        "claim_boundary": ["source_syntax_only"],
        "scanner_limitations": ["rustc_not_invoked"],
        "repository": {"identity": DIGEST_A, "root": "/repo"},
        "inventory": {"scope": "source_tree", "scanner": "source_syntax", "source": "git_tracked"},
        "inventory_basis_identity": DIGEST_A,
        "policy": {"path": "policy/allow.toml", "digest": DIGEST_B},
        "finding": {
            "kind": "panic", "family": "unwrap", "path": "src/lib.rs", "line": 1, "column": 20,
            "identity": {"language": "rust"}, "digest": DIGEST_A,
            "source_file_digest": DIGEST_B, "selector": {"callee": "unwrap"},
        },
        "outcome": {"status": "new", "allow_id": null, "message": "unreceipted panic.unwrap"},
        "candidates": [],
        "required_fields": ["owner"],
        "proof_plans": [],
    });
    assert!(parse_plan_strict(full_plan.to_string().as_bytes()).is_ok());

    // Not JSON at all.
    assert!(parse_plan_strict(b"not a plan").is_err());
    // Missing a required load-bearing object (`finding`).
    let mut missing_finding = full_plan.clone();
    missing_finding
        .as_object_mut()
        .unwrap_or_else(|| std::panic::panic_any("plan object"))
        .remove("finding");
    assert!(parse_plan_strict(missing_finding.to_string().as_bytes()).is_err());
    // A required digest with the wrong JSON type.
    let mut wrong_type = full_plan;
    if let Some(policy) = wrong_type
        .get_mut("policy")
        .and_then(serde_json::Value::as_object_mut)
    {
        policy.insert("digest".to_string(), json!(42));
    }
    assert!(parse_plan_strict(wrong_type.to_string().as_bytes()).is_err());
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

#[test]
fn enrich_with_regen_hint_appends_plan_regeneration_command() {
    let (plan, _) = matching_plan_and_bindings();
    let plan_path = std::path::Path::new("target/cargo-allow/add-finding-plan.json");
    let error = stale("policy changed since the plan was generated");
    let enriched = enrich_with_regen_hint(error, plan_path, &plan);

    let message = enriched.to_string();
    assert_eq!(enriched.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(
        message.contains("regenerate with cargo-allow why --plan"),
        "enriched error should include regeneration hint: {message}"
    );
    assert!(
        message.contains("--kind panic --path src/lib.rs --line 1"),
        "enriched error should include plan finding coordinates: {message}"
    );
}

#[test]
fn enrich_with_regen_hint_is_idempotent() {
    let (plan, _) = matching_plan_and_bindings();
    let plan_path = std::path::Path::new("target/cargo-allow/add-finding-plan.json");
    let error = stale("finding path changed since the plan was generated");
    let enriched_once = enrich_with_regen_hint(error, plan_path, &plan);
    let enriched_twice = enrich_with_regen_hint(enriched_once, plan_path, &plan);

    let hint_count = enriched_twice
        .to_string()
        .matches("regenerate with")
        .count();
    assert_eq!(
        hint_count, 1,
        "enrich should not duplicate the hint on re-application"
    );
}
