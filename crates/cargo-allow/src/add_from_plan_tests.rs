use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;

const DIGEST_A: &str = "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
const DIGEST_B: &str = "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111";
type FlagCase = (&'static str, fn(&mut AddArgs), &'static str);

fn complete_hint_inventory(include_untracked: bool) -> InventoryFacts {
    let source = if include_untracked {
        InventorySource::FilesystemIncludeUntracked
    } else {
        InventorySource::GitTracked
    };
    InventoryFacts::scanned(source, 1).with_completeness(InventoryCompleteness::Scoped)
}

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
        dry_run: false,
        update: false,
        from_plan: Some(PathBuf::from("plan.json")),
        summary_format: crate::HumanJsonFormat::Human,
        summary_output: None,
    }
}

#[test]
fn from_plan_flag_contract_errors_are_usage() {
    let cases: [FlagCase; 6] = [
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
            // `--dry-run` used to be accepted and ignored here, so a
            // documented "without writing any file" invocation applied the
            // receipt to the live ledger.
            "dry-run conflict",
            |args| {
                args.update = true;
                args.dry_run = true;
            },
            "cannot be combined with --dry-run",
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

#[test]
fn plan_file_read_failures_are_usage() {
    let path = std::env::temp_dir().join(format!(
        "cargo-allow-missing-add-finding-plan-{}.json",
        std::process::id()
    ));
    let error = read_bound_file(&path, "add-finding plan")
        .expect_err("missing add-finding plan should fail to read");
    let error = plan_input_error(error);

    assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert_eq!(error.code(), "E0001_USAGE");
    assert!(
        error
            .to_string()
            .contains("failed to read add-finding plan")
    );
    assert!(error.to_string().contains(&path.display().to_string()));
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
fn enrich_with_regen_hint_appends_plan_regeneration_command()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = from_plan_fixture_dir();
    let root = dir.join("repo");
    std::fs::create_dir(&root)?;
    let (plan, mut bindings) = matching_plan_and_bindings();
    bindings.finding_line = Some(3);
    assert!(verify_bindings(&plan, &bindings, "/repo").is_err());
    let plan_path = dir.join("add-finding-plan.json");
    std::fs::write(&plan_path, "original plan")?;
    let error = stale("finding location changed since the plan was generated");
    let enriched = enrich_with_regen_hint(
        error,
        &plan_path,
        &plan.finding,
        &bindings,
        RegenHintContext {
            root: &root,
            policy_path: &root.join("policy/allow.toml"),
            finding_path: Path::new("src/lib.rs"),
            include_untracked: false,
            ignored: &[],
            inventory_facts: complete_hint_inventory(false),
        },
    );

    let message = enriched.to_string();
    assert_eq!(enriched.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(
        message.contains("regenerate with cargo-allow why --plan"),
        "enriched error should include regeneration hint: {message}"
    );
    assert!(
        message.contains("--kind panic --path ")
            && message.contains(&root.join("src/lib.rs").display().to_string())
            && message.contains("--line 3"),
        "enriched error should include live finding coordinates: {message}"
    );
    // The recorded plan path already exists, so the advice must name a fresh
    // path instead of the doomed one (#4364).
    assert!(
        message.contains(".retry-1.json"),
        "regeneration hint should propose a fresh plan path: {message}"
    );
    assert!(
        message.contains("already exists and add-finding plans are never overwritten"),
        "regeneration hint should state why a fresh path is required: {message}"
    );
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn enrich_with_regen_hint_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
    let dir = from_plan_fixture_dir();
    let (plan, bindings) = matching_plan_and_bindings();
    let plan_path = dir.join("add-finding-plan.json");
    std::fs::write(&plan_path, "original plan")?;
    let error = stale("finding path changed since the plan was generated");
    let enriched_once = enrich_with_regen_hint(
        error,
        &plan_path,
        &plan.finding,
        &bindings,
        RegenHintContext {
            root: Path::new("/repo"),
            policy_path: Path::new("/repo/policy/allow.toml"),
            finding_path: Path::new("src/lib.rs"),
            include_untracked: false,
            ignored: &[],
            inventory_facts: complete_hint_inventory(false),
        },
    );
    let enriched_twice = enrich_with_regen_hint(
        enriched_once,
        &plan_path,
        &plan.finding,
        &bindings,
        RegenHintContext {
            root: Path::new("/repo"),
            policy_path: Path::new("/repo/policy/allow.toml"),
            finding_path: Path::new("src/lib.rs"),
            include_untracked: false,
            ignored: &[],
            inventory_facts: complete_hint_inventory(false),
        },
    );

    let hint_count = enriched_twice
        .to_string()
        .matches("regenerate with")
        .count();
    assert_eq!(
        hint_count, 1,
        "enrich should not duplicate the hint on re-application"
    );
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn recovery_hint_ignores_location_and_source_drift_only_for_advice()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = from_plan_fixture_dir();
    let plan_path = dir.join("plan.json");
    std::fs::write(&plan_path, "original plan")?;
    let (mut plan, mut bindings) = matching_plan_and_bindings();
    plan.finding.identity.insert("line_hint".into(), json!(1));
    plan.finding
        .identity
        .insert("column_hint".into(), json!(20));
    plan.finding.selector.insert("line_hint".into(), json!(1));
    bindings
        .finding_identity
        .insert("line_hint".into(), json!(3));
    bindings
        .finding_identity
        .insert("column_hint".into(), json!(22));
    bindings.finding_line = Some(3);
    bindings.finding_column = Some(22);
    bindings.source_file_digest = DIGEST_A.into();
    bindings.inventory_basis_identity = DIGEST_B.into();
    bindings.repository_identity = DIGEST_B.into();

    assert!(same_semantic_finding(&plan.finding, &bindings));
    let error = verify_bindings(&plan, &bindings, "/repo")
        .expect_err("a moved finding's stale plan must still refuse application");
    let enriched = enrich_with_regen_hint(
        error,
        &plan_path,
        &plan.finding,
        &bindings,
        RegenHintContext {
            root: Path::new("/repo"),
            policy_path: Path::new("/repo/policy/allow.toml"),
            finding_path: Path::new("src/lib.rs"),
            include_untracked: false,
            ignored: &[],
            inventory_facts: complete_hint_inventory(false),
        },
    );
    assert!(enriched.to_string().contains("--line 3"));
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn recovery_hint_refuses_each_semantic_binding_drift() {
    let mutations: [(&str, PlanMutation); 4] = [
        ("kind", |p| p.finding.kind = "unsafe".into()),
        ("family", |p| p.finding.family = None),
        ("path", |p| p.finding.path = "src/other.rs".into()),
        ("digest", |p| p.finding.digest = DIGEST_B.into()),
    ];
    for (label, mutate) in mutations {
        let (mut plan, bindings) = matching_plan_and_bindings();
        mutate(&mut plan);
        let error = stale("source inventory changed since the plan was generated");
        let before = error.to_string();
        let enriched = enrich_with_regen_hint(
            error,
            Path::new("plan.json"),
            &plan.finding,
            &bindings,
            RegenHintContext {
                root: Path::new("/repo"),
                policy_path: Path::new("/repo/policy/allow.toml"),
                finding_path: Path::new("src/lib.rs"),
                include_untracked: false,
                ignored: &[],
                inventory_facts: complete_hint_inventory(false),
            },
        );
        assert_eq!(enriched.to_string(), before, "{label} must not get advice");
    }
}

#[test]
fn recovery_hint_compares_complete_semantic_maps_in_both_directions() {
    for selector in [false, true] {
        for key in [
            "language",
            "crate_name",
            "module",
            "container",
            "ast_kind",
            "symbol",
            "callee",
            "macro_name",
            "lint",
            "receiver_fingerprint",
            "target_fingerprint",
            "normalized_snippet_hash",
            "glob",
            "future_identity_field",
        ] {
            for recorded_only in [false, true] {
                let (mut plan, mut bindings) = matching_plan_and_bindings();
                let (recorded, live) = if selector {
                    (&mut plan.finding.selector, &mut bindings.selector)
                } else {
                    (&mut plan.finding.identity, &mut bindings.finding_identity)
                };
                recorded.insert(key.into(), json!("original"));
                live.insert(key.into(), json!("replacement"));
                assert!(
                    !same_semantic_finding(&plan.finding, &bindings),
                    "changed {key}, selector={selector}"
                );
                let (recorded, live) = if selector {
                    (&mut plan.finding.selector, &mut bindings.selector)
                } else {
                    (&mut plan.finding.identity, &mut bindings.finding_identity)
                };
                if recorded_only {
                    live.remove(key);
                } else {
                    recorded.remove(key);
                }
                assert!(
                    !same_semantic_finding(&plan.finding, &bindings),
                    "missing {key}, selector={selector}, recorded_only={recorded_only}"
                );
            }
        }
    }
    let (plan, mut bindings) = matching_plan_and_bindings();
    bindings.selector.insert("column_hint".into(), json!(20));
    assert!(
        !same_semantic_finding(&plan.finding, &bindings),
        "an unknown selector field must not be ignored as a location hint"
    );
}

#[test]
fn fresh_plan_hint_path_skips_taken_names_and_stays_bounded()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-from-plan-fresh-hint-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create hint fixture: {err}")));
    let plan_path = dir.join("add-finding-plan.json");
    let first = fresh_plan_hint_path(&plan_path)
        .ok_or("a free candidate name should exist beside the recorded plan")?;
    assert_eq!(
        first.file_name().and_then(std::ffi::OsStr::to_str),
        Some("add-finding-plan.retry-1.json"),
        "the first free candidate should be retry-1: {}",
        first.display()
    );
    std::fs::write(&first, "taken")
        .unwrap_or_else(|err| std::panic::panic_any(format!("take candidate: {err}")));
    let second =
        fresh_plan_hint_path(&plan_path).ok_or("the probe should skip taken candidate names")?;
    assert_eq!(
        second.file_name().and_then(std::ffi::OsStr::to_str),
        Some("add-finding-plan.retry-2.json"),
        "taken candidate names must be skipped: {}",
        second.display()
    );
    std::fs::write(&plan_path, "original plan")?;
    for attempt in 2..=99 {
        let occupied = dir.join(format!("add-finding-plan.retry-{attempt}.json"));
        std::fs::write(occupied, "occupied")?;
    }
    require_regen_contract(
        fresh_plan_hint_path(&plan_path).is_none(),
        "the retry probe must stop after 99 occupied names",
    )?;
    let (plan, bindings) = matching_plan_and_bindings();
    let manual = enrich_with_regen_hint(
        stale("source inventory changed since the plan was generated"),
        &plan_path,
        &plan.finding,
        &bindings,
        RegenHintContext {
            root: Path::new("/repo"),
            policy_path: Path::new("/repo/policy/allow.toml"),
            finding_path: Path::new("src/lib.rs"),
            include_untracked: true,
            ignored: &[],
            inventory_facts: complete_hint_inventory(true),
        },
    );
    let manual = enrich_with_regen_hint(
        manual,
        &plan_path,
        &plan.finding,
        &bindings,
        RegenHintContext {
            root: Path::new("/repo"),
            policy_path: Path::new("/repo/policy/allow.toml"),
            finding_path: Path::new("src/lib.rs"),
            include_untracked: true,
            ignored: &[],
            inventory_facts: complete_hint_inventory(true),
        },
    );
    require_regen_contract(
        manual.kind() == allow_core::CargoAllowErrorKind::Usage
            && manual
                .to_string()
                .matches("; regenerate manually: ")
                .count()
                == 1
            && !manual.to_string().contains("regenerate with")
            && !manual.to_string().contains("<fresh-path>"),
        "exhausted retry names require one manual instruction, not a placeholder command",
    )?;
    require_regen_contract(
        std::fs::read(&plan_path)? == b"original plan" && std::fs::read(&first)? == b"taken",
        "retry probing and manual guidance must preserve existing bytes",
    )?;
    for attempt in 2..=99 {
        let occupied = dir.join(format!("add-finding-plan.retry-{attempt}.json"));
        require_regen_contract(
            std::fs::read(occupied)? == b"occupied",
            "exhaustion must preserve every occupied candidate",
        )?;
    }
    std::fs::remove_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove hint fixture: {err}")));
    Ok(())
}

fn require_regen_contract(
    condition: bool,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

#[test]
fn recovery_hint_uses_manual_guidance_for_nonpasteable_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = from_plan_fixture_dir();
    let plan_path = dir.join("plan.json");
    let (plan, bindings) = matching_plan_and_bindings();
    let mut paths = vec!["/repo\nforged command", "/repo\u{1b}[31m"];
    if cfg!(windows) {
        paths.extend(["C:\\repo%PATH%", "C:\\repo!PATH!"]);
    }
    for path in paths {
        for (root, finding_path) in [(path, "src/lib.rs"), ("/repo", path)] {
            let error = enrich_with_regen_hint(
                stale("source inventory changed since the plan was generated"),
                &plan_path,
                &plan.finding,
                &bindings,
                RegenHintContext {
                    root: Path::new(root),
                    policy_path: Path::new("/repo/policy/allow.toml"),
                    finding_path: Path::new(finding_path),
                    include_untracked: true,
                    ignored: &[],
                    inventory_facts: complete_hint_inventory(true),
                },
            );
            let message = error.to_string();
            require_regen_contract(
                error.kind() == allow_core::CargoAllowErrorKind::Usage
                    && message.contains("(policy unchanged)")
                    && message.contains("; regenerate manually: ")
                    && !message.contains("regenerate with")
                    && !message.chars().any(char::is_control),
                "unsafe display paths require manual guidance without executable or control text",
            )?;
        }
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn recovery_hint_does_not_lossily_display_paths() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::ffi::OsStrExt;

    let (plan, bindings) = matching_plan_and_bindings();
    let dir = from_plan_fixture_dir();
    let ordinary_plan = dir.join("plan.json");
    let non_utf8 = Path::new(std::ffi::OsStr::from_bytes(b"/repo-\xff"));
    for (root, policy_path, plan_path, finding_path) in [
        (
            non_utf8,
            Path::new("policy.toml"),
            ordinary_plan.as_path(),
            Path::new("src/lib.rs"),
        ),
        (
            Path::new("/repo"),
            non_utf8,
            ordinary_plan.as_path(),
            Path::new("src/lib.rs"),
        ),
        (
            Path::new("/repo"),
            Path::new("policy.toml"),
            non_utf8,
            Path::new("src/lib.rs"),
        ),
        (
            Path::new("/repo"),
            Path::new("policy.toml"),
            ordinary_plan.as_path(),
            non_utf8,
        ),
    ] {
        let error = enrich_with_regen_hint(
            stale("source inventory changed since the plan was generated"),
            plan_path,
            &plan.finding,
            &bindings,
            RegenHintContext {
                root,
                policy_path,
                finding_path,
                include_untracked: false,
                ignored: &[],
                inventory_facts: complete_hint_inventory(false),
            },
        );
        require_regen_contract(
            error.to_string().contains("; regenerate manually: ")
                && !error.to_string().contains("regenerate with"),
            "non-UTF-8 paths must not produce a lossy executable command",
        )?;
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn retry_output_requires_effective_exclusion_or_complete_tracked_inventory()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = from_plan_fixture_dir();
    let root = dir.join("plain source tree");
    std::fs::create_dir_all(root.join("plans"))?;
    let root = root.canonicalize()?;
    let fresh = root.join("plans/plan.retry-1.json");
    let policy_path = root.join("policy.toml");
    let mut context = RegenHintContext {
        root: &root,
        policy_path: &policy_path,
        finding_path: Path::new("src/lib.rs"),
        include_untracked: true,
        ignored: &[],
        inventory_facts: complete_hint_inventory(true),
    };
    require_regen_contract(
        !retry_output_is_outside_inventory(&fresh, &context) && !fresh.exists(),
        "an unavailable Git exclusion query must not advertise or create a retry",
    )?;
    let ignored = vec!["plans/**".to_string()];
    context.ignored = &ignored;
    require_regen_contract(
        retry_output_is_outside_inventory(&fresh, &context),
        "selected policy exclusion must qualify the actual prospective filename",
    )?;
    context.ignored = &[];
    context.include_untracked = false;
    context.inventory_facts = complete_hint_inventory(false);
    require_regen_contract(
        retry_output_is_outside_inventory(&fresh, &context),
        "a complete tracked inventory must keep an absent untracked retry usable",
    )?;
    for completeness in [
        InventoryCompleteness::Fallback,
        InventoryCompleteness::Partial,
    ] {
        context.inventory_facts.completeness = completeness;
        require_regen_contract(
            !retry_output_is_outside_inventory(&fresh, &context),
            "the include-untracked flag alone must not qualify a fallback or partial inventory",
        )?;
    }
    context.inventory_facts = complete_hint_inventory(true);
    context.include_untracked = true;
    require_regen_contract(
        retry_output_is_outside_inventory(&dir.join("outside.retry-1.json"), &context)
            && !retry_output_is_outside_inventory(
                &dir.join("missing parent/outside.retry-1.json"),
                &context,
            ),
        "outside placement requires an existing resolvable parent",
    )?;
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[cfg(windows)]
#[test]
fn drive_relative_recovery_base_requires_manual_guidance() -> Result<(), Box<dyn std::error::Error>>
{
    let (plan, bindings) = matching_plan_and_bindings();
    for relative in ["C:plan.json", "C:plans\\plan.json"] {
        let anchored = Path::new("C:\\original caller").join(relative);
        let error = enrich_with_regen_hint(
            stale("source inventory changed since the plan was generated"),
            &anchored,
            &plan.finding,
            &bindings,
            RegenHintContext {
                root: Path::new("C:\\selected repo"),
                policy_path: Path::new("C:\\selected repo\\policy.toml"),
                finding_path: Path::new("src\\lib.rs"),
                include_untracked: false,
                ignored: &[],
                inventory_facts: complete_hint_inventory(false),
            },
        );
        require_regen_contract(
            !anchored.is_absolute()
                && error.kind() == CargoAllowErrorKind::Usage
                && error.to_string().contains("; regenerate manually: ")
                && error.to_string().contains("not anchored absolutely")
                && !error.to_string().contains("; regenerate with "),
            "drive-relative input must not become a cross-cwd executable suggestion",
        )?;
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn recovery_hint_skips_dangling_symlink_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let dir = from_plan_fixture_dir();
    let plan_path = dir.join("plan.json");
    let occupied = dir.join("plan.retry-1.json");
    let missing = dir.join("absent-target.json");
    std::fs::write(&plan_path, "original plan")?;
    std::os::unix::fs::symlink(&missing, &occupied)?;
    require_regen_contract(
        fresh_plan_hint_path(&plan_path) == Some(dir.join("plan.retry-2.json"))
            && std::fs::read_link(&occupied)? == missing
            && std::fs::read(&plan_path)? == b"original plan",
        "a dangling symlink occupies its name and must remain untouched",
    )?;
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

static FROM_PLAN_FIXTURE_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn from_plan_fixture_dir() -> std::path::PathBuf {
    let count = FROM_PLAN_FIXTURE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-from-plan-replay-{}-{count}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create replay fixture: {err}")));
    dir
}

fn git(root: &std::path::Path, args: &[&str]) {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(root);
    if args == ["init"] {
        cmd.args(["init", "--template="]);
    } else {
        cmd.args(args);
    }
    let output = cmd
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

fn write_from_plan_git_fixture(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    std::fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    std::fs::write(
        root.join("policy/allow.toml"),
        "policy = \"cargo-allow\"\n\n[workspace]\nignored = []\ngenerated = []\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
    std::fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source write: {err}")));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow from-plan test"]);
    git(root, &["add", "policy/allow.toml", "src/lib.rs"]);
    git(root, &["commit", "-m", "base policy"]);
}

fn from_plan_replay_args(root: &std::path::Path, plan_path: std::path::PathBuf) -> AddArgs {
    AddArgs {
        root: crate::RootArgs {
            root: Some(root.to_path_buf()),
        },
        config: None,
        kind: None,
        path: None,
        line: None,
        glob: None,
        family: None,
        callee: None,
        owner: "core".to_string(),
        reason: "from-plan replay fixture".to_string(),
        classification: "reviewed_exception".to_string(),
        review_after: None,
        expires: None,
        evidence: Vec::new(),
        id: None,
        include_untracked: false,
        write: None,
        force: false,
        dry_run: false,
        update: true,
        from_plan: Some(plan_path),
        summary_format: crate::HumanJsonFormat::Human,
        summary_output: None,
    }
}

#[test]
fn replayed_from_plan_rejection_does_not_advise_a_doomed_regen() {
    // The #4364 loop: plan -> apply -> replay the same `add --from-plan`. The
    // replay is rejected because the finding is already receipted, and the
    // rejection must not suggest a `why --plan` regeneration that can never
    // succeed from this state (why refuses plans for matched findings and the
    // recorded plan path is never overwritten).
    let root = from_plan_fixture_dir();
    write_from_plan_git_fixture(&root);
    let plan_path = root.join("target/add-finding-plan.json");

    // WhyArgs fields are module-private; parse the CLI surface the way an
    // operator would invoke it.
    use clap::Parser;
    let why_args = crate::why::WhyArgs::try_parse_from([
        "cargo-allow".to_string(),
        "--kind".to_string(),
        "panic".to_string(),
        "--path".to_string(),
        "src/lib.rs".to_string(),
        "--line".to_string(),
        "1".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--plan".to_string(),
        plan_path.to_string_lossy().into_owned(),
        "--root".to_string(),
        root.to_string_lossy().into_owned(),
    ])
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse why argv: {err}")));
    crate::why::cmd_why(&why_args)
        .unwrap_or_else(|err| std::panic::panic_any(format!("why --plan should succeed: {err}")));
    assert!(
        plan_path.exists(),
        "why should have written the add-finding plan"
    );

    let args = from_plan_replay_args(&root, plan_path);
    crate::add::cmd_add(&args).unwrap_or_else(|err| {
        std::panic::panic_any(format!("first add --from-plan should apply: {err}"))
    });

    let replay =
        crate::add::cmd_add(&args).expect_err("replaying a satisfied plan must be rejected");
    assert_eq!(replay.kind(), allow_core::CargoAllowErrorKind::Usage);
    let message = replay.to_string();
    assert!(
        message.contains("already receipted or blocked with status `matched`"),
        "replay rejection should name the receipted posture: {message}"
    );
    assert!(
        message.contains("use list or explain before editing policy"),
        "replay rejection should keep its executable guidance: {message}"
    );
    assert!(
        !message.contains("regenerate with"),
        "an already-receipted rejection must not advise a regeneration that cannot succeed: {message}"
    );
    std::fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove replay fixture: {err}")));
}
