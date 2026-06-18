use std::path::PathBuf;

use super::{Finding, FindingKind, Span, StructuralIdentity, finding_identity_key};

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
