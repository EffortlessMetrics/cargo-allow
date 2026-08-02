use super::test_support::test_finding_at_line;
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat, RootArgs};
use allow_core::{AllowConfig, AllowEntry, CargoAllowErrorKind, MatchOutcome, MatchStatus};
use allow_policy::render_policy;
use clap::Parser;
use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn clap_parses_add_from_finding() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "add",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "42",
        "--owner",
        "parser",
        "--reason",
        "validated invariant",
        "--evidence",
        "test:parser_invariant",
        "--write",
        "policy/allow.proposed.toml",
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        "target/add-summary.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Add(AddArgs {
            kind,
            path: Some(path),
            line: Some(42),
            glob: _,
            family: _,
            callee: _,
            owner,
            reason,
            evidence,
            write: Some(write),
            force: true,
            summary_format: HumanJsonFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if kind.as_deref() == Some("panic")
            && path.as_path() == Path::new("src/lib.rs")
            && owner == "parser"
            && reason == "validated invariant"
            && evidence == vec!["test:parser_invariant".to_string()]
            && write == Path::new("policy/allow.proposed.toml")
            && summary_output == Path::new("target/add-summary.json")
    ));
}

#[test]
fn select_add_finding_picks_nearest_path_and_kind() {
    let findings = vec![
        test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            10,
        ),
        test_finding_at_line(
            FindingKind::Panic,
            Some("expect"),
            "src/lib.rs",
            "method_call",
            40,
        ),
        test_finding_at_line(
            FindingKind::Unsafe,
            Some("unsafe_fn"),
            "src/lib.rs",
            "unsafe_fn",
            39,
        ),
    ];
    let kind = parse_kind_filter("panic")
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

    let (_index, selected) = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 39)
        .unwrap_or_else(|err| std::panic::panic_any(format!("finding should select: {err}")));

    assert_eq!(selected.family.as_deref(), Some("expect"));
    assert_eq!(selected.span.as_ref().map(|span| span.line), Some(40));
}

#[test]
fn select_add_finding_fails_closed_on_equal_nearest_findings() {
    let findings = vec![
        test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            40,
        ),
        test_finding_at_line(
            FindingKind::Panic,
            Some("expect"),
            "src/lib.rs",
            "method_call",
            42,
        ),
    ];
    let kind = parse_kind_filter("panic")
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

    let path = Path::new("src/lib.rs");
    let line = 41;
    let err = select_add_finding(&findings, kind, path, line)
        .expect_err("equally near findings should be ambiguous");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
}

#[test]
fn select_add_finding_reports_missing_nearby_findings() {
    let findings = vec![test_finding_at_line(
        FindingKind::Unsafe,
        Some("unsafe_fn"),
        "src/other.rs",
        "unsafe_fn",
        40,
    )];
    let kind = parse_kind_filter("panic")
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));
    let path = Path::new("src/lib.rs");
    let line = 39;

    let err = select_add_finding(&findings, kind, path, line)
        .expect_err("missing nearby panic finding should fail closed");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
}

#[test]
fn ensure_addable_outcome_rejects_already_matched_findings() {
    assert!(ensure_addable_outcome(MatchStatus::New).is_ok());

    let err = ensure_addable_outcome(MatchStatus::Matched)
        .expect_err("matched finding should not be addable");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
}

#[test]
fn allow_entry_from_finding_uses_structural_selector_and_review_metadata() {
    let mut finding = test_finding_at_line(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
        42,
    );
    finding.identity.container = Some("parse_span".to_string());
    finding.identity.callee = Some("unwrap".to_string());
    finding.identity.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

    let entry = allow_entry_from_finding(AddEntryRequest {
        finding: &finding,
        id: "allow-0099".to_string(),
        owner: "parser".to_string(),
        classification: "validated_invariant".to_string(),
        reason: "Parser validates the span before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_span".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: None,
    });

    assert_eq!(entry.id, "allow-0099");
    assert_eq!(entry.owner, "parser");
    assert_eq!(entry.selector.container.as_deref(), Some("parse_span"));
    assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
    assert_eq!(
        entry.selector.normalized_snippet_hash.as_deref(),
        Some("fnv1a64:1234")
    );
    assert_eq!(entry.last_seen.as_ref().map(|last| last.line), Some(42));
}

