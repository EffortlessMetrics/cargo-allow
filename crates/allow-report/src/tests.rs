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
fn explain_json_renderer_records_context_and_current_status() {
    let entry = AllowEntry {
        id: "allow-explain-json".to_string(),
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: Some(PathBuf::from("src\\ffi.rs")),
        glob: None,
        owner: "runtime".to_string(),
        classification: "ffi_boundary".to_string(),
        reason: "FFI pointer boundary requires unsafe.".to_string(),
        evidence: vec!["doc:docs/safety/ffi.md".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-27".to_string()),
            review_after: Some("2026-11-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("unsafe_block".to_string()),
            container: Some("read_byte".to_string()),
            callee: None,
            macro_name: None,
            lint: None,
            symbol: None,
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: Some("fnv1a64:unsafe".to_string()),
            line_hint: Some(9),
            glob: None,
        },
        last_seen: Some(LastSeen { line: 9, column: 5 }),
    };
    let mut identity = StructuralIdentity::new("rust", "unsafe_block");
    identity.crate_name = Some("runtime".to_string());
    identity.container = Some("read_byte".to_string());
    let finding = Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: PathBuf::from("src\\ffi.rs"),
        span: Some(Span { line: 9, column: 5 }),
        identity,
        message: "unsafe block".to_string(),
    };
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::EvidenceMissing,
        allow_id: Some("allow-explain-json".to_string()),
        finding_index: Some(0),
        message: "unsafe entry has missing evidence".to_string(),
        score: 9,
    }];
    let evidence_references = vec![EvidenceReference {
        raw: "doc:docs/safety/ffi.md",
        prefix: Some("doc"),
        target: Some("docs/safety/ffi.md"),
        status: "missing",
        message: "local evidence file is missing",
    }];
    let suggested_actions = vec!["add missing evidence".to_string()];
    let proof_commands = vec!["cargo-allow check --kind unsafe".to_string()];

    let report = ExplainReport {
        inventory: InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        entry: &entry,
        current_findings: &[finding],
        match_outcomes: &outcomes,
        evidence_references: &evidence_references,
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
    };

    let json = render_explain_json(report);

    assert!(json.contains("\"schema_id\": \"cargo-allow.explain.v1\""));
    assert!(json.contains("\"command\": \"explain\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"id\": \"allow-explain-json\""));
    assert!(json.contains("\"current_status\": \"evidence_missing\""));
    assert!(json.contains("\"current_matches\": 1"));
    assert!(json.contains("\"match_outcomes\": 1"));
    assert!(json.contains("\"raw\": \"doc:docs/safety/ffi.md\""));
    assert!(json.contains("\"target\": \"docs/safety/ffi.md\""));
    assert!(json.contains("\"status\": \"missing\""));
    assert!(json.contains("\"path\": \"src/ffi.rs\""));
    assert!(json.contains("\"source_package\": \"runtime\""));
    assert!(json.contains("\"score\": 9"));
    assert!(json.contains("\"add missing evidence\""));
    assert!(json.contains("\"cargo-allow check --kind unsafe\""));

    let text = render_explain_human(report);

    assert!(text.contains("allow-explain-json"));
    assert!(text.contains("kind: unsafe.unsafe_block"));
    assert!(text.contains("scope: src/ffi.rs"));
    assert!(text.contains("owner: runtime"));
    assert!(text.contains("classification: ffi_boundary"));
    assert!(text.contains("evidence references:"));
    assert!(text.contains(
            "- doc:docs/safety/ffi.md prefix=doc target=docs/safety/ffi.md status=missing message=local evidence file is missing"
        ));
    assert!(text.contains("current_status: evidence_missing"));
    assert!(text.contains("current_matches: 1"));
    assert!(
        text.contains("- evidence_missing: src/ffi.rs:9:5 (unsafe_block, source_package=runtime)")
    );
    assert!(text.contains("- evidence_missing: unsafe entry has missing evidence"));
    assert!(text.contains("- action: add missing evidence"));
    assert!(text.contains("- proof: cargo-allow check --kind unsafe"));
}

#[test]
fn doctor_json_renderer_records_root_config_and_inventory() {
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        inventory_source: "git_tracked",
        files_scanned: 50,
    });

    assert!(json.contains("\"schema_id\": \"cargo-allow.doctor.v1\""));
    assert!(json.contains("\"command\": \"doctor\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"discovery\": \"nearest_git_root\""));
    assert!(json.contains("\"found\": true"));
    assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow/policy/allow.toml\""));
    assert!(json.contains("\"scanner\": \"source_syntax\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 50"));
}

