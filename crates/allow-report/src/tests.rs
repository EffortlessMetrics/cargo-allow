use super::*;
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchOutcome, MatchStatus, Selector,
    Span, StructuralIdentity,
};
use std::path::PathBuf;

fn context(source: &'static str) -> ReportContext<'static> {
    ReportContext {
        inventory_source: source,
        ..ReportContext::default()
    }
}

#[test]
fn policy_and_finding_json_helpers_render_current_contract() {
    let entry = AllowEntry {
        id: "allow-json".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("crates\\parser\\src\\lib.rs")),
        glob: None,
        owner: "parser".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "generated baseline".to_string(),
        evidence: vec!["test:parser_handles_empty".to_string()],
        links: vec!["adr:docs/adr/0001.md".to_string()],
        occurrence_limit: Some(2),
        lifecycle: Lifecycle {
            created: Some("2026-05-27".to_string()),
            review_after: Some("2026-07-01".to_string()),
            expires: Some("2026-08-02".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("parse".to_string()),
            callee: Some("unwrap".to_string()),
            macro_name: None,
            lint: None,
            symbol: Some("value.unwrap()".to_string()),
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: Some("fnv1a64:test".to_string()),
            line_hint: Some(12),
            glob: None,
        },
        last_seen: Some(LastSeen {
            line: 12,
            column: 9,
        }),
    };
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.crate_name = Some("parser".to_string());
    identity.container = Some("parse".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("crates\\parser\\src\\lib.rs"),
        span: Some(Span {
            line: 12,
            column: 9,
        }),
        identity,
        message: "unwrap call".to_string(),
    };

    let entry_json = render_allow_entry_json(&entry, "  ");
    let finding_json = render_explain_finding_json(&finding, "selected", "  ");

    assert!(entry_json.contains("\"id\": \"allow-json\""));
    assert!(entry_json.contains("\"path\": \"crates/parser/src/lib.rs\""));
    assert!(entry_json.contains("\"occurrence_limit\": 2"));
    assert!(entry_json.contains("\"normalized_snippet_hash\": \"fnv1a64:test\""));
    assert!(entry_json.contains("\"line\": 12"));
    assert!(finding_json.contains("\"status\": \"selected\""));
    assert!(finding_json.contains("\"path\": \"crates/parser/src/lib.rs\""));
    assert!(finding_json.contains("\"source_package\": \"parser\""));
    assert!(finding_json.contains("\"container\": \"parse\""));
}

#[test]
fn json_contains_claim_boundary() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext {
            inventory_source: "filesystem_fallback",
            source_tree_root: Some("fixtures/source-snapshot"),
            inventory_files: Some(7),
            ..ReportContext::default()
        },
    );
    assert!(CLAIM_BOUNDARY.contains(&"source_tree_inventory"));
    assert!(SCANNER_LIMITATIONS.contains(&"cargo_metadata_not_invoked"));
    assert_eq!(CLAIM_BOUNDARY.len(), SCANNER_LIMITATIONS.len() + 2);
    assert!(json.contains("source_tree_inventory"));
    assert!(json.contains("cargo_metadata_not_invoked"));
    assert!(json.contains("cargo_commands_not_invoked"));
    assert!(json.contains("rustc_not_invoked"));
    assert!(json.contains("clippy_not_invoked"));
    assert!(json.contains("build_scripts_not_executed"));
    assert!(json.contains("proc_macros_not_executed"));
    assert!(json.contains("macro_expansion_not_analyzed"));
    assert!(json.contains("macro_token_tree_contents_not_analyzed"));
    assert!(json.contains("repository_code_not_executed"));
}

#[test]
fn json_report_exposes_v1_schema_contract() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext {
            inventory_source: "filesystem_fallback",
            source_tree_root: Some("fixtures/source-snapshot"),
            inventory_files: Some(7),
            ..ReportContext::default()
        },
    );
    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains("\"schema_id\": \"cargo-allow.report.v1\""));
    assert!(json.contains("\"failed\": false"));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"scope\": \"source_tree\""));
    assert!(json.contains("\"scanner\": \"source_syntax\""));
    assert!(json.contains("\"source\": \"filesystem_fallback\""));
    assert!(json.contains("\"root\": \"fixtures/source-snapshot\""));
    assert!(json.contains("\"files_scanned\": 7"));
    assert!(json.contains("\"review_due\": 0"));
    assert!(json.contains("\"baseline_debt\": 0"));
    assert!(json.contains("\"trend\""));
    assert!(json.contains("\"review_items\": 0"));
}

