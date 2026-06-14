use crate::text::{byte_column_to_char_column, extract_lints, lint_policy_reference};

#[test]
fn extract_lints_filters_metadata_empty_segments_and_trailing_text() {
    let lints = extract_lints(
        r#" clippy::unwrap_used, , reason = "policy:allow-lint", dead_code) trailing"#,
    );

    assert_eq!(
        lints,
        vec!["clippy::unwrap_used".to_string(), "dead_code".to_string()]
    );
    assert_eq!(
        extract_lints(r#"reason = "policy:allow-lint", , ) ignored"#),
        Vec::<String>::new()
    );
}

#[test]
fn lint_policy_reference_accepts_stable_id_characters_and_stops_at_boundaries() {
    assert_eq!(
        lint_policy_reference("reason = \"policy:ALLOW-123_a-b.\""),
        Some("ALLOW-123_a-b".to_string())
    );
    assert_eq!(
        lint_policy_reference("reason = \"policy:allow_lint extra\""),
        Some("allow_lint".to_string())
    );
    assert_eq!(lint_policy_reference("reason without marker"), None);
    assert_eq!(lint_policy_reference("reason = \"policy:!\""), None);
}

#[test]
fn byte_columns_map_to_character_columns_around_unicode_boundaries() {
    let line = format!("{}x{}z", '\u{00e9}', '\u{1f600}');

    assert_eq!(byte_column_to_char_column(&line, 0), 1);
    assert_eq!(byte_column_to_char_column(&line, 1), 2);
    assert_eq!(byte_column_to_char_column(&line, 2), 2);
    assert_eq!(byte_column_to_char_column(&line, 3), 3);
    assert_eq!(byte_column_to_char_column(&line, 7), 4);
    assert_eq!(byte_column_to_char_column(&line, 8), 5);
}