#[test]
fn add_evidence_requirement_covers_high_risk_policy_exceptions() {
    let unsafe_finding = test_finding_at_line(
        FindingKind::Unsafe,
        Some("unsafe_block"),
        "src/lib.rs",
        "unsafe_block",
        42,
    );
    let process_finding = test_finding_at_line(
        FindingKind::PolicyException,
        Some("process_spawn"),
        "src/lib.rs",
        "policy_exception",
        42,
    );
    let network_finding = test_finding_at_line(
        FindingKind::PolicyException,
        Some("network_destination"),
        "src/lib.rs",
        "policy_exception",
        42,
    );
    let workflow_finding = test_finding_at_line(
        FindingKind::PolicyException,
        Some("github_workflow"),
        ".github/workflows/ci.yml",
        "tracked_file",
        1,
    );

    let unsafe_err =
        require_add_evidence(&unsafe_finding, &[]).expect_err("unsafe add should require evidence");
    assert_eq!(unsafe_err.kind(), CargoAllowErrorKind::Unknown);
    let process_err = require_add_evidence(&process_finding, &[])
        .expect_err("process policy add should require evidence");
    assert_eq!(process_err.kind(), CargoAllowErrorKind::Unknown);
    let network_err = require_add_evidence(&network_finding, &[])
        .expect_err("network policy add should require evidence");
    assert_eq!(network_err.kind(), CargoAllowErrorKind::Unknown);
    assert!(
        require_add_evidence(&workflow_finding, &[]).is_ok(),
        "lower-risk policy exceptions can still be added without immediate evidence"
    );
}

#[test]
fn add_required_evidence_must_be_typed() {
    let process_finding = test_finding_at_line(
        FindingKind::PolicyException,
        Some("process_spawn"),
        "src/lib.rs",
        "policy_exception",
        42,
    );
    let weak_references = vec![
        "manual review note".to_string(),
        "spreadsheet:manual-review".to_string(),
        "test:".to_string(),
    ];

    let err = require_add_evidence(&process_finding, &weak_references)
        .expect_err("weak evidence should not satisfy high-risk add evidence gate");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
    assert!(
        require_add_evidence(
            &process_finding,
            &["test:process_spawn_is_guarded".to_string()]
        )
        .is_ok(),
        "recognized non-empty evidence references should satisfy the gate"
    );
    assert!(
        require_add_evidence(
            &process_finding,
            &[
                "manual review note".to_string(),
                "doc:docs/policy/process-spawn.md".to_string()
            ]
        )
        .is_ok(),
        "one typed reference should satisfy the gate even if another retained reference is weak"
    );
}

#[test]
fn default_add_review_after_is_relative_to_current_date() {
    let before = allow_core::SimpleDate::today_utc_approx().add_days(ADD_REVIEW_AFTER_DEFAULT_DAYS);
    let review_after = default_add_review_after();
    let after = allow_core::SimpleDate::today_utc_approx().add_days(ADD_REVIEW_AFTER_DEFAULT_DAYS);
    let parsed = allow_core::SimpleDate::parse(&review_after)
        .unwrap_or_else(|| std::panic::panic_any("default review_after should be a valid date"));

    assert!(
        before <= parsed && parsed <= after,
        "default add review_after should stay relative to the current UTC date"
    );
}

#[test]
fn selected_add_outcome_errors_when_finding_index_missing() {
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Stale,
        allow_id: Some("allow-stale".to_string()),
        candidate_ids: Vec::new(),
        finding_index: None,
        message: "allow-stale is stale".to_string(),
        score: 0,
    }];

    let err =
        selected_add_outcome(&outcomes, 0).expect_err("missing finding_index should fail closed");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
}

