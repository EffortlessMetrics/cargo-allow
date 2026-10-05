use super::why_render::{render_why_target_scan_json, render_why_target_scan_text, why_next_steps};
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat, RootArgs};
use allow_core::{
    AllowEntry, Finding, FindingKind, Lifecycle, MatchOutcome, MatchStatus, Selector, Span,
    StructuralIdentity, normalize_path,
};
use allow_report::EvaluationContext;
use clap::Parser;
use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[test]
fn clap_parses_why_finding_location() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "why",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "42",
        "--format",
        "json",
        "--plan",
        "target/add-plan.json",
        "--root",
        ".",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse why: {err}")));

    match parsed.command {
        Some(CargoAllowCommand::Why(args)) => {
            assert_eq!(args.kind, "panic");
            assert_eq!(args.path, PathBuf::from("src/lib.rs"));
            assert_eq!(args.line, 42);
            assert_eq!(args.format, HumanJsonFormat::Json);
            assert_eq!(args.plan, Some(PathBuf::from("target/add-plan.json")));
            assert_eq!(args.root.root.as_deref(), Some(std::path::Path::new(".")));
        }
        other => std::panic::panic_any(format!("expected Why command, got {other:?}")),
    }
}

#[test]
fn why_rejects_same_plan_and_output_as_usage() {
    let path = PathBuf::from("target/cargo-allow-why-plan.json");
    let err = cmd_why(&preflight_args(Some(path.clone()), Some(path)))
        .expect_err("same plan and output path should be rejected");

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(err.to_string().contains("--plan and --output"));
}

#[test]
fn why_rejects_existing_plan_as_usage() {
    let path = std::env::temp_dir().join(format!(
        "cargo-allow-why-existing-plan-{}.json",
        std::process::id()
    ));
    fs::write(&path, "existing plan")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write existing plan: {err}")));

    let err = cmd_why(&preflight_args(Some(path.clone()), None))
        .expect_err("existing plan path should be rejected");
    let _ = fs::remove_file(&path);

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn why_command_uses_the_retained_scoped_context() -> Result<(), Box<dyn Error>> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-why-context-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(root.join("policy"))?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("policy/allow.toml"),
        "policy = \"cargo-allow\"\n\n[[allow]]\nid = \"why-context\"\nkind = \"panic\"\nfamily = \"unwrap\"\nglob = \"src/**/*.rs\"\nowner = \"test\"\nclassification = \"fixture\"\nreason = \"why context fixture\"\nreview_after = \"2099-01-01\"\n\n[allow.selector]\nast_kind = \"call_expression\"\nglob = \"src/**/*.rs\"\n\n[workspace]\nignored = []\ngenerated = []\n",
    )?;
    fs::write(
        root.join("src/lib.rs"),
        "pub fn probe() { let value: Option<u8> = None; let _ = value.unwrap(); }\n",
    )?;
    for args in [
        vec!["init"],
        vec!["config", "user.email", "cargo-allow@example.invalid"],
        vec!["config", "user.name", "cargo-allow why test"],
        vec!["add", "--all"],
        vec!["commit", "-m", "why context fixture"],
    ] {
        let status = std::process::Command::new("git")
            .args(&args)
            .current_dir(&root)
            .status()?;
        if !status.success() {
            return Err(format!("git command failed: {args:?}").into());
        }
    }

    let args = WhyArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        kind: "panic".to_string(),
        path: PathBuf::from("src/lib.rs"),
        line: 1,
        include_untracked: false,
        format: HumanJsonFormat::Json,
        output: Some(root.join("why.json")),
        plan: None,
    };
    let scoped = crate::world::load_world_for_path(
        Some(&root),
        None,
        true,
        Some("panic"),
        false,
        &root.join("src/lib.rs"),
    )?;
    let scoped_digest = scoped.0.inventory_facts.policy_digest_text();
    let broadened = broaden_context(&scoped.0, scoped_digest.clone(), false, Some("panic"))?;
    if broadened.findings.is_empty() {
        return Err("broadened why context lost the fixture finding".into());
    }
    if broadened.inventory_facts.policy_digest_text() != scoped_digest {
        return Err("broadened why context changed the selected policy digest".into());
    }
    cmd_why(&args)?;
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn why_missing_evaluation_outcome_is_an_internal_invariant() {
    let err = missing_evaluation_outcome_error(std::path::Path::new("src/lib.rs"), 42);

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Internal);
    assert_eq!(err.code(), "E0008_INTERNAL");
    assert!(
        err.to_string()
            .contains("no evaluation outcome for finding at src/lib.rs:42")
    );
}

