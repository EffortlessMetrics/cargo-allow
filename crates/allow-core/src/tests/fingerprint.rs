use crate::{maybe_line_distance_score, normalize_snippet, stable_hash_hex};

#[test]
fn hash_is_stable() {
    assert_eq!(stable_hash_hex("abc"), stable_hash_hex("abc"));
    assert_ne!(stable_hash_hex("abc"), stable_hash_hex("abd"));
}

#[test]
fn line_distance_score_uses_documented_buckets() {
    assert_eq!(maybe_line_distance_score(Some(10), Some(10)), 15);
    assert_eq!(maybe_line_distance_score(Some(10), Some(13)), 12);
    assert_eq!(maybe_line_distance_score(Some(10), Some(20)), 8);
    assert_eq!(maybe_line_distance_score(Some(10), Some(35)), 3);
    assert_eq!(maybe_line_distance_score(Some(10), Some(36)), 0);
    assert_eq!(maybe_line_distance_score(None, Some(10)), 0);
    assert_eq!(maybe_line_distance_score(Some(10), None), 0);
}

#[test]
fn normalize_snippet_collapses_mixed_whitespace() {
    assert_eq!(
        normalize_snippet("  let\tvalue =\nitems [ index ];\r\n"),
        "let value = items [ index ];"
    );
}

#[test]
fn maybe_line_distance_score_covers_boundary_bands() {
    assert_eq!(maybe_line_distance_score(Some(10), Some(10)), 15);
    assert_eq!(maybe_line_distance_score(Some(10), Some(13)), 12);
    assert_eq!(maybe_line_distance_score(Some(10), Some(20)), 8);
    assert_eq!(maybe_line_distance_score(Some(10), Some(35)), 3);
    assert_eq!(maybe_line_distance_score(Some(10), Some(36)), 0);
    assert_eq!(maybe_line_distance_score(None, Some(10)), 0);
    assert_eq!(maybe_line_distance_score(Some(10), None), 0);
    assert_eq!(maybe_line_distance_score(None, None), 0);
}

#[test]
fn normalize_snippet_collapses_all_whitespace_runs() {
    assert_eq!(
        normalize_snippet("  fn   load() {\n\tvalue . unwrap()  }  "),
        "fn load() { value . unwrap() }"
    );
}

#[test]
fn normalize_snippet_ignores_line_comments() {
    assert_eq!(
        normalize_snippet("let value = maybe.unwrap(); // reviewed in policy"),
        "let value = maybe.unwrap();"
    );
}

#[test]
fn normalize_snippet_ignores_nested_block_comments() {
    assert_eq!(
        normalize_snippet("let value /* outer /* inner */ done */ = maybe.unwrap();"),
        "let value = maybe.unwrap();"
    );
}

#[test]
fn normalize_snippet_preserves_comment_markers_inside_strings() {
    assert_eq!(
        normalize_snippet(r#"let url = "https://example.test/*not-comment*/"; // comment"#),
        r#"let url = "https://example.test/*not-comment*/";"#
    );
    assert_eq!(
        normalize_snippet(r##"let raw = r#"not // a comment"#; // comment"##),
        r##"let raw = r#"not // a comment"#;"##
    );
}

#[test]
fn stable_hash_ignores_comment_only_snippet_edits() {
    let before = normalize_snippet("let value = maybe.unwrap();");
    let after = normalize_snippet("let value = maybe.unwrap(); // reviewer note");

    assert_eq!(stable_hash_hex(&before), stable_hash_hex(&after));
}
