use super::test_support::test_finding_at_line;
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, RootArgs};
use allow_core::{AllowConfig, AllowEntry};
use allow_policy::render_policy;
use clap::Parser;
use std::fs;
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
            path,
            line: 42,
            owner,
            reason,
            evidence,
            write: Some(write),
            force: true,
            summary_format: AddSummaryFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if kind == "panic"
            && path == Path::new("src/lib.rs")
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

    let err = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 41)
        .expect_err("equally near findings should be ambiguous");

    assert!(err.to_string().contains("ambiguous add request"));
}

#[test]
fn ensure_addable_outcome_rejects_already_matched_findings() {
    assert!(ensure_addable_outcome(MatchStatus::New).is_ok());

    let err = ensure_addable_outcome(MatchStatus::Matched)
        .expect_err("matched finding should not be addable");

    assert!(err.to_string().contains("already receipted"));
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
        require_add_evidence(&unsafe_finding).expect_err("unsafe add should require evidence");
    assert!(
        unsafe_err
            .to_string()
            .contains("unsafe allow entries require at least one --evidence reference")
    );
    let process_err = require_add_evidence(&process_finding)
        .expect_err("process policy add should require evidence");
    assert!(process_err.to_string().contains(
        "policy_exception.process_spawn allow entries require at least one --evidence reference"
    ));
    let network_err = require_add_evidence(&network_finding)
        .expect_err("network policy add should require evidence");
    assert!(
        network_err.to_string().contains(
            "policy_exception.network_destination allow entries require at least one --evidence reference"
        )
    );
    assert!(
        require_add_evidence(&workflow_finding).is_ok(),
        "lower-risk policy exceptions can still be added without immediate evidence"
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
fn cmd_add_rejects_untracked_local_evidence_by_default() {
    let root = add_fixture_dir();
    write_add_fixture_with_untracked_evidence(&root);
    let output = root.join("policy/allow.added.toml");

    let err = cmd_add(&AddArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("policy/allow.toml")),
        kind: "panic".to_string(),
        path: PathBuf::from("src/lib.rs"),
        line: 1,
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
        summary_format: AddSummaryFormat::Human,
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
        kind: "panic".to_string(),
        path: PathBuf::from("src/lib.rs"),
        line: 1,
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
        summary_format: AddSummaryFormat::Human,
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

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
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