#[test]
fn why_output_path_resolution_failures_are_artifacts() {
    let path = std::path::Path::new("target/why-output.json");
    let source = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
    let err = resolve_output_path_result(path, Err(source)).expect_err("resolution should fail");

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Artifact);
    assert_eq!(err.code(), "E0007_ARTIFACT");
    assert!(err.to_string().contains("failed to resolve output path"));
    assert!(err.to_string().contains("target/why-output.json"));
    assert!(Error::source(&err).is_some());
}

#[test]
fn render_why_lists_mismatch_reasons_for_new_findings() {
    let finding = sample_finding_at("src/lib.rs", 10);
    let entry = near_miss_entry();
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic.unwrap at src/lib.rs:10:1".to_string(),
        score: 0,
    };
    let reasons = explain_match_failure(&entry, &finding);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("callee mismatch")),
        "expected callee mismatch, got {reasons:?}"
    );
    let text = render_why_text(
        &finding,
        &outcome,
        &[WhyCandidate {
            entry: &entry,
            reasons,
        }],
    );

    assert!(text.contains("# Why this finding is unreceipted"));
    assert!(text.contains("src/lib.rs:10:1"));
    assert!(text.contains("### `allow-near-miss`"));
    assert!(text.contains("callee mismatch"));
    assert!(text.contains("cargo-allow"));
    assert!(text.contains("result_class: exact_scoped"));
    assert!(text.contains("Claim boundary"));

    let styled = render_why_text_styled(
        &finding,
        &outcome,
        &[WhyCandidate {
            entry: &entry,
            reasons: explain_match_failure(&entry, &finding),
        }],
        allow_report::Style::ANSI,
    );
    assert!(styled.contains("- status: \u{1b}[31mnew\u{1b}[0m"));
    assert!(
        !styled.contains("unreceipted panic.unwrap at src/lib.rs:10:1\u{1b}"),
        "repository-controlled messages must stay unstyled"
    );
}

#[test]
fn render_why_text_sanitizes_repository_control_text() {
    let mut finding = sample_finding_at("src/\nlib.rs", 10);
    finding.message = "message\u{1b}[31m\nforged".to_string();
    finding.identity.container = Some("container\u{7f}".to_string());
    let mut entry = near_miss_entry();
    entry.id = "allow-\u{1b}[31m".to_string();
    entry.glob = Some("src/\n*.rs".to_string());
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "outcome\rmessage".to_string(),
        score: 0,
    };
    let text = render_why_text(
        &finding,
        &outcome,
        &[WhyCandidate {
            entry: &entry,
            reasons: vec!["reason\twith control".to_string()],
        }],
    );

    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\u{7f}'));
    assert!(text.contains("message\u{fffd}[31m\\nforged"));
    assert!(text.contains("message\u{fffd}[31m"));
    assert!(text.contains("src/\\nlib.rs"));
    assert!(text.contains("reason\\twith control"));
    assert!(text.contains("allow-\u{fffd}[31m"));
}

#[test]
fn render_why_text_does_not_make_hostile_proof_paths_pasteable() {
    let finding = sample_finding_at("src/\tlib.rs", 10);
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic.unwrap at src/lib.rs:10:1".to_string(),
        score: 0,
    };

    let text = render_why_text(&finding, &outcome, &[]);

    assert!(text.contains("not a pasteable shell command"));
    assert!(text.contains("structured argv / proof_plans JSON"));
    assert!(!text.contains("--path 'src/\\tlib.rs'") && !text.contains("--path src/\\tlib.rs"));
}

