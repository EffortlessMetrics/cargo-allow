use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
use std::path::PathBuf;

fn context(source: &'static str) -> ReportContext<'static> {
    ReportContext::source_syntax(source, None, None, None)
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
        ReportContext::source_syntax(
            "filesystem_fallback",
            Some("fixtures/snapshot"),
            Some(2),
            None,
        ),
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
    assert!(text.contains("external evidence tools"));
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
        ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(1),
            None,
        ),
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
    assert!(text.contains("external evidence tools"));
    assert!(text.contains("proc macros"));
}

#[test]
fn human_report_discloses_omitted_non_rust_inventory_rows() {
    let findings = (0..41)
        .map(|index| {
            file_finding(
                FindingKind::NonRustFile,
                "documentation",
                &format!("docs/{index}.md"),
            )
        })
        .collect::<Vec<_>>();
    let outcomes = (0..41)
        .map(|index| outcome(MatchStatus::Matched, Some(index)))
        .collect::<Vec<_>>();

    let text = render_human_with_context(
        "audit",
        &findings,
        &outcomes,
        false,
        ReportContext::source_syntax("git_tracked", None, Some(41), None),
    );

    assert!(text.contains("... 1 additional non-Rust file omitted from this listing"));
}

#[test]
fn markdown_report_discloses_omitted_non_rust_inventory_rows() {
    let findings = (0..61)
        .map(|index| {
            file_finding(
                FindingKind::NonRustFile,
                "documentation",
                &format!("docs/{index}.md"),
            )
        })
        .collect::<Vec<_>>();
    let outcomes = (0..61)
        .map(|index| outcome(MatchStatus::Matched, Some(index)))
        .collect::<Vec<_>>();

    let text = render_markdown_with_context(
        "audit",
        &findings,
        &outcomes,
        false,
        ReportContext::source_syntax("git_tracked", None, Some(61), None),
    );

    assert!(text.contains("1 additional non-Rust file omitted from this listing."));
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
fn human_audit_report_summarizes_source_exception_inventory() {
    let findings = vec![
        file_finding(FindingKind::Panic, "unwrap", "src/lib.rs"),
        file_finding(FindingKind::Unsafe, "unsafe_block", "src/ffi.rs"),
    ];
    let outcomes = vec![
        outcome(MatchStatus::Matched, Some(0)),
        outcome(MatchStatus::New, Some(1)),
    ];

    let text =
        render_human_with_context("audit", &findings, &outcomes, false, context("git_tracked"));

    assert!(text.contains("Source exception inventory:"));
    assert!(text.contains("findings                 2"));
    assert!(text.contains("panic"));
    assert!(text.contains("unsafe"));
    assert!(text.contains("panic.unwrap"));
    assert!(text.contains("unsafe.unsafe_block"));
    assert!(text.contains("total=1 matched=1 new=0 review_items=0"));
    assert!(text.contains("total=1 matched=0 new=1 review_items=1"));
}

#[test]
fn markdown_audit_report_summarizes_source_exception_inventory() {
    let findings = vec![
        file_finding(FindingKind::Panic, "unwrap", "src/lib.rs"),
        file_finding(FindingKind::Unsafe, "unsafe_block", "src/ffi.rs"),
    ];
    let outcomes = vec![
        outcome(MatchStatus::Matched, Some(0)),
        outcome(MatchStatus::New, Some(1)),
    ];

    let text =
        render_markdown_with_context("audit", &findings, &outcomes, false, context("git_tracked"));

    assert!(text.contains("## Source Exception Inventory"));
    assert!(text.contains("Findings inventoried: `2`"));
    assert!(text.contains("| `panic` | 1 | 1 | 0 | 0 |"));
    assert!(text.contains("| `unsafe` | 1 | 0 | 1 | 1 |"));
    assert!(text.contains("| `panic.unwrap` | 1 | 1 | 0 | 0 |"));
    assert!(text.contains("| `unsafe.unsafe_block` | 1 | 0 | 1 | 1 |"));
}

#[test]
fn human_audit_report_discloses_omitted_review_queue_items() {
    let outcomes = audit_review_queue_outcomes(21);

    let text = render_human_with_context("audit", &[], &outcomes, false, context("git_tracked"));

    assert!(text.contains("Audit review queue:"));
    assert!(text.contains("... 1 additional audit review item omitted from this queue"));
}

#[test]
fn markdown_audit_report_discloses_omitted_review_queue_items() {
    let outcomes = audit_review_queue_outcomes(21);

    let text = render_markdown_with_context("audit", &[], &outcomes, false, context("git_tracked"));

    assert!(text.contains("## Audit Review Queue"));
    assert!(text.contains("1 additional audit review item omitted from this queue."));
}

#[test]
fn human_report_discloses_omitted_non_matched_outcomes() {
    let outcomes = (0..81)
        .map(|index| MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: None,
            message: format!("new source exception {index}"),
            score: 0,
        })
        .collect::<Vec<_>>();

    let text = render_human_with_context("check", &[], &outcomes, true, context("git_tracked"));

    assert!(text.contains("... 1 additional non-matched outcome omitted from this listing"));
}

