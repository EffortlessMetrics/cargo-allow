//! Lifecycle cadence artifact model tests (#4239): row construction and
//! identity retention, summary counts, deterministic ordering, and the
//! three renderings (JSON, human, markdown) from the one typed result.
//! The class-decision boundary tests live in the `cargo-allow` command
//! crate next to the classifier, which reuses the match-engine and loader
//! lifecycle predicates.

use allow_core::{FindingKind, Lifecycle, Selector, SimpleDate};
use std::path::PathBuf;

use crate::{
    CADENCE_SCHEMA_ID, CADENCE_SCHEMA_VERSION, EXPIRING_SOON_DAYS, InventoryContext,
    LifecycleCadenceAsOfSourceV1, LifecycleCadenceClassV1, LifecycleCadenceInventoryV1,
    LifecycleCadenceReportV1, LifecycleCadenceRowV1, REVIEW_DUE_SOON_DAYS,
    lifecycle_cadence_report, lifecycle_cadence_row, lifecycle_cadence_summary,
    render_lifecycle_cadence_report_human, render_lifecycle_cadence_report_markdown,
    render_lifecycle_cadence_report_v1, sort_lifecycle_cadence_rows,
};

const AS_OF: &str = "2026-10-01";

fn as_of() -> SimpleDate {
    SimpleDate::parse(AS_OF)
        .unwrap_or_else(|| std::panic::panic_any("fixed fixture as-of date must parse"))
}

