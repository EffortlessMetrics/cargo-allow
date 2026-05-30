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
