use super::*;

#[test]
fn human_vocabulary_lists_all_kind_groups() {
    let text = render_vocabulary_human_styled(allow_report::Style::PLAIN);

    assert!(text.contains("panic"));
    assert!(text.contains("unsafe"));
    assert!(text.contains("lint-exception"));
    assert!(text.contains("non-rust"));
    assert!(text.contains("generated"));
    assert!(text.contains("executable"));
    assert!(text.contains("workflow"));
    assert!(text.contains("dependency-surface"));
    assert!(text.contains("process"));
    assert!(text.contains("network"));
}

#[test]
fn human_vocabulary_lists_kind_aliases() {
    let text = render_vocabulary_human_styled(allow_report::Style::PLAIN);

    assert!(
        text.contains("clippy"),
        "clippy alias should appear: {text}"
    );
    assert!(text.contains("dep"), "dep alias should appear: {text}");
    assert!(text.contains("proc"), "proc alias should appear: {text}");
}

#[test]
fn human_vocabulary_lists_evidence_prefix_categories() {
    let text = render_vocabulary_human_styled(allow_report::Style::PLAIN);

    assert!(text.contains("Local-file"));
    assert!(text.contains("Traceability"));
    assert!(text.contains("doc"));
    assert!(text.contains("test"));
    assert!(text.contains("unsafe-review"));
    assert!(text.contains("legacy-policy"));
}

#[test]
fn human_vocabulary_lists_all_statuses() {
    let text = render_vocabulary_human_styled(allow_report::Style::PLAIN);

    for status in MatchStatus::ALL {
        assert!(
            text.contains(status.as_str()),
            "status `{}` should appear: {text}",
            status.as_str()
        );
    }
}

#[test]
fn human_vocabulary_styles_fixed_headings_and_statuses_only() {
    let text = render_vocabulary_human_styled(allow_report::Style::ANSI);

    assert!(text.starts_with("\u{1b}[1mcargo-allow vocabulary\u{1b}[0m\n"));
    assert!(text.contains("  \u{1b}[31mnew\u{1b}[0m\n"));
    assert!(text.contains("  \u{1b}[32mmatched\u{1b}[0m\n"));
    assert!(!text.contains("panic\u{1b}"));
    assert!(!text.contains("doc:\u{1b}"));
}

#[test]
fn json_vocabulary_parses_and_contains_expected_fields() {
    let json = render_vocabulary_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|err| {
        std::panic::panic_any(format!("vocabulary JSON should parse: {err}\n{json}"))
    });

    let kinds = parsed
        .pointer("/kinds")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| std::panic::panic_any("kinds should be an array"));
    assert!(
        kinds.len() >= 10,
        "should have at least 10 kind groups, got {}",
        kinds.len()
    );

    let canonical = parsed
        .pointer("/evidence_prefixes/canonical")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| std::panic::panic_any("evidence_prefixes.canonical should be an array"));
    assert!(
        canonical.len() >= 10,
        "should have at least 10 canonical evidence prefixes"
    );

    let statuses = parsed
        .pointer("/statuses")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| std::panic::panic_any("statuses should be an array"));
    assert_eq!(
        statuses.len(),
        MatchStatus::ALL.len(),
        "should list all MatchStatus variants"
    );
}

#[test]
fn json_vocabulary_kind_group_has_aliases_array() {
    let json = render_vocabulary_json();
    let parsed: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|err| std::panic::panic_any(format!("vocabulary JSON parse: {err}")));

    let kinds = parsed
        .pointer("/kinds")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| std::panic::panic_any("kinds should be an array"));

    let panic_group = kinds
        .iter()
        .find(|k| k.pointer("/canonical") == Some(&serde_json::json!("panic")))
        .unwrap_or_else(|| std::panic::panic_any("panic kind group should exist"));

    let aliases = panic_group
        .pointer("/aliases")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| std::panic::panic_any("panic group should have aliases array"));
    assert!(
        aliases.iter().any(|a| a == "no-panic-allowlist"),
        "panic aliases should include no-panic-allowlist: {aliases:?}"
    );
}
