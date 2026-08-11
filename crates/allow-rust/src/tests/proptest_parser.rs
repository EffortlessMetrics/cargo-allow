//! Parser property tests using proptest (#1911).
//!
//! These tests verify the tree-sitter Rust parser and container extraction
//! never panic on arbitrary Rust-like input, including edge cases like empty
//! strings, Unicode identifiers, deeply nested modules, and malformed
//! fragments.

use crate::parse_rust_syntax;
use proptest::prelude::*;

/// Generate a valid Rust identifier: ASCII alpha/underscore start, alphanumeric
/// continuation.
fn rust_identifier() -> impl Strategy<Value = String> {
    ("[a-zA-Z_]", "[a-zA-Z0-9_]*").prop_map(|(start, rest)| format!("{start}{rest}"))
}

/// Generate a simple Rust source with optional function and module nesting.
fn rust_source() -> impl Strategy<Value = String> {
    let depth = 0u8..4;
    depth.prop_flat_map(|d| {
        let mut mods = Vec::new();
        for i in 0..d {
            mods.push(format!("mod m{i} {{"));
        }
        let mut close = Vec::new();
        for _ in 0..d {
            close.push("}".to_string());
        }
        rust_identifier().prop_map(move |fn_name| {
            let mut src = String::new();
            for m in &mods {
                src.push_str(m);
                src.push('\n');
            }
            src.push_str(&format!("fn {fn_name}() {{}}\n"));
            for c in &close {
                src.push_str(c);
                src.push('\n');
            }
            src
        })
    })
}

proptest! {
    /// Any generated Rust source must parse without panicking and produce a
    /// non-zero named-node count.
    #[test]
    fn prop_parses_rust_like_source_without_panic(src in rust_source()) {
        let tree = parse_rust_syntax(&src)?;
        prop_assert!(tree.named_node_count() > 0, "named_node_count should be > 0 for:\n{src}");
    }

    /// Container extraction must never panic, regardless of input.
    #[test]
    fn prop_container_extraction_never_panics(src in rust_source()) {
        let tree = parse_rust_syntax(&src)?;
        let _containers = tree.containers(&src);
        // No assertion on count — the property is "does not panic".
    }

    /// Empty and whitespace-only input must not panic the parser.
    #[test]
    fn prop_empty_input_does_not_panic(src in "( |\n|\t)*") {
        let result = parse_rust_syntax(&src);
        // Empty input may succeed (zero nodes) or error; it must not panic.
        if let Ok(tree) = result {
            let _ = tree.containers(&src);
        }
    }

    /// Random byte strings must not panic the parser (tree-sitter should
    /// produce error nodes, not panics).
    #[test]
    fn prop_arbitrary_bytes_do_not_panic(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        let src = String::from_utf8_lossy(&bytes);
        let result = parse_rust_syntax(&src);
        if let Ok(tree) = result {
            let _ = tree.containers(&src);
        }
    }
}
