use super::*;
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use std::path::PathBuf;

#[test]
fn renders_and_parses_occurrence_limit() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(AllowEntry {
        id: "allow-counted".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "Generated baseline debt.".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: Some(3),
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: None,
            expires: Some("2026-08-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    });

    let rendered = render_policy(&cfg);
    assert!(rendered.contains("occurrence_limit = 3"));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert_eq!(
        reparsed
            .allow
            .first()
            .and_then(|entry| entry.occurrence_limit),
        Some(3)
    );
}

#[test]
fn renders_and_parses_general_evidence_requirement() {
    let mut cfg = AllowConfig::empty();
    cfg.requirements.evidence_required = true;

    let rendered = render_policy(&cfg);
    assert!(rendered.contains("evidence_required = true"));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert!(reparsed.requirements.evidence_required);
}

#[test]
fn renders_and_parses_escaped_basic_strings() {
    let mut cfg = AllowConfig::empty();
    let reason = "Quoted \"reason\"\nwith backslash \\ and tab\t";
    let evidence = "test:line\nbreak";
    cfg.allow.push(AllowEntry {
        id: "allow-escaped".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: reason.to_string(),
        evidence: vec![evidence.to_string()],
        links: vec!["doc:docs/quoted\"path.md".to_string()],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            symbol: Some("value[\"key\"]\n.unwrap()".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    });

    let rendered = render_policy(&cfg);
    assert!(
        rendered.contains("reason = \"Quoted \\\"reason\\\"\\nwith backslash \\\\ and tab\\t\"")
    );
    assert!(rendered.contains("evidence = [\"test:line\\nbreak\"]"));
    assert!(rendered.contains("links = [\"doc:docs/quoted\\\"path.md\"]"));
    assert!(rendered.contains("symbol = \"value[\\\"key\\\"]\\n.unwrap()\""));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    let entry = reparsed
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("rendered policy should keep allow entry"));
    assert_eq!(entry.reason, reason);
    assert_eq!(entry.evidence, vec![evidence.to_string()]);
    assert_eq!(entry.links, vec!["doc:docs/quoted\"path.md".to_string()]);
    assert_eq!(
        entry.selector.symbol.as_deref(),
        Some("value[\"key\"]\n.unwrap()")
    );
}
