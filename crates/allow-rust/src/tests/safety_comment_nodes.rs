use allow_core::FindingKind;

use crate::scan_rust_source;
use crate::syntax_facts::syntax_facts_with_outcome;

#[test]
fn safety_comment_nodes_collect_from_comment_ast() {
    let source = r#"
fn main() {
    // SAFETY: caller validates the pointer.
    unsafe { load(ptr) };
    let text = "SAFETY: not a comment";
    /* SAFETY: block comment form */
    unsafe { store(ptr) }; // SAFETY: inline trailing form
}
"#;

    let outcome = syntax_facts_with_outcome(source);
    let lines = &outcome.facts.safety_comment_lines;

    assert!(lines.contains(&3));
    assert!(lines.contains(&6));
    assert!(lines.contains(&7));
    assert!(!lines.contains(&5));
}

#[test]
fn safety_comment_nodes_reject_string_literal_lookalikes() {
    let source = r##"
fn main() {
    let text = "SAFETY: not a comment";
    let raw = r#"// SAFETY: also not a comment"#;
    let bytes = b"/* SAFETY: byte string */";
    let code = "fn f() { /* SAFETY: inside normal string */ }";
}
"##;

    let outcome = syntax_facts_with_outcome(source);
    assert!(
        outcome.facts.safety_comment_lines.is_empty(),
        "string/raw/byte/char lookalikes must not produce SAFETY comment facts"
    );
}

#[test]
fn safety_comment_nodes_reject_substring_false_positives() {
    let source = r#"
fn main() {
    // see the SAFETY: section of the RFC
    // discussing SAFETY: trade-offs
    /* note about SAFETY: not a proof */
}
"#;

    let outcome = syntax_facts_with_outcome(source);
    assert!(outcome.facts.safety_comment_lines.is_empty());
}

#[test]
fn safety_comment_nodes_block_comment_span_covers_continuation_lines() {
    let source = r#"
fn main() {
    /*
     * SAFETY: first line
     * continued proof
     */
    unsafe { load(ptr) };
}
"#;

    let outcome = syntax_facts_with_outcome(source);
    let lines = &outcome.facts.safety_comment_lines;

    assert!(lines.contains(&3));
    assert!(lines.contains(&4));
    assert!(lines.contains(&5));
    assert!(lines.contains(&6));
}

#[test]
fn safety_comment_association_preserves_nearby_metadata_for_real_comments() {
    let src = r#"
        fn read(ptr: *const u8) -> u8 {
            // SAFETY: caller provides a valid pointer.
            unsafe { core::ptr::read(ptr) }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let unsafe_block = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .unwrap_or_else(|| std::panic::panic_any("unsafe block should be found"));
    assert_eq!(
        unsafe_block.identity.target_fingerprint.as_deref(),
        Some("safety-comment:present")
    );
}

#[test]
fn safety_comment_association_rejects_string_literal_near_unsafe() {
    let src = r##"
        fn read(ptr: *const u8) -> u8 {
            let proof = "// SAFETY: not a real comment";
            unsafe { core::ptr::read(ptr) }
        }
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    let unsafe_block = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .unwrap_or_else(|| std::panic::panic_any("unsafe block should be found"));
    assert_ne!(
        unsafe_block.identity.target_fingerprint.as_deref(),
        Some("safety-comment:present")
    );
}

#[test]
fn safety_comment_association_inline_trailing_comment() {
    let src = r#"
        fn read(ptr: *const u8) -> u8 {
            unsafe { core::ptr::read(ptr) } // SAFETY: pointer validated by caller.
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let unsafe_block = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .unwrap_or_else(|| std::panic::panic_any("unsafe block should be found"));
    assert_eq!(
        unsafe_block.identity.target_fingerprint.as_deref(),
        Some("safety-comment:present")
    );
}