#[test]
fn cmd_add_rejects_duplicate_allow_id() {
    let root = add_fixture_dir();
    write_add_fixture_with_new_panic_finding(&root);
    let output = root.join("policy/allow.added.toml");
    let duplicate_id = "allow-0001".to_string();

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: Some(duplicate_id.clone()),
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: Some(output.clone()),
        force: false,
        update: false,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("add should reject duplicate allow ids");

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(
        !output.exists(),
        "add should not write policy output when id validation fails"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_rejects_already_matched_finding() {
    let root = add_fixture_dir();
    write_add_fixture_with_matched_panic_finding(&root);
    let output = root.join("policy/allow.added.toml");

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: Some(output.clone()),
        force: false,
        update: false,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("add should reject already matched findings");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
    assert!(
        !output.exists(),
        "add should not write policy output when match status blocks add"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_rejects_untracked_local_evidence_by_default() {
    let root = add_fixture_dir();
    write_add_fixture_with_untracked_evidence(&root);
    let output = root.join("policy/allow.added.toml");

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: Some(output.clone()),
        force: false,
        update: false,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("add should reject retained untracked local evidence by default");

    assert!(
        err.to_string()
            .contains("not in the default source-tree inventory"),
        "diagnostic should explain source-tree evidence boundary: {err}"
    );
    assert!(
        !output.exists(),
        "add should not write policy output when evidence validation fails"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_include_untracked_accepts_untracked_local_evidence() {
    let root = add_fixture_dir();
    write_add_fixture_with_untracked_evidence(&root);
    let output = root.join("policy/allow.added.toml");

    cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: true,
        write: Some(output.clone()),
        force: false,
        update: false,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "add should accept include-untracked evidence: {err}"
        ))
    });

    let rendered = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read output policy: {err}")));
    assert!(rendered.contains("allow-0002"));
    assert!(rendered.contains("test:parser_validates"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_reports_missing_policy_config_with_exact_error() {
    let root = add_fixture_dir();
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "src/lib.rs"]);
    git(&root, &["commit", "-m", "source only"]);
    let output = root.join("policy/allow.added.toml");

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: Some(output.clone()),
        force: false,
        update: false,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("add without policy config should fail at load_world");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
    assert!(
        !output.exists(),
        "add should not write policy output when load_world fails"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_validate_policy_rejects_unsupported_schema_version() {
    let root = add_fixture_dir();
    write_add_fixture_with_unsupported_schema_version(&root);
    let output = root.join("policy/allow.added.toml");
    let summary = root.join("target/add-error-summary.json");

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: Some(output.clone()),
        force: false,
        update: false,
        from_plan: None,
        summary_format: HumanJsonFormat::Json,
        summary_output: Some(summary.clone()),
    })
    .expect_err("add should fail closed when validate_policy rejects schema_version");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
    assert!(
        !output.exists(),
        "add should not write policy output when validate_policy fails"
    );
    assert!(
        !summary.exists(),
        "failed add should not write a success JSON summary"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_rejects_write_to_existing_output_without_force() {
    let root = add_fixture_dir();
    write_add_fixture_with_new_panic_finding(&root);
    let output = root.join("policy/allow.added.toml");
    fs::write(&output, "existing policy output")
        .unwrap_or_else(|err| std::panic::panic_any(format!("seed output policy: {err}")));

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: Some(output.clone()),
        force: false,
        update: false,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("add should reject overwriting an existing output file without --force");

    let message = err.to_string();
    assert!(
        message.contains("already exists; use --force to overwrite"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("allow.added.toml"),
        "unexpected error path in: {message}"
    );
    assert_eq!(
        fs::read_to_string(&output).unwrap_or_else(|read_err| std::panic::panic_any(format!(
            "read output policy: {read_err}"
        ))),
        "existing policy output"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_update_writes_entry_into_live_policy() {
    let root = add_fixture_dir();
    write_add_fixture_with_new_panic_finding(&root);
    let policy_path = root.join("policy/allow.toml");
    // Snapshot the existing entry so we can prove it survives the update.
    let before = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read before: {err}")));

    cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: None,
        force: false,
        update: true,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("add --update should write live policy: {err}"))
    });

    let after = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read after: {err}")));
    // New entry landed.
    assert!(
        after.contains("allow-0002") && after.contains("test:parser_validates"),
        "updated policy should contain the new entry"
    );
    // Existing entry preserved (allow-0001 block intact).
    assert!(
        before.lines().all(|line| after.contains(line)),
        "update must preserve every existing line"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_update_rejects_when_write_also_set() {
    let root = add_fixture_dir();
    write_add_fixture_with_new_panic_finding(&root);

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: Some(root.join("policy/allow.proposed.toml")),
        force: false,
        update: true,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("--update and --write together should be rejected");

    assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_update_requires_existing_policy() {
    let root = add_fixture_dir();
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "src/lib.rs"]);
    git(&root, &["commit", "-m", "source only"]);

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: None,
        force: false,
        update: true,
        from_plan: None,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("--update without a discovered policy should fail");

    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_add_update_json_summary_reports_discovered_target_and_written() {
    // Happy-path --update with `config: None`, exercising the advertised
    // discovered-policy flow (not a caller-supplied config path). Proves the
    // JSON summary reports the discovered target and `result=written` (#2413).
    let root = add_fixture_dir();
    write_add_fixture_with_new_panic_finding(&root);
    let policy_path = root.join("policy/allow.toml");
    let summary_path = root.join("add-summary.json");

    cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        kind: Some("panic".to_string()),
        glob: None,
        family: None,
        callee: None,
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates before unwrap.".to_string(),
        evidence: vec!["test:parser_validates".to_string()],
        id: None,
        review_after: Some("2026-11-01".to_string()),
        expires: None,
        include_untracked: false,
        write: None,
        force: false,
        update: true,
        from_plan: None,
        summary_format: HumanJsonFormat::Json,
        summary_output: Some(summary_path.clone()),
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("add --update should write live policy: {err}"))
    });

    let summary = fs::read_to_string(&summary_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read summary: {err}")));
    // The mutation receipt records a write, not the stdout fallback.
    assert!(
        summary.contains("\"result\": \"written\""),
        "update summary must report result=written: {summary}"
    );
    // Assert on the update-specific `policy_output` field specifically, not the
    // always-present `config_source`: extract that field's value so the check
    // cannot pass on config_source alone. It must name the discovered live
    // policy target (a real `.../policy/allow.toml` path), never `stdout`.
    let policy_output_value = summary
        .split("\"policy_output\": \"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("summary missing policy_output field: {summary}"))
        });
    assert_ne!(
        policy_output_value, "stdout",
        "discovered --update target must not fall back to stdout: {summary}"
    );
    assert!(
        policy_output_value.contains("policy") && policy_output_value.ends_with("allow.toml"),
        "policy_output must name the discovered policy target, got `{policy_output_value}`: {summary}"
    );

    // The discovered live policy actually received the new entry.
    let after = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read after: {err}")));
    assert!(
        after.contains("allow-0002") && after.contains("test:parser_validates"),
        "discovered --update flow should write the new entry into the live policy"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn clap_rejects_add_update_with_write() {
    // The --update/--write mutual exclusion is a Clap `conflicts_with`
    // contract, rejected at parse time (#2413).
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "add",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "1",
        "--owner",
        "parser",
        "--reason",
        "validated invariant",
        "--evidence",
        "test:parser_invariant",
        "--update",
        "--write",
        "policy/allow.proposed.toml",
    ]));

    let err = parsed.expect_err("--update and --write must conflict at parse time");
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

fn write_add_fixture_with_new_panic_finding(root: &std::path::Path) {
    write_add_git_fixture(
        root,
        r#"policy = "cargo-allow"

[[allow]]
id = "allow-0001"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-11-01"

[allow.selector]
ast_kind = "tracked_file"
"#,
    );
}

fn write_add_fixture_with_matched_panic_finding(root: &std::path::Path) {
    write_add_git_fixture(
        root,
        r#"policy = "cargo-allow"

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
evidence = ["test:parser_validates"]
review_after = "2026-11-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
    );
}

fn write_add_fixture_with_unsupported_schema_version(root: &std::path::Path) {
    write_add_git_fixture(
        root,
        r#"schema_version = "9.9"
policy = "cargo-allow"

[[allow]]
id = "allow-0001"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-11-01"

[allow.selector]
ast_kind = "tracked_file"
"#,
    );
}

fn write_add_git_fixture(root: &std::path::Path, policy: &str) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source write: {err}")));
    fs::write(root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "policy/allow.toml", "src/lib.rs"]);
    git(root, &["commit", "-m", "base policy"]);
}

fn write_add_fixture_with_untracked_evidence(root: &std::path::Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn parse(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source write: {err}")));
    let mut cfg = AllowConfig::empty();
    cfg.workspace.ignored = vec!["policy/evidence.md".to_string()];
    cfg.allow.push(test_policy_entry_with_untracked_evidence());
    fs::write(root.join("policy/allow.toml"), render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "policy/allow.toml", "src/lib.rs"]);
    git(root, &["commit", "-m", "base policy"]);
    fs::write(root.join("policy/evidence.md"), "untracked evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("evidence doc: {err}")));
}

fn test_policy_entry_with_untracked_evidence() -> AllowEntry {
    AllowEntry {
        id: "allow-0001".to_string(),
        kind: FindingKind::NonRustFile,
        family: Some("configuration".to_string()),
        path: Some(PathBuf::from("policy/allow.toml")),
        glob: None,
        owner: "core".to_string(),
        classification: "fixture".to_string(),
        reason: "fixture policy file".to_string(),
        evidence: vec!["doc:policy/evidence.md".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: allow_core::Lifecycle {
            created: None,
            review_after: Some("2026-11-01".to_string()),
            expires: None,
        },
        selector: allow_core::Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..allow_core::Selector::default()
        },
        last_seen: None,
    }
}

fn add_fixture_dir() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("cargo-allow-add-{}-{stamp}", std::process::id()));
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove add fixture {}: {err}", dir.display())),
    }
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create add fixture: {err}")));
    dir
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
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
