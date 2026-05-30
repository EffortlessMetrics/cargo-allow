use super::*;
use allow_core::{MatchOutcome, MatchStatus};
use serde_json::Value;

#[test]
fn markdown_pr_summary_reports_unchanged_posture() {
    let text = render_diff_pr_summary_markdown(0, &[], &[], &[]);

    assert!(text.contains("**Net posture:** `unchanged`"));
    assert!(text.contains("| Current check failures | 0 |"));
    assert!(text.contains("no source exception posture change detected"));
}

#[test]
fn markdown_pr_summary_reports_review_required_for_new_source_finding() {
    let changes = vec![finding_posture_change(
        allow_diff::FindingPostureKind::New,
        "panic",
        Some("unwrap"),
        "src/lib.rs",
    )];

    let text = render_diff_pr_summary_markdown(0, &[], &changes, &[]);

    assert!(text.contains("**Net posture:** `review-required`"));
    assert!(text.contains("| New source findings | 1 |"));
    assert!(text.contains("review the source exception posture change"));
}

#[test]
fn markdown_pr_summary_reports_worse_for_policy_failure() {
    let changes = vec![policy_change(
        allow_diff::PolicyChangeSeverity::Fail,
        allow_diff::PolicyChangeKind::ScopeBroadened,
    )];

    let text = render_diff_pr_summary_markdown(0, &[], &[], &changes);

    assert!(text.contains("**Net posture:** `worse`"));
    assert!(text.contains("| Policy failures | 1 |"));
    assert!(text.contains("block until failing source exception changes"));
}

#[test]
fn markdown_pr_summary_reports_improved_for_removed_source_finding() {
    let changes = vec![finding_posture_change(
        allow_diff::FindingPostureKind::Removed,
        "panic",
        Some("unwrap"),
        "src/lib.rs",
    )];

    let text = render_diff_pr_summary_markdown(0, &[], &changes, &[]);

    assert!(text.contains("**Net posture:** `improved`"));
    assert!(text.contains("| Removed source findings | 1 |"));
    assert!(text.contains("keep the narrower posture"));
}

#[test]
fn markdown_pr_summary_reports_improved_for_removed_policy_entry() {
    let changes = vec![policy_change(
        allow_diff::PolicyChangeSeverity::Improvement,
        allow_diff::PolicyChangeKind::RemovedAllow,
    )];

    let text = render_diff_pr_summary_markdown(0, &[], &[], &changes);

    assert!(text.contains("**Net posture:** `improved`"));
    assert!(text.contains("| Policy improvements | 1 |"));
    assert!(text.contains("keep the narrower posture"));
}