#[test]
fn markdown_report_discloses_omitted_non_matched_outcomes() {
    let outcomes = (0..101)
        .map(|index| MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: None,
            message: format!("new source exception {index}"),
            score: 0,
        })
        .collect::<Vec<_>>();

    let text = render_markdown_with_context("check", &[], &outcomes, true, context("git_tracked"));

    assert!(text.contains("1 additional non-matched outcome omitted from this listing."));
}

#[test]
fn human_audit_report_includes_review_summary() {
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
        render_human_with_context("audit", &findings, &outcomes, false, context("git_tracked"));

    assert!(text.contains("Audit summary:"));
    assert!(text.contains("match_outcomes"));
    assert!(text.contains("review_items"));
    assert!(text.contains("new_unreceipted"));
    assert!(text.contains("evidence_gaps"));
    assert!(
        text.contains("Recommended next step: review the queue below before tightening policy.")
    );
    assert!(text.contains("Audit review queue:"));
    assert!(text.contains("new: unreceipted shell script at scripts/new.sh"));
    assert!(
        text.contains(
            "evidence_missing: allow-unsafe-ffi matched unsafe finding but has no evidence"
        )
    );
}

#[test]
fn human_audit_report_routes_clean_policy_to_no_new_ci() {
    let text = render_human_with_context("audit", &[], &[], false, context("git_tracked"));

    assert!(text.contains("Audit summary:"));
    assert!(text.contains("review_items"));
    assert!(text.contains(" 0\n"));
    assert!(text.contains("Recommended next step: keep `cargo-allow check --mode no-new` in CI."));
    assert!(!text.contains("Audit review queue:"));
}

#[test]
fn markdown_audit_report_counts_policy_baseline_debt_context() {
    let text = render_markdown_with_context(
        "audit",
        &[],
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, Some(3)),
    );

    assert!(text.contains("| Review items | 3 |"));
    assert!(text.contains("| Baseline debt | 3 |"));
    assert!(text.contains("cargo-allow worklist --format json"));
    assert!(!text.contains("## Audit Review Queue"));
}

#[test]
fn markdown_audit_report_counts_broken_evidence_links_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(2);
    let text = render_markdown_with_context("audit", &[], &[], false, context);

    assert!(text.contains("| Review items | 2 |"));
    assert!(text.contains("| Broken evidence links | 2 |"));
    assert!(text.contains("worklist --item-kind broken_evidence_link --format json"));
    assert!(text.contains("repair broken local evidence/link references"));
    assert!(!text.contains("## Audit Review Queue"));
}

#[test]
fn markdown_audit_report_counts_weak_evidence_references_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.weak_evidence_references = Some(2);
    let text = render_markdown_with_context("audit", &[], &[], false, context);

    assert!(text.contains("| Review items | 2 |"));
    assert!(text.contains("| Weak evidence/link references | 2 |"));
    assert!(text.contains("replace unstructured or unknown-prefix evidence/link references"));
    assert!(!text.contains("## Audit Review Queue"));
}

#[test]
fn markdown_audit_report_counts_policy_missing_evidence_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.policy_missing_evidence_entries = Some(4);
    let text = render_markdown_with_context("audit", &[], &[], false, context);

    assert!(text.contains("| Review items | 4 |"));
    assert!(text.contains("| Policy missing evidence | 4 |"));
    assert!(text.contains("worklist --format json"));
    assert!(text.contains("add `--missing-evidence` to focus that queue"));
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

#[test]
fn check_text_reports_policy_baseline_debt_context() {
    let context = ReportContext::source_syntax("git_tracked", None, None, Some(3));
    let human = render_human_with_context("check", &[], &[], false, context);
    let markdown = render_markdown_with_context("check", &[], &[], false, context);

    assert!(human.contains("policy_baseline_debt"));
    assert!(human.contains("3"));
    assert!(markdown.contains("| `baseline_debt` | 0 |"));
    assert!(markdown.contains("| `policy_baseline_debt` | 3 |"));
}

#[test]
fn check_text_reports_broken_evidence_links_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(2);
    let human = render_human_with_context("check", &[], &[], false, context);
    let markdown = render_markdown_with_context("check", &[], &[], false, context);

    assert!(human.contains("broken_evidence_links"));
    assert!(human.contains("2"));
    assert!(markdown.contains("| `broken_evidence_links` | 2 |"));
}

#[test]
fn check_text_reports_weak_evidence_references_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.weak_evidence_references = Some(2);
    let human = render_human_with_context("check", &[], &[], false, context);
    let markdown = render_markdown_with_context("check", &[], &[], false, context);

    assert!(human.contains("weak_evidence_references"));
    assert!(human.contains("2"));
    assert!(markdown.contains("| `weak_evidence_references` | 2 |"));
}

#[test]
fn check_text_reports_policy_missing_evidence_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.policy_missing_evidence_entries = Some(4);
    let human = render_human_with_context("check", &[], &[], false, context);
    let markdown = render_markdown_with_context("check", &[], &[], false, context);

    assert!(human.contains("policy_missing_evidence"));
    assert!(human.contains("4"));
    assert!(markdown.contains("| `policy_missing_evidence` | 4 |"));
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

fn audit_review_queue_outcomes(count: usize) -> Vec<MatchOutcome> {
    (0..count)
        .map(|index| MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            finding_index: None,
            message: format!("new source exception {index}"),
            score: 0,
        })
        .collect()
}
