use std::collections::BTreeMap;
use std::path::Path;

use allow_core::FindingKind;

use super::scan_source_lines;
use crate::syntax_facts::syntax_facts_with_outcome;
use crate::syntax_kinds::{RustSyntaxFacts, SafetyCommentAssociation, UnsafeSyntaxConstruct, UnsafeSyntaxKind};

#[test]
fn scan_source_lines_call_presence_observer() {
    let source = "fn read() {\n    // SAFETY: pointer validated by caller.\n    unsafe { core::ptr::read(ptr) }\n}\n";
    let outcome = syntax_facts_with_outcome(source);
    let findings = scan_source_lines(Path::new("src/lib.rs"), source, outcome.facts);

    match findings.as_slice() {
        [finding] => {
            assert_eq!(finding.kind, FindingKind::Unsafe);
            assert_eq!(finding.family.as_deref(), Some("unsafe_block"));
            assert_eq!(finding.identity.line_hint, Some(3));
            assert_eq!(
                finding.identity.target_fingerprint.as_deref(),
                Some("safety-comment:present")
            );
        }
        other => assert_eq!(other.len(), 1),
    }

    let empty = scan_source_lines(
        Path::new("src/main.rs"),
        "fn main() {\n    let value = 1;\n}\n",
        RustSyntaxFacts::default(),
    );
    assert_eq!(empty, Vec::new());
}

#[test]
fn scan_source_lines_uses_structural_association_map() {
    let source = "fn read() {\n    unsafe { core::ptr::read(ptr) }\n}\n";
    let mut syntax = RustSyntaxFacts::default();
    syntax.unsafe_constructs.insert(
        2,
        vec![UnsafeSyntaxConstruct {
            kind: UnsafeSyntaxKind::Block,
            column: 5,
            start_byte: 0,
            symbol: None,
        }],
    );
    syntax
        .safety_comment_associations
        .insert((2, 5), SafetyCommentAssociation::Attached);

    let findings = scan_source_lines(Path::new("src/lib.rs"), source, syntax);
    assert_eq!(
        findings[0].identity.target_fingerprint.as_deref(),
        Some("safety-comment:present")
    );
}