#[test]
fn render_why_json_deserializes_and_asserts_semantic_paths() {
    let finding = sample_finding_at("src/lib.rs", 10);
    let entry = near_miss_entry();
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic.unwrap at src/lib.rs:10:1".to_string(),
        score: 0,
    };
    let reasons = explain_match_failure(&entry, &finding);
    let json = render_why_json(
        allow_report::InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(3)),
        &finding,
        &outcome,
        &[WhyCandidate {
            entry: &entry,
            reasons,
        }],
    );
    let value = parse_why_json(&json);
    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some("cargo-allow.why.v1")
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some("why")
    );
    assert_eq!(
        value
            .pointer("/candidate_entries/0/id")
            .and_then(Value::as_str),
        Some("allow-near-miss")
    );
    assert!(
        value
            .pointer("/candidate_entries/0/mismatch_reasons/0")
            .and_then(Value::as_str)
            .is_some_and(|reason| reason.contains("callee mismatch"))
    );
    assert_eq!(
        value.pointer("/outcome/status").and_then(Value::as_str),
        Some("new")
    );
    assert_eq!(
        value
            .pointer("/evaluation/result_class")
            .and_then(Value::as_str),
        Some("exact_scoped")
    );
    assert_eq!(
        value
            .pointer("/next/proof_plans/0/program")
            .and_then(Value::as_str),
        Some("cargo-allow")
    );
    assert_eq!(
        path_arg_from_plan(&value, 0),
        Some("src/lib.rs".to_string())
    );
}

#[test]
fn render_why_json_defaults_missing_inventory_completeness_for_test_helper() {
    let finding = sample_finding_at("src/lib.rs", 10);
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic.unwrap at src/lib.rs:10:1".to_string(),
        score: 0,
    };
    let value = parse_why_json(&render_why_json(
        allow_report::InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(3)),
        &finding,
        &outcome,
        &[],
    ));
    assert_eq!(
        value
            .pointer("/evaluation/result_class")
            .and_then(Value::as_str),
        Some("exact_scoped")
    );
}

#[test]
fn proof_plans_preserve_argument_identity_for_hostile_paths() {
    let fixtures = [
        "src/ordinary.rs",
        "src/has space.rs",
        "src/quote'and\"double.rs",
        "src/$(touch pwned).rs",
        "src/a;echo injected.rs",
        "-leading-dash.rs",
        "src/line\nbreak.rs",
        "src/ユニコード.rs",
    ];
    for path in fixtures {
        let finding = sample_finding_at(path, 7);
        let outcome = MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            candidate_ids: Vec::new(),
            finding_index: Some(0),
            message: format!("unreceipted at {path}"),
            score: 0,
        };
        let expected = normalize_path(std::path::Path::new(path));
        let next = why_next_steps(&finding, &outcome, &[]);
        let add = next
            .proof_plans
            .first()
            .unwrap_or_else(|| std::panic::panic_any("new findings need an add plan"));
        let path_arg = add
            .args
            .iter()
            .position(|arg| arg == "--path")
            .and_then(|index| add.args.get(index + 1))
            .map(String::as_str);
        assert_eq!(
            path_arg,
            Some(expected.as_str()),
            "path fixture `{path}` must remain one argv element"
        );

        let json = render_why_json(
            allow_report::InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(1)),
            &finding,
            &outcome,
            &[],
        );
        let value = parse_why_json(&json);
        assert_eq!(
            path_arg_from_plan(&value, 0).as_deref(),
            Some(expected.as_str()),
            "JSON argv must keep exact path for `{path}`"
        );
        // Guidance must never execute payloads; we only assert structure.
        assert!(
            !std::path::Path::new("pwned").exists(),
            "fixture must not create sidecar files"
        );
    }
}

#[test]
fn render_why_points_matched_findings_to_explain() {
    let finding = sample_finding_at("src/lib.rs", 10);
    let outcome = MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-0007".to_string()),
        candidate_ids: vec!["allow-0007".to_string()],
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 200,
    };
    let text = render_why_text(&finding, &outcome, &[]);
    assert!(
        text.starts_with("# Why this finding is receipted\n"),
        "matched findings must open with the receipted heading, got {}",
        text.lines().next().unwrap_or_default()
    );
    assert!(text.contains("Already receipted"));
    assert!(text.contains("cargo-allow explain allow-0007"));
}