#[test]
fn json_report_exposes_trend_metrics() {
    let outcomes = vec![
        outcome(MatchStatus::New, Some(0)),
        outcome(MatchStatus::EvidenceMissing, Some(1)),
        outcome(MatchStatus::Stale, None),
    ];

    let json = render_json("audit", &[], &outcomes, false);

    assert!(json.contains("\"trend\""));
    assert!(json.contains("\"review_items\": 3"));
    assert!(json.contains("\"new\": 1"));
    assert!(json.contains("\"stale\": 1"));
    assert!(json.contains("\"evidence_missing\": 1"));
    assert!(json.contains("\"baseline_debt\": 0"));
}

#[test]
fn json_report_trend_counts_policy_baseline_debt_context() {
    let json = render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext {
            inventory_source: "git_tracked",
            baseline_debt_entries: Some(3),
            ..ReportContext::default()
        },
    );

    assert!(json.contains("\"review_items\": 3"));
    assert!(json.contains("\"baseline_debt\": 3"));
}

#[test]
fn json_report_exposes_source_package_context_on_findings() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.crate_name = Some("parser".to_string());
    let findings = vec![Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("crates/parser/src/lib.rs"),
        span: Some(Span {
            line: 12,
            column: 8,
        }),
        identity,
        message: "unwrap call".to_string(),
    }];

    let json = render_json("audit", &findings, &[], false);

    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"path\": \"crates/parser/src/lib.rs\""));
}

#[test]
fn receipt_exposes_v1_schema_contract() {
    let json = render_receipt_with_context(
        "check",
        &[],
        true,
        ReportContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(42),
            ..ReportContext::default()
        },
    );
    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains("\"schema_id\": \"cargo-allow.receipt.v1\""));
    assert!(json.contains("\"failed\": true"));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 42"));
    assert!(json.contains("\"cargo_metadata_not_invoked\""));
    assert!(json.contains("\"cargo_commands_not_invoked\""));
    assert!(json.contains("\"build_output_not_analyzed\""));
    assert!(json.contains("\"macro_token_tree_contents_not_analyzed\""));
    assert!(json.contains("\"missing_required_field\": 0"));
    assert!(json.contains("\"evidence_missing\": 0"));
}

#[test]
fn human_report_summarizes_non_rust_inventory() {
    let findings = vec![
        file_finding(FindingKind::NonRustFile, "configuration", ".gitignore"),
        file_finding(
            FindingKind::GeneratedCode,
            "generated_code",
            "schemas/api.yaml",
        ),
    ];
    let outcomes = vec![
        outcome(MatchStatus::Matched, Some(0)),
        outcome(MatchStatus::New, Some(1)),
    ];

    let text = render_human_with_context(
        "audit",
        &findings,
        &outcomes,
        false,
        ReportContext {
            inventory_source: "filesystem_fallback",
            source_tree_root: Some("fixtures/snapshot"),
            inventory_files: Some(2),
            ..ReportContext::default()
        },
    );

    assert!(text.contains(
        "Inventory: source_tree/source_syntax via filesystem_fallback; files scanned: 2"
    ));
    assert!(text.contains("Source tree root: fixtures/snapshot"));
    assert!(text.contains("Non-Rust file inventory:"));
    assert!(text.contains("files scanned              2"));
    assert!(text.contains("new                        1"));
    assert!(text.contains("generated                  1"));
    assert!(text.contains("configuration"));
    assert!(text.contains("generated_code"));
    assert!(text.contains("    matched      configuration            .gitignore"));
    assert!(text.contains("schemas/api.yaml"));
    assert!(text.contains("did not invoke Cargo metadata"));
    assert!(text.contains("repository code"));
}

