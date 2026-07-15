use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, RootArgs};
use allow_core::{
    AllowEntry, Finding, FindingKind, Lifecycle, MatchOutcome, MatchStatus, Selector, Span,
    StructuralIdentity,
};
use clap::Parser;
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
        "--root",
        ".",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse why: {err}")));

    match parsed.command {
        Some(CargoAllowCommand::Why(args)) => {
            assert_eq!(args.kind, "panic");
            assert_eq!(args.path, PathBuf::from("src/lib.rs"));
            assert_eq!(args.line, 42);
            assert_eq!(args.root.root.as_deref(), Some(std::path::Path::new(".")));
        }
        other => std::panic::panic_any(format!("expected Why command, got {other:?}")),
    }
}

#[test]
fn render_why_lists_mismatch_reasons_for_new_findings() {
    let finding = sample_finding();
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
    assert!(text.contains("cargo-allow add --kind panic"));
    assert!(text.contains("Claim boundary"));
}

#[test]
fn render_why_points_matched_findings_to_explain() {
    let finding = sample_finding();
    let outcome = MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-0007".to_string()),
        candidate_ids: vec!["allow-0007".to_string()],
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 200,
    };
    let text = render_why_text(&finding, &outcome, &[]);
    assert!(text.contains("Already receipted"));
    assert!(text.contains("cargo-allow explain allow-0007"));
}

fn sample_finding() -> Finding {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some("load".to_string());
    identity.callee = Some("unwrap".to_string());
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 1,
        }),
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

#[test]
fn why_args_round_trip_through_root_args() {
    let args = WhyArgs {
        root: RootArgs { root: None },
        config: None,
        kind: "unsafe".to_string(),
        path: PathBuf::from("crates/foo/src/lib.rs"),
        line: 3,
        include_untracked: true,
        output: Some(PathBuf::from("target/why.md")),
    };
    assert_eq!(args.line, 3);
    assert!(args.include_untracked);
}