#[test]
fn render_why_heading_tracks_the_typed_outcome() {
    let finding = sample_finding_at("src/lib.rs", 10);
    let outcome_for = |status: MatchStatus, allow_id: Option<&str>| MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "fixture".to_string(),
        score: 0,
    };

    // Genuinely new findings keep the unreceipted heading.
    let text = render_why_text(&finding, &outcome_for(MatchStatus::New, None), &[]);
    assert!(text.starts_with("# Why this finding is unreceipted\n"));

    // Matched findings must never claim unreceipted.
    let text = render_why_text(
        &finding,
        &outcome_for(MatchStatus::Matched, Some("allow-0007")),
        &[],
    );
    assert!(text.starts_with("# Why this finding is receipted\n"));

    // Non-clean outcomes stay neutral: they claim neither an unreceipted gap
    // nor a satisfied receipt.
    for status in [
        MatchStatus::Stale,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::LocationDrift,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::MissingRequiredField,
        MatchStatus::EvidenceMissing,
        MatchStatus::BaselineDebt,
    ] {
        let text = render_why_text(&finding, &outcome_for(status, Some("allow-0007")), &[]);
        assert!(
            text.starts_with("# Why this finding has its current status\n"),
            "status {status:?} should use the neutral heading, got {}",
            text.lines().next().unwrap_or_default()
        );
    }
}

#[test]
fn ambiguous_why_emits_explain_plan_for_every_candidate_id() {
    let finding = sample_finding_at("src/lib.rs", 10);
    let outcome = MatchOutcome {
        status: MatchStatus::Ambiguous,
        allow_id: None,
        candidate_ids: vec!["allow-a".to_string(), "allow-b".to_string()],
        finding_index: Some(0),
        message: "ambiguous".to_string(),
        score: 0,
    };
    let next = why_next_steps(&finding, &outcome, &[]);
    let explain_ids: Vec<&str> = next
        .proof_plans
        .iter()
        .filter(|plan| plan.args.first().map(String::as_str) == Some("explain"))
        .filter_map(|plan| plan.args.get(1).map(String::as_str))
        .collect();
    assert_eq!(explain_ids, vec!["allow-a", "allow-b"]);

    let json = render_why_json(
        allow_report::InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(1)),
        &finding,
        &outcome,
        &[],
    );
    let value = parse_why_json(&json);
    assert_eq!(
        value.pointer("/outcome/status").and_then(Value::as_str),
        Some("ambiguous")
    );
    assert_eq!(
        value
            .pointer("/next/proof_plans/0/args/1")
            .and_then(Value::as_str),
        Some("allow-a")
    );
    assert_eq!(
        value
            .pointer("/next/proof_plans/1/args/1")
            .and_then(Value::as_str),
        Some("allow-b")
    );
    let text = render_why_text(&finding, &outcome, &[]);
    assert!(text.contains("allow-a"));
    assert!(text.contains("allow-b"));
    assert!(text.contains("no candidate is selected as authoritative"));
}

fn sample_finding_at(path: &str, line: u32) -> Finding {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some("load".to_string());
    identity.callee = Some("unwrap".to_string());
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 1 }),
        identity,
        message: "unwrap call".to_string(),
        ledger: None,
    }
}

