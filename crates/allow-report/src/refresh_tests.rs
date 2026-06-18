use super::*;
use crate::{InventoryContext, RefreshModeContext, RefreshReport};
use allow_core::{AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity};

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
        last_seen: Some(LastSeen { line: 22, column: 4 }),
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
        Some(LastSeen { line: 14, column: 8 }),
        "allow-drift last_seen changed from 14:8 to 22:4",
        RefreshModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
        },
    ));

    assert!(json.contains(REFRESH_SCHEMA_ID));
    assert!(json.contains("\"lifecycle_preserved\": true"));
    assert!(json.contains("\"previous_last_seen\""));
    assert!(json.contains("\"refreshed_last_seen\""));
    assert!(json.contains("\"drift_message\""));
}

#[test]
fn refresh_human_mentions_lifecycle_preservation() {
    let text = render_refresh_human(RefreshReport::new(
        InventoryContext::source_syntax("unknown", None, None),
        &sample_entry(),
        &sample_finding(),
        Some(LastSeen { line: 14, column: 8 }),
        "allow-drift last_seen changed from 14:8 to 22:4",
        RefreshModeContext {
            explicit_dry_run: false,
            write_requested: false,
            written_path: None,
        },
    ));

    assert!(text.contains("lifecycle: preserved"));
    assert!(text.contains("fixture-refresh-drift"));
}
