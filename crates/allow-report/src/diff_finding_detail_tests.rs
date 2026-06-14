use allow_core::StructuralIdentity;

use crate::diff_finding_detail::structural_identity_summary;

#[test]
fn structural_identity_summary_renders_stable_ordered_fields() {
    let mut identity = StructuralIdentity::new("rust", "call_expr");
    identity.module = Some("crate::module".to_string());
    identity.container = Some("impl Parser".to_string());
    identity.callee = Some("parse".to_string());
    identity.macro_name = Some("assert_eq".to_string());
    identity.lint = Some("clippy::unwrap_used".to_string());
    identity.symbol = Some("Parser::parse".to_string());
    identity.receiver_fingerprint = Some("receiver-123".to_string());
    identity.target_fingerprint = Some("target-456".to_string());
    identity.normalized_snippet_hash = Some("snippet-789".to_string());

    let summary = structural_identity_summary(&identity);

    assert_eq!(
        summary,
        "ast_kind=call_expr,module=crate::module,container=impl Parser,\
callee=parse,macro_name=assert_eq,lint=clippy::unwrap_used,\
symbol=Parser::parse,receiver_fingerprint=receiver-123,\
target_fingerprint=target-456,normalized_snippet_hash=snippet-789"
    );
}

#[test]
fn structural_identity_summary_trims_values_and_omits_empty_fields() {
    let mut identity = StructuralIdentity::new("rust", "  unsafe_block  ");
    identity.module = Some("  crate::scanner  ".to_string());
    identity.container = Some("   ".to_string());
    identity.callee = Some(" visit_unsafe ".to_string());
    identity.macro_name = Some(String::new());
    identity.lint = Some(" clippy::panic ".to_string());
    identity.symbol = Some(" ".to_string());
    identity.receiver_fingerprint = Some(" receiver ".to_string());
    identity.target_fingerprint = None;
    identity.normalized_snippet_hash = Some(" hash ".to_string());

    let summary = structural_identity_summary(&identity);

    assert_eq!(
        summary,
        "ast_kind=unsafe_block,module=crate::scanner,callee=visit_unsafe,\
lint=clippy::panic,receiver_fingerprint=receiver,\
normalized_snippet_hash=hash"
    );
}

#[test]
fn structural_identity_summary_returns_empty_for_blank_identity() {
    let identity = StructuralIdentity::new("rust", "   ");

    let summary = structural_identity_summary(&identity);

    assert_eq!(summary, "");
}