fn near_miss_entry() -> AllowEntry {
    AllowEntry {
        id: "allow-near-miss".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "near miss fixture".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("expect".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

fn parse_why_json(json: &str) -> Value {
    serde_json::from_str(json).unwrap_or_else(|err| {
        std::panic::panic_any(format!("why JSON should deserialize: {err}\n{json}"))
    })
}

fn path_arg_from_plan(value: &Value, plan_index: usize) -> Option<String> {
    let args = value.pointer(&format!("/next/proof_plans/{plan_index}/args"))?;
    let arr = args.as_array()?;
    let path_index = arr
        .iter()
        .position(|item| item.as_str() == Some("--path"))?;
    arr.get(path_index + 1)?.as_str().map(str::to_string)
}

#[test]
fn why_args_round_trip_through_root_args() {
    let args = WhyArgs {
        root: RootArgs { root: None },
        config: None,
        kind: "unsafe".to_string(),
        path: PathBuf::from("crates/foo/src/lib.rs"),
        line: 3,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: Some(PathBuf::from("target/why.md")),
        plan: Some(PathBuf::from("target/add-plan.json")),
    };
    assert_eq!(args.line, 3);
    assert!(args.include_untracked);
    assert_eq!(args.plan, Some(PathBuf::from("target/add-plan.json")));
}

#[test]
fn target_scan_renderers_report_partial_scope_without_a_finding() -> Result<(), String> {
    let inventory = allow_report::InventoryContext::source_syntax("git_tracked", None, None)
        .with_completeness("complete");
    let evaluation = EvaluationContext {
        scope: "scoped",
        locality: "proven",
        reasons: &[],
    };
    let json = render_why_target_scan_json(
        inventory,
        evaluation,
        "src/large.rs",
        "skipped",
        Some("file exceeds scanner limit"),
    );
    let value: Value = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    for (pointer, expected) in [
        ("/evaluation/result_class", "target_scanner_partial"),
        ("/evaluation/scanner_completeness", "partial"),
        ("/target/path", "src/large.rs"),
        ("/target/status", "skipped"),
        ("/target/reason", "file exceeds scanner limit"),
    ] {
        if value.pointer(pointer).and_then(Value::as_str) != Some(expected) {
            return Err(format!("{pointer} did not equal {expected}: {value}"));
        }
    }
    for pointer in ["/finding", "/outcome"] {
        if !value.pointer(pointer).is_some_and(Value::is_null) {
            return Err(format!("{pointer} should be null: {value}"));
        }
    }
    let proof_plans = value.pointer("/next/proof_plans").and_then(Value::as_array);
    let proof_plans_missing_or_nonempty = match proof_plans {
        None => true,
        Some(plans) => !plans.is_empty(),
    };
    if proof_plans_missing_or_nonempty {
        return Err(format!(
            "partial target should not emit proof plans: {value}"
        ));
    }

    let text = render_why_target_scan_text(
        evaluation,
        inventory,
        "src/large.rs",
        "skipped",
        Some("file exceeds scanner limit"),
    );
    for expected in [
        "src/large.rs",
        "status: skipped",
        "reason: file exceeds scanner limit",
        "result_class: target_scanner_partial",
        "No finding was selected",
    ] {
        if !text.contains(expected) {
            return Err(format!("human output missed {expected:?}: {text}"));
        }
    }
    Ok(())
}

fn preflight_args(plan: Option<PathBuf>, output: Option<PathBuf>) -> WhyArgs {
    WhyArgs {
        root: RootArgs { root: None },
        config: None,
        kind: "panic".to_string(),
        path: PathBuf::from("src/lib.rs"),
        line: 1,
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output,
        plan,
    }
}

static WHY_LINE_FIXTURE_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// A minimal git fixture whose single panic finding sits on line 1 of
/// src/lib.rs, for requested-vs-matched line behavior (#4364).
fn why_line_fixture() -> PathBuf {
    let count = WHY_LINE_FIXTURE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-why-line-{}-{count}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        "policy = \"cargo-allow\"\n\n[workspace]\nignored = []\ngenerated = []\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn probe() { let value: Option<u8> = None; let _ = value.unwrap(); }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source write: {err}")));
    for args in [
        vec!["init", "--template="],
        vec!["config", "user.email", "cargo-allow@example.invalid"],
        vec!["config", "user.name", "cargo-allow why line test"],
        vec!["add", "policy/allow.toml", "src/lib.rs"],
        vec!["commit", "-m", "why line fixture"],
    ] {
        let status = std::process::Command::new("git")
            .args(&args)
            .current_dir(&root)
            .status()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
        if !status.success() {
            std::panic::panic_any(format!("git command failed: {args:?}"));
        }
    }
    root
}