fn entry(id: &str) -> allow_core::AllowEntry {
    allow_core::AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core/parser".to_string(),
        classification: "reviewed".to_string(),
        reason: "cadence fixture".to_string(),
        evidence: vec!["test:cadence_fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-06-01".to_string()),
            review_after: Some("2026-10-08".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn row(id: &str, class: LifecycleCadenceClassV1) -> LifecycleCadenceRowV1 {
    lifecycle_cadence_row(&entry(id), class, as_of())
}

fn sample_inventory() -> LifecycleCadenceInventoryV1 {
    LifecycleCadenceInventoryV1 {
        scope: "source_tree".to_string(),
        scanner: "source_syntax".to_string(),
        source: "git_tracked".to_string(),
        root: Some("fixture".to_string()),
        files_scanned: Some(3),
        empty_git_tracked: false,
        completeness: Some("complete".to_string()),
    }
}

fn sample_report(rows: Vec<LifecycleCadenceRowV1>) -> LifecycleCadenceReportV1 {
    lifecycle_cadence_report(
        as_of(),
        LifecycleCadenceAsOfSourceV1::Explicit,
        "policy/allow.toml",
        sample_inventory(),
        rows,
    )
}

#[test]
fn horizons_use_the_house_fourteen_day_values() {
    assert_eq!(REVIEW_DUE_SOON_DAYS, 14);
    assert_eq!(EXPIRING_SOON_DAYS, 14);
}

#[test]
fn required_dispositions_cover_every_class_from_one_mapping() {
    let expected = [
        (LifecycleCadenceClassV1::Current, "none"),
        (LifecycleCadenceClassV1::ReviewDueSoon, "schedule review"),
        (
            LifecycleCadenceClassV1::ReviewOverdue,
            "review now or narrow",
        ),
        (
            LifecycleCadenceClassV1::Expiring,
            "renew, narrow, or plan removal",
        ),
        (
            LifecycleCadenceClassV1::Expired,
            "renew via reviewed commit or remove",
        ),
        (
            LifecycleCadenceClassV1::Invalid,
            "fix malformed lifecycle date (fail-closed)",
        ),
    ];
    for (class, disposition) in expected {
        assert_eq!(class.required_disposition(), disposition, "{class}");
        let row = row("allow-x", class);
        assert_eq!(row.required_disposition, disposition, "{class}");
    }
}

#[test]
fn rows_retain_owner_source_evidence_and_exact_day_deltas() {
    let mut review_entry = entry("allow-identity");
    review_entry.owner = "runtime/scheduler".to_string();
    review_entry.path = Some(PathBuf::from("src/scheduler/mod.rs"));
    review_entry.evidence = vec!["review:docs/reviews/scheduler.md".to_string()];
    let review_row = lifecycle_cadence_row(
        &review_entry,
        LifecycleCadenceClassV1::ReviewDueSoon,
        as_of(),
    );
    assert_eq!(review_row.days_remaining, Some(7));
    assert_eq!(review_row.driving_date.as_str(), "review_after");
    assert_eq!(review_row.owner, "runtime/scheduler");
    assert_eq!(
        review_row.source_path.as_deref(),
        Some("src/scheduler/mod.rs")
    );
    assert_eq!(
        review_row.evidence_refs,
        vec!["review:docs/reviews/scheduler.md"]
    );
    assert!(review_row.selector_summary.contains("panic"));
    assert!(review_row.selector_summary.contains("unwrap"));
    assert_eq!(review_row.review_after.as_deref(), Some("2026-10-08"));
    assert_eq!(review_row.required_disposition, "schedule review");

    let mut overdue = entry("allow-overdue-identity");
    overdue.lifecycle.review_after = Some("2026-09-24".to_string());
    overdue.path = None;
    overdue.glob = Some("generated/**/*.rs".to_string());
    let overdue_row =
        lifecycle_cadence_row(&overdue, LifecycleCadenceClassV1::ReviewOverdue, as_of());
    assert_eq!(overdue_row.days_remaining, Some(-7));
    assert_eq!(
        overdue_row.source_glob.as_deref(),
        Some("generated/**/*.rs")
    );
    assert_eq!(overdue_row.required_disposition, "review now or narrow");

    let mut expiring = entry("allow-expiring-identity");
    expiring.lifecycle.review_after = None;
    expiring.lifecycle.expires = Some("2026-10-03".to_string());
    let expiring_row = lifecycle_cadence_row(&expiring, LifecycleCadenceClassV1::Expiring, as_of());
    assert_eq!(expiring_row.days_remaining, Some(2));
    assert_eq!(expiring_row.driving_date.as_str(), "expires");
    assert_eq!(
        expiring_row.required_disposition,
        "renew, narrow, or plan removal"
    );

    let mut expired = entry("allow-expired-identity");
    expired.lifecycle.review_after = None;
    expired.lifecycle.expires = Some("2026-09-21".to_string());
    let expired_row = lifecycle_cadence_row(&expired, LifecycleCadenceClassV1::Expired, as_of());
    assert_eq!(expired_row.days_remaining, Some(-10));
    assert_eq!(expired_row.driving_date.as_str(), "expires");
    assert_eq!(
        expired_row.required_disposition,
        "renew via reviewed commit or remove"
    );

    for undated in [
        LifecycleCadenceClassV1::Current,
        LifecycleCadenceClassV1::Invalid,
    ] {
        let undated_row = row("allow-undated", undated);
        assert_eq!(undated_row.days_remaining, None, "{undated}");
        assert_eq!(undated_row.driving_date.as_str(), "none", "{undated}");
    }
}

#[test]
fn summary_counts_and_deterministic_order_come_from_the_rows() {
    let rows = vec![
        row("allow-b", LifecycleCadenceClassV1::Current),
        row("allow-a", LifecycleCadenceClassV1::Current),
        row("allow-z", LifecycleCadenceClassV1::Expired),
        row("allow-y", LifecycleCadenceClassV1::Invalid),
        row("allow-m", LifecycleCadenceClassV1::ReviewDueSoon),
        row("allow-n", LifecycleCadenceClassV1::Expiring),
        row("allow-o", LifecycleCadenceClassV1::ReviewOverdue),
    ];
    let summary = lifecycle_cadence_summary(&rows);
    assert_eq!(summary.total_entries, 7);
    assert_eq!(summary.current, 2);
    assert_eq!(summary.review_due_soon, 1);
    assert_eq!(summary.review_overdue, 1);
    assert_eq!(summary.expiring, 1);
    assert_eq!(summary.expired, 1);
    assert_eq!(summary.invalid, 1);

    let sorted = sort_lifecycle_cadence_rows(rows);
    let order: Vec<&str> = sorted
        .iter()
        .map(|sorted_row| sorted_row.allow_id.as_str())
        .collect();
    assert_eq!(
        order,
        vec![
            "allow-y", "allow-z", "allow-n", "allow-o", "allow-m", "allow-a", "allow-b"
        ],
        "rows sort by class rank, then allow id"
    );
}

#[test]
fn same_rows_and_context_render_byte_identically_twice() {
    let build = || {
        sample_report(vec![
            row("allow-current", LifecycleCadenceClassV1::Current),
            row("allow-due-soon", LifecycleCadenceClassV1::ReviewDueSoon),
            row("allow-expired", LifecycleCadenceClassV1::Expired),
        ])
    };
    let first = build();
    let second = build();
    let json_first = render_lifecycle_cadence_report_v1(&first)
        .unwrap_or_else(|err| std::panic::panic_any(format!("render cadence JSON: {err}")));
    let json_second = render_lifecycle_cadence_report_v1(&second)
        .unwrap_or_else(|err| std::panic::panic_any(format!("render cadence JSON again: {err}")));
    assert_eq!(
        json_first, json_second,
        "same (policy, as_of) renders byte-identical JSON"
    );
    assert_eq!(first, second);
    assert_eq!(
        render_lifecycle_cadence_report_human(&first),
        render_lifecycle_cadence_report_human(&second)
    );
    assert_eq!(
        render_lifecycle_cadence_report_markdown(&first),
        render_lifecycle_cadence_report_markdown(&second)
    );
}

#[test]
fn human_rendering_carries_row_identities_and_summary() {
    // Fixture dates match each row's class so the driving-date deltas are
    // the ones a real classification would produce.
    let mut expired_entry = entry("allow-expired");
    expired_entry.lifecycle.review_after = None;
    expired_entry.lifecycle.expires = Some("2026-09-24".to_string());
    let report = sample_report(vec![
        row("allow-due-soon", LifecycleCadenceClassV1::ReviewDueSoon),
        lifecycle_cadence_row(&expired_entry, LifecycleCadenceClassV1::Expired, as_of()),
        row("allow-current", LifecycleCadenceClassV1::Current),
    ]);
    let human = render_lifecycle_cadence_report_human(&report);
    for fragment in [
        "cargo-allow cadence",
        "as_of: 2026-10-01 (explicit)",
        "allow-due-soon",
        "allow-expired",
        "allow-current",
        "[review_due_soon]",
        "[expired]",
        "[current]",
        "7 days remaining",
        "7 days overdue",
        "entries: 3",
        "schedule review",
        "renew via reviewed commit or remove",
        "never feeds check or diff verdicts",
    ] {
        assert!(
            human.contains(fragment),
            "human rendering must contain {fragment}"
        );
    }
}

#[test]
fn markdown_rendering_carries_the_same_rows_and_escapes_cells() {
    let mut rows = vec![
        row("allow-due-soon", LifecycleCadenceClassV1::ReviewDueSoon),
        row("allow-expired", LifecycleCadenceClassV1::Expired),
        row("allow-current", LifecycleCadenceClassV1::Current),
    ];
    // Policy-derived text can contain pipe characters; the markdown table
    // must escape it instead of breaking row structure.
    let mut piped = entry("allow-piped");
    piped.owner = "core/parse|r".to_string();
    rows.push(lifecycle_cadence_row(
        &piped,
        LifecycleCadenceClassV1::ReviewDueSoon,
        as_of(),
    ));

    let report = sample_report(rows);
    let markdown = render_lifecycle_cadence_report_markdown(&report);
    assert!(markdown.starts_with("# cargo-allow cadence\n"));
    assert!(markdown.contains("review_due_soon within 14 days; expiring within 14 days"));
    for report_row in &report.rows {
        let escaped_id = report_row.allow_id.replace('|', "\\|");
        assert!(
            markdown.contains(&format!("| {escaped_id} | {} |", report_row.class.as_str())),
            "markdown must carry row {} as {}",
            report_row.allow_id,
            report_row.class.as_str()
        );
    }
    assert!(
        markdown.contains("core/parse\\|r"),
        "markdown cells escape pipe characters"
    );
    assert!(markdown.contains("| total_entries | 4 |"));
    assert!(markdown.contains("never feeds check or diff verdicts"));
}

#[test]
fn cadence_schema_id_consts_are_stable() {
    assert_eq!(CADENCE_SCHEMA_ID, "cargo-allow.cadence.v1");
    assert_eq!(CADENCE_SCHEMA_VERSION, 1);
    assert!(crate::CADENCE_CLAIM_BOUNDARY.contains("never feeds check or diff verdicts"));
}

#[test]
fn report_assembly_carries_the_shared_artifact_context() {
    let report = sample_report(vec![row("allow-x", LifecycleCadenceClassV1::Current)]);
    assert_eq!(report.schema_id, "cargo-allow.cadence.v1");
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.tool, "cargo-allow");
    assert_eq!(report.command, "cadence");
    assert_eq!(report.as_of, "2026-10-01");
    assert_eq!(report.as_of_source.as_str(), "explicit");
    assert_eq!(report.horizons.review_due_soon_days, 14);
    assert_eq!(report.horizons.expiring_soon_days, 14);
    assert_eq!(report.policy_path, "policy/allow.toml");
    assert_eq!(report.inventory.scope, "source_tree");
    assert_eq!(report.inventory.completeness.as_deref(), Some("complete"));
    assert!(
        report
            .claim_boundary
            .iter()
            .any(|flag| flag == "source_tree_inventory"),
        "the artifact carries the shared claim-boundary flags"
    );
    assert!(
        report
            .scanner_limitations
            .iter()
            .any(|flag| flag == "repository_code_not_executed"),
        "the artifact carries the shared scanner-limitation flags"
    );
    let json = render_lifecycle_cadence_report_v1(&report)
        .unwrap_or_else(|err| std::panic::panic_any(format!("render cadence JSON: {err}")));
    assert!(
        json.contains("\"as_of_source\": \"explicit\""),
        "serde snake_case rendering: {json}"
    );
}

#[test]
fn inventory_context_converts_into_the_cadence_inventory() {
    let context = InventoryContext::source_syntax("filesystem_fallback", Some("snap"), Some(9))
        .with_completeness("scoped");
    let inventory = LifecycleCadenceInventoryV1::from(&context);
    assert_eq!(inventory.scope, "source_tree");
    assert_eq!(inventory.scanner, "source_syntax");
    assert_eq!(inventory.source, "filesystem_fallback");
    assert_eq!(inventory.root.as_deref(), Some("snap"));
    assert_eq!(inventory.files_scanned, Some(9));
    assert!(!inventory.empty_git_tracked);
    assert_eq!(inventory.completeness.as_deref(), Some("scoped"));
}
