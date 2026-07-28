use allow_core::{AllowConfig, AllowEntry, FindingKind, LastSeen, Lifecycle, Selector};
use std::path::PathBuf;

use crate::{parse_policy, render_policy};

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
fn renders_and_parses_escaped_basic_strings() {
    let mut cfg = AllowConfig::empty();
    let reason = "Quoted \"reason\"\nwith backslash \\ and tab\tinside";
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
        rendered
            .contains("reason = \"Quoted \\\"reason\\\"\\nwith backslash \\\\ and tab\\tinside\"")
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

#[test]
fn renders_and_parses_selector_metadata() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(AllowEntry {
        id: "allow-selector-metadata".to_string(),
        kind: FindingKind::Panic,
        family: Some("indexing_slicing".to_string()),
        path: Some(PathBuf::from("src/parser/span.rs")),
        glob: None,
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates span ranges before slicing.".to_string(),
        evidence: vec!["test:parser_rejects_invalid_span_range".to_string()],
        links: vec!["doc:docs/parser-spans.md".to_string()],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-29".to_string()),
            review_after: Some("2026-08-29".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("index_expr".to_string()),
            container: Some("slice_checked_span".to_string()),
            callee: Some("index".to_string()),
            macro_name: Some("span_guard".to_string()),
            lint: Some("clippy::indexing_slicing".to_string()),
            symbol: Some("source[range]".to_string()),
            receiver_fingerprint: Some("fnv1a64:receiver".to_string()),
            target_fingerprint: Some("fnv1a64:target".to_string()),
            normalized_snippet_hash: Some("fnv1a64:snippet".to_string()),
            line_hint: Some(42),
            glob: Some("src/parser/span.rs".to_string()),
        },
        last_seen: Some(LastSeen {
            line: 45,
            column: 17,
        }),
    });

    let rendered = render_policy(&cfg);
    for expected in [
        "[allow.selector]",
        "ast_kind = \"index_expr\"",
        "container = \"slice_checked_span\"",
        "callee = \"index\"",
        "macro_name = \"span_guard\"",
        "lint = \"clippy::indexing_slicing\"",
        "symbol = \"source[range]\"",
        "receiver_fingerprint = \"fnv1a64:receiver\"",
        "target_fingerprint = \"fnv1a64:target\"",
        "normalized_snippet_hash = \"fnv1a64:snippet\"",
        "glob = \"src/parser/span.rs\"",
        "[allow.last_seen]",
        "line = 45",
        "column = 17",
    ] {
        assert!(
            rendered.contains(expected),
            "rendered policy should contain `{expected}`:\n{rendered}"
        );
    }

    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    let entry = reparsed
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("rendered policy should keep allow entry"));
    assert_eq!(entry.selector.ast_kind.as_deref(), Some("index_expr"));
    assert_eq!(
        entry.selector.container.as_deref(),
        Some("slice_checked_span")
    );
    assert_eq!(entry.selector.callee.as_deref(), Some("index"));
    assert_eq!(entry.selector.macro_name.as_deref(), Some("span_guard"));
    assert_eq!(
        entry.selector.lint.as_deref(),
        Some("clippy::indexing_slicing")
    );
    assert_eq!(entry.selector.symbol.as_deref(), Some("source[range]"));
    assert_eq!(
        entry.selector.receiver_fingerprint.as_deref(),
        Some("fnv1a64:receiver")
    );
    assert_eq!(
        entry.selector.target_fingerprint.as_deref(),
        Some("fnv1a64:target")
    );
    assert_eq!(
        entry.selector.normalized_snippet_hash.as_deref(),
        Some("fnv1a64:snippet")
    );
    assert_eq!(entry.selector.line_hint, None);
    assert_eq!(entry.selector.glob.as_deref(), Some("src/parser/span.rs"));
    assert_eq!(
        entry
            .last_seen
            .as_ref()
            .map(|last_seen| (last_seen.line, last_seen.column)),
        Some((45, 17))
    );
}