fn why_line_args(root: &std::path::Path, line: u32) -> WhyArgs {
    WhyArgs {
        root: RootArgs {
            root: Some(root.to_path_buf()),
        },
        config: None,
        kind: "panic".to_string(),
        path: PathBuf::from("src/lib.rs"),
        line,
        include_untracked: false,
        format: HumanJsonFormat::Json,
        output: None,
        plan: None,
    }
}

#[test]
fn why_refuses_to_plan_a_typoed_line_and_signals_the_mismatch() {
    let root = why_line_fixture();
    let plan_path = root.join("target/add-finding-plan.json");
    let mut args = why_line_args(&root, 999);
    args.plan = Some(plan_path.clone());

    let err = cmd_why(&args).expect_err("a mismatched requested line must not produce a plan");
    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Usage);
    let message = err.to_string();
    assert!(
        message.contains("did not exactly match"),
        "the refusal should name the mismatch: {message}"
    );
    assert!(
        message.contains("src/lib.rs:1"),
        "the refusal should name the actually selected finding: {message}"
    );
    assert!(
        message.contains("--line 1"),
        "the refusal should carry the executable correction: {message}"
    );
    assert!(
        !plan_path.exists(),
        "no plan artifact may be written for a mismatched line"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn why_json_echoes_requested_vs_matched_line() {
    let root = why_line_fixture();

    let mismatched_output = root.join("why-999.json");
    let mut args = why_line_args(&root, 999);
    args.output = Some(mismatched_output.clone());
    cmd_why(&args).unwrap_or_else(|err| std::panic::panic_any(format!("why 999: {err}")));
    let value = parse_why_json(
        &fs::read_to_string(&mismatched_output)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read why 999: {err}"))),
    );
    assert_eq!(
        value.pointer("/line_targeting/requested_line").cloned(),
        Some(serde_json::json!(999)),
        "the requested line must be echoed on mismatch: {value}"
    );
    assert_eq!(
        value.pointer("/line_targeting/matched_line").cloned(),
        Some(serde_json::json!(1)),
        "the matched finding's line must be echoed on mismatch: {value}"
    );

    let exact_output = root.join("why-1.json");
    let mut args = why_line_args(&root, 1);
    args.output = Some(exact_output.clone());
    cmd_why(&args).unwrap_or_else(|err| std::panic::panic_any(format!("why 1: {err}")));
    let value = parse_why_json(
        &fs::read_to_string(&exact_output)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read why 1: {err}"))),
    );
    assert!(
        value.get("line_targeting").is_none(),
        "an exact-line why result must not grow a line_targeting field: {value}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn why_human_text_names_the_requested_line_on_mismatch() {
    let finding = sample_finding_at("src/lib.rs", 15);
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic.unwrap at src/lib.rs:15:1".to_string(),
        score: 0,
    };
    let evaluation = EvaluationContext {
        scope: "scoped",
        locality: "proven",
        reasons: &[],
    };

    let mismatched = render_why_text_styled_with_evaluation_and_scanner_completeness(
        allow_report::InventoryContext::source_syntax("git_tracked", None, None)
            .with_completeness("complete"),
        &finding,
        &outcome,
        &[],
        allow_report::Style::PLAIN,
        evaluation,
        Some("complete"),
        Some(allow_report::WhyLineTargeting {
            requested: 999,
            matched: 15,
        }),
    );
    assert!(
        mismatched.contains("requested_line: 999"),
        "human output should echo the requested line: {mismatched}"
    );
    assert!(
        mismatched.contains("the selected finding is at line 15"),
        "human output should name the matched line: {mismatched}"
    );

    let exact = render_why_text_styled_with_evaluation_and_scanner_completeness(
        allow_report::InventoryContext::source_syntax("git_tracked", None, None)
            .with_completeness("complete"),
        &finding,
        &outcome,
        &[],
        allow_report::Style::PLAIN,
        evaluation,
        Some("complete"),
        None,
    );
    assert!(
        !exact.contains("requested_line"),
        "an exact-line report must not grow a requested_line entry: {exact}"
    );
}
