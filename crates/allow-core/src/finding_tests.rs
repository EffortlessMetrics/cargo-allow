use std::path::PathBuf;

use super::{
    Finding, FindingKind, MAX_IDENTITY_FIELD_LEN, Span, StructuralIdentity, finding_identity_key,
};

#[test]
fn stable_identity_key_from_parts_call_presence_observer() {
    let identity = StructuralIdentity::new("rust", "method_call");
    assert_eq!(
        identity.stable_key(),
        "language:4:rust|crate_name:0:|module:0:|container:0:|ast_kind:11:method_call|symbol:0:|callee:0:|macro_name:0:|lint:0:|receiver_fingerprint:0:|target_fingerprint:0:|normalized_snippet_hash:0:"
    );

    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("rust", "method_call"),
        message: "observer finding".to_string(),
        ledger: None,
    };
    assert_eq!(
        finding_identity_key(&finding),
        "kind:5:panic|family:6:unwrap|path:10:src/lib.rs|language:4:rust|crate_name:0:|module:0:|container:0:|ast_kind:11:method_call|symbol:0:|callee:0:|macro_name:0:|lint:0:|receiver_fingerprint:0:|target_fingerprint:0:|normalized_snippet_hash:0:"
    );
}

#[test]
fn truncate_in_place_caps_over_length_identity_fields() {
    // #1919: a scanned file with a megabyte-long identifier must not inflate the
    // report/receipt artifact unboundedly. Every source-derived string field is
    // capped at MAX_IDENTITY_FIELD_LEN.
    let huge = "x".repeat(MAX_IDENTITY_FIELD_LEN * 4);
    let mut identity = StructuralIdentity::new(&huge, &huge);
    identity.symbol = Some(huge.clone());
    identity.callee = Some(huge.clone());
    identity.macro_name = Some(huge.clone());
    identity.container = Some(huge.clone());
    identity.module = Some(huge.clone());
    identity.lint = Some(huge.clone());

    identity.truncate_in_place();

    assert_eq!(identity.language.len(), MAX_IDENTITY_FIELD_LEN);
    assert_eq!(identity.ast_kind.len(), MAX_IDENTITY_FIELD_LEN);
    assert_eq!(
        identity.symbol.as_ref().map(String::len),
        Some(MAX_IDENTITY_FIELD_LEN)
    );
    assert_eq!(
        identity.callee.as_ref().map(String::len),
        Some(MAX_IDENTITY_FIELD_LEN)
    );
    assert_eq!(
        identity.macro_name.as_ref().map(String::len),
        Some(MAX_IDENTITY_FIELD_LEN)
    );
    assert_eq!(
        identity.container.as_ref().map(String::len),
        Some(MAX_IDENTITY_FIELD_LEN)
    );
    assert_eq!(
        identity.module.as_ref().map(String::len),
        Some(MAX_IDENTITY_FIELD_LEN)
    );
    assert_eq!(
        identity.lint.as_ref().map(String::len),
        Some(MAX_IDENTITY_FIELD_LEN)
    );
}

#[test]
fn truncate_in_place_leaves_short_fields_unchanged() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.symbol = Some("unwrap".to_string());
    identity.truncate_in_place();
    assert_eq!(identity.language, "rust");
    assert_eq!(identity.ast_kind, "method_call");
    assert_eq!(identity.symbol.as_deref(), Some("unwrap"));
}

#[test]
fn redact_source_text_fields_clears_text_but_preserves_anchors() {
    // #1920: redaction clears the source-text-bearing fields (info-leak
    // surface) while preserving the structural anchors matching relies on.
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.symbol = Some("secret_token".to_string());
    identity.callee = Some("leaky_call".to_string());
    identity.container = Some("my_impl".to_string());
    identity.module = Some("my_mod".to_string());
    identity.macro_name = Some("my_macro".to_string());
    identity.lint = Some("clippy::foo".to_string());
    identity.normalized_snippet_hash = Some("fnv1a64:abc".to_string());
    identity.receiver_fingerprint = Some("rx".to_string());
    identity.target_fingerprint = Some("tx".to_string());
    identity.line_hint = Some(42);
    identity.column_hint = Some(7);

    identity.redact_source_text_fields();

    // Source-text-bearing fields cleared.
    assert_eq!(identity.symbol, None);
    assert_eq!(identity.callee, None);
    assert_eq!(identity.container, None);
    assert_eq!(identity.module, None);
    assert_eq!(identity.macro_name, None);
    assert_eq!(identity.lint, None);
    // Structural anchors preserved (matching still works).
    assert_eq!(identity.language, "rust");
    assert_eq!(identity.ast_kind, "method_call");
    assert_eq!(
        identity.normalized_snippet_hash.as_deref(),
        Some("fnv1a64:abc")
    );
    assert_eq!(identity.receiver_fingerprint.as_deref(), Some("rx"));
    assert_eq!(identity.target_fingerprint.as_deref(), Some("tx"));
    assert_eq!(identity.line_hint, Some(42));
    assert_eq!(identity.column_hint, Some(7));
}
