use crate::{
    AllowEntry, FindingKind, LastSeen, Lifecycle, Selector, allow_entry_content_fingerprint,
    normalize_snippet, sha256_v1_bytes, stable_hash_hex,
};
use std::path::PathBuf;

#[test]
fn hash_is_stable() {
    assert_eq!(stable_hash_hex("abc"), stable_hash_hex("abc"));
    assert_ne!(stable_hash_hex("abc"), stable_hash_hex("abd"));
}

#[test]
fn byte_fingerprint_uses_versioned_sha256() {
    assert_eq!(
        sha256_v1_bytes(b"abc"),
        "sha256:v1:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn normalize_snippet_collapses_mixed_whitespace() {
    assert_eq!(
        normalize_snippet("  let\tvalue =\nitems [ index ];\r\n"),
        "let value = items [ index ];"
    )
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

fn sample_allow_entry() -> AllowEntry {
    AllowEntry {
        id: "allow-0042".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src\\lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "test".to_string(),
        reason: "fixture".to_string(),
        evidence: vec!["test:sample".to_string()],
        links: vec!["issue:42".to_string()],
        occurrence_limit: Some(2),
        lifecycle: Lifecycle {
            created: Some("2026-07-13".to_string()),
            review_after: Some("2026-10-13".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: Some(LastSeen { line: 7, column: 9 }),
    }
}

#[test]
fn allow_entry_fingerprint_is_versioned_sha256() {
    let fingerprint = allow_entry_content_fingerprint(&sample_allow_entry());
    assert!(fingerprint.starts_with("sha256:v1:"));
    assert_eq!(fingerprint.len(), "sha256:v1:".len() + 64);
    assert!(
        fingerprint["sha256:v1:".len()..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
}

#[test]
fn allow_entry_fingerprint_changes_when_content_changes() {
    let first = sample_allow_entry();
    let mut changed = first.clone();
    changed.reason = "different fixture".to_string();

    assert_ne!(
        allow_entry_content_fingerprint(&first),
        allow_entry_content_fingerprint(&changed)
    );
}

#[test]
fn allow_entry_fingerprint_is_stable_across_glob_path_separators() {
    // Semantically identical scopes authored on Windows vs Unix must
    // fingerprint identically, matching the existing `path` normalization.
    let mut windows_style = sample_allow_entry();
    windows_style.glob = Some("docs\\**".to_string());
    windows_style.selector.glob = Some("docs\\**".to_string());

    let mut unix_style = sample_allow_entry();
    unix_style.glob = Some("docs/**".to_string());
    unix_style.selector.glob = Some("docs/**".to_string());

    assert_eq!(
        allow_entry_content_fingerprint(&windows_style),
        allow_entry_content_fingerprint(&unix_style)
    );
}

#[test]
fn allow_entry_fingerprint_still_distinguishes_different_globs() {
    let mut narrow = sample_allow_entry();
    narrow.glob = Some("docs/guide.md".to_string());

    let mut broad = sample_allow_entry();
    broad.glob = Some("docs/**".to_string());

    assert_ne!(
        allow_entry_content_fingerprint(&narrow),
        allow_entry_content_fingerprint(&broad)
    );
}

#[test]
fn fingerprint_is_sensitive_to_last_seen() {
    let first = sample_allow_entry();
    let mut changed = first.clone();
    changed.last_seen = Some(LastSeen {
        line: 99,
        column: 1,
    });

    assert_ne!(
        allow_entry_content_fingerprint(&first),
        allow_entry_content_fingerprint(&changed)
    );
}

#[test]
fn fingerprint_is_sensitive_to_family() {
    let first = sample_allow_entry();
    let mut changed = first.clone();
    changed.family = Some("expect".to_string());

    assert_ne!(
        allow_entry_content_fingerprint(&first),
        allow_entry_content_fingerprint(&changed)
    );
}

#[test]
fn fingerprint_is_sensitive_to_occurrence_limit() {
    let first = sample_allow_entry();
    let mut changed = first.clone();
    changed.occurrence_limit = Some(42);

    assert_ne!(
        allow_entry_content_fingerprint(&first),
        allow_entry_content_fingerprint(&changed)
    );
}

#[test]
fn fingerprint_is_sensitive_to_selector_fields() {
    let first = sample_allow_entry();
    let mut changed = first.clone();
    changed.selector.callee = Some("expect".to_string());

    assert_ne!(
        allow_entry_content_fingerprint(&first),
        allow_entry_content_fingerprint(&changed)
    );
}

#[test]
fn fingerprint_is_sensitive_to_entry_id() {
    let first = sample_allow_entry();
    let mut changed = first.clone();
    changed.id = "allow-different".to_string();

    assert_ne!(
        allow_entry_content_fingerprint(&first),
        allow_entry_content_fingerprint(&changed)
    );
}
