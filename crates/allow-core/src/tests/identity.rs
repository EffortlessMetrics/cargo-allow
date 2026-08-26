use std::path::PathBuf;

use crate::{
    Finding, FindingKind, Selector, Span, StructuralIdentity, finding_identity_key, normalize_path,
    source_tree_path_is_ignored, source_tree_path_matches_filter,
};

#[test]
fn cross_platform_semantics() {
    let nfc = "src/café.rs";
    let nfd = "src/cafe\u{301}.rs";
    assert_eq!(normalize_path(nfc), normalize_path(nfd));
    assert!(source_tree_path_matches_filter(
        "crates/allow-core/src/lib.rs",
        "crates/*/src/*.rs"
    ));
    assert!(source_tree_path_is_ignored(
        "target/debug/cargo-allow",
        &["target/**".to_string()]
    ));

    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some("parse".to_string());
    identity.callee = Some("unwrap".to_string());
    let first = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: "src/lib.rs".into(),
        span: Some(Span {
            line: 10,
            column: 2,
        }),
        identity,
        message: "test".to_string(),
        ledger: None,
    };
    let mut moved = first.clone();
    moved.span = Some(Span {
        line: 99,
        column: 8,
    });
    assert_eq!(finding_identity_key(&first), finding_identity_key(&moved));
}

#[test]
fn selector_structural_identity_excludes_scope_and_location_hints() {
    let scoped = Selector {
        line_hint: Some(12),
        glob: Some("src/lib.rs".to_string()),
        ..Selector::default()
    };
    assert!(!scoped.has_structural_identity());

    let structural = Selector {
        ast_kind: Some("method_call".to_string()),
        ..Selector::default()
    };
    assert!(structural.has_structural_identity());
}

#[test]
fn structural_identity_key_excludes_line_and_column_hints() {
    let mut first = StructuralIdentity::new("rust", "method_call");
    first.module = Some("parser::span".to_string());
    first.container = Some("parse_span".to_string());
    first.callee = Some("unwrap".to_string());
    first.normalized_snippet_hash = Some("fnv1a64:1234".to_string());
    first.line_hint = Some(12);
    first.column_hint = Some(8);

    let mut moved = first.clone();
    moved.line_hint = Some(99);
    moved.column_hint = Some(42);

    assert_eq!(first.stable_key(), moved.stable_key());

    moved.container = Some("parse_other_span".to_string());

    assert_ne!(first.stable_key(), moved.stable_key());
}

#[test]
fn structural_identity_has_v1_schema_id() {
    assert_eq!(
        StructuralIdentity::schema_id(),
        "cargo-allow.structural-identity.v1"
    );
}

#[test]
fn structural_identity_key_uses_length_prefixed_parts() {
    let mut first = StructuralIdentity::new("rust", "method_call");
    first.container = Some("load".to_string());
    first.callee = Some("unwrap".to_string());
    first.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());

    let key = first.stable_key();

    assert!(key.contains("language:4:rust"));
    assert!(key.contains("container:4:load"));
    assert!(key.contains("callee:6:unwrap"));
    assert!(key.contains("normalized_snippet_hash:12:fnv1a64:abcd"));
}

#[test]
fn structural_identity_v1_fields_affect_stable_key_except_hints() {
    let mut base = StructuralIdentity::new("rust", "method_call");
    base.crate_name = Some("parser".to_string());
    base.module = Some("parser::span".to_string());
    base.container = Some("slice_checked_text_range".to_string());
    base.symbol = Some("source[range]".to_string());
    base.callee = Some("expect".to_string());
    base.macro_name = Some("panic".to_string());
    base.lint = Some("clippy::indexing_slicing".to_string());
    base.receiver_fingerprint = Some("source".to_string());
    base.target_fingerprint = Some("range".to_string());
    base.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());
    base.line_hint = Some(10);
    base.column_hint = Some(4);

    let mut moved = base.clone();
    moved.line_hint = Some(100);
    moved.column_hint = Some(40);
    assert_eq!(base.stable_key(), moved.stable_key());

    let cases: &[fn(&mut StructuralIdentity)] = &[
        |id| id.language = "file".to_string(),
        |id| id.crate_name = Some("runtime".to_string()),
        |id| id.module = Some("runtime::ffi".to_string()),
        |id| id.container = Some("read_buffer".to_string()),
        |id| id.ast_kind = "macro_call".to_string(),
        |id| id.symbol = Some("buffer[index]".to_string()),
        |id| id.callee = Some("unwrap".to_string()),
        |id| id.macro_name = Some("todo".to_string()),
        |id| id.lint = Some("dead_code".to_string()),
        |id| id.receiver_fingerprint = Some("buffer".to_string()),
        |id| id.target_fingerprint = Some("index".to_string()),
        |id| id.normalized_snippet_hash = Some("fnv1a64:dcba".to_string()),
    ];

    for mutate in cases {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert_ne!(base.stable_key(), changed.stable_key());
    }
}

#[test]
fn finding_identity_key_excludes_span_but_includes_structural_scope() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some("load".to_string());
    identity.callee = Some("unwrap".to_string());
    identity.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());

    let mut first = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 4,
        }),
        identity,
        message: "test finding".to_string(),
        ledger: None,
    };
    let mut moved = first.clone();
    moved.span = Some(Span {
        line: 200,
        column: 40,
    });

    assert_eq!(finding_identity_key(&first), finding_identity_key(&moved));

    moved.path = PathBuf::from("src/other.rs");
    assert_ne!(finding_identity_key(&first), finding_identity_key(&moved));

    moved.path = first.path.clone();
    first.family = Some("expect".to_string());
    assert_ne!(finding_identity_key(&first), finding_identity_key(&moved));
}

#[test]
fn finding_source_package_name_trims_source_derived_crate_name() {
    let mut finding = Finding {
        kind: FindingKind::Panic,
        family: None,
        path: PathBuf::from("src/lib.rs"),
        span: None,
        identity: StructuralIdentity::new("rust", "method_call"),
        message: "test finding".to_string(),
        ledger: None,
    };

    assert_eq!(finding.source_package_name(), None);

    finding.identity.crate_name = Some("  allow-core  ".to_string());
    assert_eq!(finding.source_package_name(), Some("allow-core"));

    finding.identity.crate_name = Some("   ".to_string());
    assert_eq!(finding.source_package_name(), None);
}

#[test]
fn stable_key_trims_crate_name_so_whitespace_does_not_corrupt_identity() {
    // Regression for #1799: source_package_name() trims but stable_key_parts()
    // did not, so a scanner-fed crate_name with stray whitespace produced a
    // different identity than the trimmed form — silent matching failures.
    let mut padded = Finding {
        kind: FindingKind::Panic,
        family: None,
        path: PathBuf::from("src/lib.rs"),
        span: None,
        identity: StructuralIdentity::new("rust", "method_call"),
        message: "test".to_string(),
        ledger: None,
    };
    padded.identity.crate_name = Some("  allow-core  ".to_string());

    let mut clean = padded.clone();
    clean.identity.crate_name = Some("allow-core".to_string());

    assert_eq!(
        padded.identity.stable_key(),
        clean.identity.stable_key(),
        "padded and clean crate_name must produce the same stable key"
    );
}