#[test]
fn json_report_includes_structured_posture_changes() {
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        None,
        Some(0),
        "unreceipted panic.unwrap at src/lib.rs:1:1",
    )];
    let finding_changes = vec![finding_posture_change(
        allow_diff::FindingPostureKind::New,
        "panic",
        Some("unwrap"),
        "src/lib.rs",
    )];
    let policy_changes = vec![
        allow_diff::PolicyChange {
            allow_id: "allow-0001".to_string(),
            kind: allow_diff::PolicyChangeKind::SelectorPrecisionDecreased,
            severity: allow_diff::PolicyChangeSeverity::Fail,
            message: "allow-0001 selector precision decreased: 80 -> 45".to_string(),
            selector_precision: Some(allow_diff::SelectorPrecisionChange {
                before: 80,
                after: 45,
                removed_fields: vec!["container", "normalized_snippet_hash"],
                added_fields: vec![],
            }),
            scope: None,
        },
        allow_diff::PolicyChange {
            allow_id: "allow-0002".to_string(),
            kind: allow_diff::PolicyChangeKind::ScopeBroadened,
            severity: allow_diff::PolicyChangeSeverity::Fail,
            message: "allow-0002 scope broadened".to_string(),
            selector_precision: None,
            scope: Some(allow_diff::ScopeChange {
                field: allow_diff::ScopeChangeField::Effective,
                before: Some("src/lib.rs".to_string()),
                after: Some("src/**".to_string()),
            }),
        },
    ];

    let json = render_diff_json_with_posture(
        allow_report::render_json_with_context(
            "diff",
            &[],
            &[],
            false,
            allow_report::ReportContext::default(),
        ),
        1,
        &outcomes,
        &finding_changes,
        &policy_changes,
    );
    let value = parse_json("diff report", &json);

    assert_eq!(
        value.pointer("/diff/net_posture").and_then(Value::as_str),
        Some("worse")
    );
    assert_eq!(
        value
            .pointer("/diff/reviewer_action")
            .and_then(Value::as_str),
        Some("block until failing source exception changes are fixed, narrowed, or receipted.")
    );
    assert_eq!(
        value
            .pointer("/diff/summary/current_failures")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/new_findings")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_failures")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        value
            .pointer("/diff/summary/policy_improvements")
            .and_then(Value::as_u64),
        Some(0)
    );
    let finding_change = first_array_item(&value, "/diff/finding_changes");
    assert_eq!(
        finding_change.get("change").and_then(Value::as_str),
        Some("new")
    );
    assert_eq!(
        finding_change.get("kind").and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        finding_change.get("family").and_then(Value::as_str),
        Some("unwrap")
    );
    assert_eq!(
        finding_change.get("path").and_then(Value::as_str),
        Some("src/lib.rs")
    );
    let policy_change = first_array_item(&value, "/diff/policy_changes");
    assert_eq!(
        policy_change.get("severity").and_then(Value::as_str),
        Some("fail")
    );
    assert_eq!(
        policy_change.get("allow_id").and_then(Value::as_str),
        Some("allow-0001")
    );
    assert_eq!(
        policy_change.get("kind").and_then(Value::as_str),
        Some("selector_precision_decreased")
    );
    assert_eq!(
        policy_change
            .pointer("/selector_precision/before")
            .and_then(Value::as_u64),
        Some(80)
    );
    assert_eq!(
        policy_change
            .pointer("/selector_precision/after")
            .and_then(Value::as_u64),
        Some(45)
    );
    assert_eq!(
        policy_change
            .pointer("/selector_precision/removed_fields/0")
            .and_then(Value::as_str),
        Some("container")
    );
    let policy_changes = value
        .pointer("/diff/policy_changes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"));
    let scope_change = policy_changes
        .get(1)
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should include scope row"));
    assert_eq!(
        scope_change.get("kind").and_then(Value::as_str),
        Some("scope_broadened")
    );
    assert_eq!(
        scope_change.pointer("/scope/field").and_then(Value::as_str),
        Some("effective")
    );
    assert_eq!(
        scope_change
            .pointer("/scope/before")
            .and_then(Value::as_str),
        Some("src/lib.rs")
    );
    assert_eq!(
        scope_change.pointer("/scope/after").and_then(Value::as_str),
        Some("src/**")
    );
    assert!(json.ends_with("}\n"));
}

#[test]
fn json_report_keeps_base_report_when_append_fails() {
    let base = "not json".to_string();

    let json = render_diff_json_with_posture(base.clone(), 0, &[], &[], &[]);

    assert_eq!(json, base);
}

fn test_outcome(
    status: MatchStatus,
    allow_id: Option<&str>,
    finding_index: Option<usize>,
    message: &str,
) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}

fn finding_posture_change(
    kind: allow_diff::FindingPostureKind,
    finding_kind: &str,
    family: Option<&str>,
    path: &str,
) -> allow_diff::FindingPostureChange {
    allow_diff::FindingPostureChange {
        kind,
        key: format!("{finding_kind}:{path}"),
        finding_kind: finding_kind.to_string(),
        family: family.map(str::to_string),
        path: path.to_string(),
    }
}

fn policy_change(
    severity: allow_diff::PolicyChangeSeverity,
    kind: allow_diff::PolicyChangeKind,
) -> allow_diff::PolicyChange {
    allow_diff::PolicyChange {
        allow_id: "allow-0001".to_string(),
        kind,
        severity,
        message: "allow-0001 changed".to_string(),
        selector_precision: None,
        scope: None,
    }
}

fn parse_json(name: &str, json: &str) -> Value {
    match serde_json::from_str(json) {
        Ok(value) => value,
        Err(err) => std::panic::panic_any(format!("{name} should parse as JSON: {err}\n{json}")),
    }
}

fn first_array_item<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    let Some(items) = value.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{pointer} should be an array"));
    };
    let Some(item) = items.first() else {
        std::panic::panic_any(format!("{pointer} should contain at least one item"));
    };
    item
}