#[test]
fn markdown_report_summarizes_non_rust_inventory() {
    let findings = vec![file_finding(
        FindingKind::NonRustFile,
        "ci_declarative",
        ".github/workflows/ci.yml",
    )];
    let outcomes = vec![outcome(MatchStatus::Matched, Some(0))];

    let text = render_markdown_with_context(
        "audit",
        &findings,
        &outcomes,
        false,
        ReportContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(1),
            ..ReportContext::default()
        },
    );

    assert!(text.contains(
        "Inventory: `source_tree` / `source_syntax` via `git_tracked`; files scanned: `1`"
    ));
    assert!(text.contains("Source tree root: `H:/Code/Rust/cargo-allow`"));
    assert!(text.contains("## Non-Rust File Inventory"));
    assert!(text.contains("| Files scanned | 1 |"));
    assert!(text.contains("| `ci_declarative` | 1 |"));
    assert!(text.contains("| `matched` | `ci_declarative` | `.github/workflows/ci.yml` |"));
    assert!(!text.contains("## Non-matched outcomes"));
    assert!(text.contains("did not invoke Cargo metadata"));
    assert!(text.contains("proc macros"));
}

#[test]
fn markdown_audit_report_includes_review_summary() {
    let findings = vec![
        file_finding(FindingKind::NonRustFile, "shell_script", "scripts/new.sh"),
        file_finding(FindingKind::Unsafe, "unsafe_block", "src/ffi.rs"),
    ];
    let outcomes = vec![
        MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: Some(0),
            message: "unreceipted shell script at scripts/new.sh".to_string(),
            score: 0,
        },
        MatchOutcome {
            status: MatchStatus::EvidenceMissing,
            allow_id: Some("allow-unsafe-ffi".to_string()),
            finding_index: Some(1),
            message: "allow-unsafe-ffi matched unsafe finding but has no evidence".to_string(),
            score: 0,
        },
    ];

    let text =
        render_markdown_with_context("audit", &findings, &outcomes, false, context("git_tracked"));

    assert!(text.contains("## Audit Summary"));
    assert!(text.contains("| Match outcomes | 2 |"));
    assert!(text.contains("| Review items | 2 |"));
    assert!(text.contains("| New unreceipted | 1 |"));
    assert!(text.contains("| Evidence gaps | 1 |"));
    assert!(
        text.contains("Recommended next step: review the queue below before tightening policy.")
    );
    assert!(text.contains("## Audit Review Queue"));
    assert!(text.contains("- `new`: unreceipted shell script at scripts/new.sh"));
    assert!(text.contains(
        "- `evidence_missing`: allow-unsafe-ffi matched unsafe finding but has no evidence"
    ));
}

#[test]
fn markdown_audit_report_counts_policy_baseline_debt_context() {
    let text = render_markdown_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext {
            inventory_source: "git_tracked",
            baseline_debt_entries: Some(3),
            ..ReportContext::default()
        },
    );

    assert!(text.contains("| Review items | 3 |"));
    assert!(text.contains("| Baseline debt | 3 |"));
    assert!(text.contains("cargo-allow worklist --format json"));
    assert!(!text.contains("## Audit Review Queue"));
}

#[test]
fn text_reports_include_review_due_and_invalid_selector_counts() {
    let outcomes = vec![
        MatchOutcome {
            status: MatchStatus::ReviewDue,
            allow_id: Some("allow-review".to_string()),
            finding_index: None,
            message: "allow-review is due for review".to_string(),
            score: 0,
        },
        MatchOutcome {
            status: MatchStatus::InvalidSelector,
            allow_id: Some("allow-invalid".to_string()),
            finding_index: None,
            message: "allow-invalid selector is invalid".to_string(),
            score: 0,
        },
    ];

    let human = render_human("check", &[], &outcomes, true);
    let markdown = render_markdown("check", &[], &outcomes, true);

    assert!(human.contains("review_due"));
    assert!(human.contains("invalid_selector"));
    assert!(markdown.contains("| `review_due` | 1 |"));
    assert!(markdown.contains("| `invalid_selector` | 1 |"));
}

fn file_finding(kind: FindingKind, family: &str, path: &str) -> Finding {
    Finding {
        kind,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", "tracked_file"),
        message: "tracked non-Rust file".to_string(),
    }
}

fn outcome(status: MatchStatus, finding_index: Option<usize>) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: None,
        finding_index,
        message: String::new(),
        score: 0,
    }
}