#[test]
fn doctor_human_renderer_records_root_config_and_inventory() {
    let text = render_doctor_human(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: None,
        inventory_source: "filesystem_fallback",
        files_scanned: 7,
    });

    assert!(text.contains("source tree root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("root discovery: nearest_git_root"));
    assert!(text.contains("config: not found; run `cargo-allow init`"));
    assert!(text.contains(
        "inventory: source_tree/source_syntax via filesystem_fallback; files scanned: 7"
    ));
    assert!(text.contains("did not invoke Cargo metadata"));
}

#[test]
fn propose_json_renderer_records_options_summary_and_defaults() {
    let report = ProposeReport {
        inventory: InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        kind: Some("panic"),
        expires: "2026-08-02",
        policy_output: Some("target/cargo-allow/proposed.toml"),
        force: true,
        findings_scanned: 54,
        baseline_debt_entries_proposed: 2,
    };

    let json = render_propose_json(report);

    assert!(json.contains("\"schema_id\": \"cargo-allow.propose.v1\""));
    assert!(json.contains("\"command\": \"propose\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"expires\": \"2026-08-02\""));
    assert!(json.contains("\"policy_output\": \"target/cargo-allow/proposed.toml\""));
    assert!(json.contains("\"force\": true"));
    assert!(json.contains("\"findings_scanned\": 54"));
    assert!(json.contains("\"baseline_debt_entries_proposed\": 2"));
    assert!(json.contains("\"owner\": \"unowned\""));
    assert!(json.contains("\"classification\": \"baseline_debt\""));

    let text = render_propose_human(report);

    assert!(text.contains("cargo-allow propose summary"));
    assert!(text.contains("findings scanned: 54"));
    assert!(text.contains("baseline_debt entries proposed: 2"));
    assert!(text.contains("owner: unowned"));
    assert!(text.contains("classification: baseline_debt"));
    assert!(text.contains("expires: 2026-08-02"));
    assert!(text.contains("output: target/cargo-allow/proposed.toml"));
    assert!(text.contains("generated debt still requires human review"));
}

#[test]
fn migrate_json_renderer_records_io_summary_and_notes() {
    let report = MigrateReport {
        inventory: InventoryContext::new(
            "source_tree",
            "policy_migration",
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        input_kind: "repo_policy",
        input_path: "policy",
        output_path: "policy/allow.toml",
        force: true,
        allow_entries: 12,
        baseline_debt: 5,
        unsafe_entries: 2,
        entries_with_evidence: 3,
        notes: "migration notes",
    };

    let json = render_migrate_json(report);

    assert!(json.contains("\"schema_id\": \"cargo-allow.migrate.v1\""));
    assert!(json.contains("\"command\": \"migrate\""));
    assert!(json.contains("\"scanner\": \"policy_migration\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"kind\": \"repo_policy\""));
    assert!(json.contains("\"path\": \"policy\""));
    assert!(json.contains("\"path\": \"policy/allow.toml\""));
    assert!(json.contains("\"force\": true"));
    assert!(json.contains("\"allow_entries\": 12"));
    assert!(json.contains("\"baseline_debt\": 5"));
    assert!(json.contains("\"unsafe_entries\": 2"));
    assert!(json.contains("\"entries_with_evidence\": 3"));
    assert!(json.contains("\"notes\": \"migration notes\""));

    let text = render_migrate_human(report);

    assert!(text.contains("cargo-allow migrate summary"));
    assert!(text.contains("input_kind: repo_policy"));
    assert!(text.contains("input: policy"));
    assert!(text.contains("output: policy/allow.toml"));
    assert!(text.contains("force: true"));
    assert!(text.contains("allow_entries: 12"));
    assert!(text.contains("baseline_debt: 5"));
    assert!(text.contains("unsafe_entries: 2"));
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("inventory_source: git_tracked"));
    assert!(text.contains("files_scanned: 76"));
    assert!(text.contains("migration notes"));
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
fn sarif_report_emits_non_matched_results_with_locations() {
    let findings = vec![file_finding(
        FindingKind::NonRustFile,
        "shell_script",
        "scripts/new.sh",
    )];
    let outcomes = vec![
        outcome(MatchStatus::Matched, Some(0)),
        MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: Some(0),
            message: "unreceipted shell script at scripts/new.sh".to_string(),
            score: 0,
        },
    ];

    let sarif =
        render_sarif_with_context("check", &findings, &outcomes, true, context("git_tracked"));

    assert!(sarif.contains("\"version\": \"2.1.0\""));
    assert!(sarif.contains("\"name\": \"cargo-allow\""));
    assert!(sarif.contains("\"ruleId\": \"cargo-allow/new\""));
    assert!(sarif.contains("\"level\": \"error\""));
    assert!(sarif.contains("\"uri\": \"scripts/new.sh\""));
    assert!(sarif.contains("\"startLine\": 1"));
    assert!(sarif.contains("\"source_tree_inventory\""));
    assert!(sarif.contains("\"cargo_commands_not_invoked\""));
    assert!(!sarif.contains("\"ruleId\": \"cargo-allow/matched\""));
}

#[test]
fn sarif_result_properties_include_source_package_context() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.crate_name = Some("parser".to_string());
    let findings = vec![Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("crates/parser/src/lib.rs"),
        span: Some(Span { line: 4, column: 9 }),
        identity,
        message: "unwrap call".to_string(),
    }];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        finding_index: Some(0),
        message: "unreceipted unwrap".to_string(),
        score: 0,
    }];

    let sarif = render_sarif("check", &findings, &outcomes, true);

    assert!(sarif.contains("\"source_package\": \"parser\""));
    assert!(sarif.contains("\"uri\": \"crates/parser/src/lib.rs\""));
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
fn html_report_summarizes_audit_posture() {
    let findings = vec![file_finding(
        FindingKind::NonRustFile,
        "shell_script",
        "scripts/new.sh",
    )];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        finding_index: Some(0),
        message: "unreceipted shell script at scripts/new.sh".to_string(),
        score: 0,
    }];

    let html =
        render_html_with_context("audit", &findings, &outcomes, true, context("git_tracked"));

    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("<h1>cargo-allow audit</h1>"));
    assert!(html.contains("Result: failed"));
    assert!(html.contains("<h2>Audit Summary</h2>"));
    assert!(html.contains("<h2>Non-Rust File Inventory</h2>"));
    assert!(html.contains("<code>new</code>"));
    assert!(html.contains("<code>scripts/new.sh</code>"));
    assert!(html.contains("did not invoke Cargo metadata"));
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
