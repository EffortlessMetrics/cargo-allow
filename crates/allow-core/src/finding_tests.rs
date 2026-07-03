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
