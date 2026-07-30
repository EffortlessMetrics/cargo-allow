use super::*;
use crate::{InventoryContext, RefreshModeContext, RefreshReport, Style};
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity,
};

fn sample_mutation_receipt() -> crate::MutationReceipt<'static> {
    crate::MutationReceipt {
        operation: "refresh",
        tool_version: "0.1.10",
        repo_root: Some("tests/fixtures/refresh/advisory-drift"),
        config_source: Some("policy/allow.toml"),
        ledger_ids: Vec::new(),
        changed_allow_ids: vec!["fixture-refresh-drift"],
        before_fingerprints: vec![Some(
            "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        )],
        after_fingerprints: vec![Some(
            "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
        )],
        result: "stdout",
        next_commands: vec!["cargo-allow check --mode no-new".to_string()],
    }
}

fn sample_entry() -> AllowEntry {
    AllowEntry {
        id: "fixture-refresh-drift".to_string(),
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "lint".to_string(),
        classification: "reviewed_lint_exception".to_string(),
        reason: "Fixture refresh receipt".to_string(),
        evidence: vec!["test:refresh-receipt-fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: None,
        },
        selector: Selector::default(),
        last_seen: Some(LastSeen {
            line: 22,
            column: 4,
        }),
    }
}

fn sample_finding() -> Finding {
    Finding {
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: "src/lib.rs".into(),
        identity: StructuralIdentity::new("rust", "attribute"),
        message: "fixture".to_string(),
        span: Some(Span {
            line: 22,
            column: 4,
        }),
        ledger: None,
    }
}

#[test]
fn refresh_json_records_operator_approved_drift_refresh_metadata() {
    let json = render_refresh_json(RefreshReport::new(
        InventoryContext::source_syntax(
            "filesystem_include_untracked",
            Some("tests/fixtures/refresh/advisory-drift"),
            Some(2),
        ),
        &sample_entry(),
        &sample_finding(),
        Some(LastSeen {
            line: 14,
            column: 8,
        }),
        "allow-drift last_seen changed from 14:8 to 22:4",
        RefreshModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
        },
        sample_mutation_receipt(),
    ));

    assert!(json.contains(REFRESH_SCHEMA_ID));
    assert!(json.contains("\"lifecycle_preserved\": true"));
    assert!(json.contains("\"previous_last_seen\""));
    assert!(json.contains("\"refreshed_last_seen\""));
    assert!(json.contains("\"drift_message\""));
    assert!(json.contains("\"mutation_receipt\""));
    let parsed = serde_json::from_str::<serde_json::Value>(&json);
    assert!(parsed.is_ok(), "refresh output must remain valid JSON");
    let Some(parsed) = parsed.ok() else {
        return;
    };
    let receipt = parsed.pointer("/mutation_receipt");
    assert!(
        receipt.is_some(),
        "refresh JSON should contain a mutation receipt"
    );
    let Some(receipt) = receipt else {
        return;
    };
    assert_eq!(receipt["operation"], "refresh");
    assert_eq!(receipt["changed_allow_ids"][0], "fixture-refresh-drift");
    assert_eq!(
        receipt["before_fingerprints"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        receipt["after_fingerprints"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(receipt["result"], "stdout");
}

#[test]
fn refresh_human_mentions_lifecycle_preservation() {
    let text = render_refresh_human(RefreshReport::new(
        InventoryContext::source_syntax("unknown", None, None),
        &sample_entry(),
        &sample_finding(),
        Some(LastSeen {
            line: 14,
            column: 8,
        }),
        "allow-drift last_seen changed from 14:8 to 22:4",
        RefreshModeContext {
            explicit_dry_run: false,
            write_requested: false,
            written_path: None,
        },
        sample_mutation_receipt(),
    ));

    assert!(text.contains("lifecycle: preserved"));
    assert!(text.contains("fixture-refresh-drift"));
}

#[test]
fn refresh_human_styles_only_the_fixed_lifecycle_marker() {
    let text = render_refresh_human_styled(
        RefreshReport::new(
            InventoryContext::source_syntax("unknown", None, None),
            &sample_entry(),
            &sample_finding(),
            Some(LastSeen {
                line: 14,
                column: 8,
            }),
            "allow-drift last_seen changed from 14:8 to 22:4",
            RefreshModeContext {
                explicit_dry_run: false,
                write_requested: false,
                written_path: None,
            },
            sample_mutation_receipt(),
        ),
        Style::ANSI,
    );

    assert!(text.contains("lifecycle: \u{1b}[32mpreserved\u{1b}[0m"));
    assert!(text.contains("drift: allow-drift last_seen changed from 14:8 to 22:4"));
    assert_eq!(text.matches('\u{1b}').count(), 2);
}
