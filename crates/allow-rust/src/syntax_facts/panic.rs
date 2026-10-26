use allow_core::normalize_snippet;
use tree_sitter::Node;

use crate::syntax_facts::fingerprint::structural_receiver_fingerprint;
use crate::syntax_kinds::{
    PanicMacroInvocation, PanicMacroKind, PanicMethodCall, PanicMethodKind, RustSyntaxFacts,
};
use crate::syntax_tree::node_text;
use crate::text::{SourceLineIndex, source_column};

pub(super) fn record_node_panic_constructs(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    facts: &mut RustSyntaxFacts,
) {
    if let Some((line, invocation)) = panic_macro_invocation(node, source, line_index) {
        facts.panic_macros.entry(line).or_default().push(invocation);
    }
    if let Some((line, method_call)) = panic_method_call(node, source, line_index) {
        facts
            .panic_methods
            .entry(line)
            .or_default()
            .push(method_call);
    }
}

fn panic_macro_invocation(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
) -> Option<(u32, PanicMacroInvocation)> {
    if node.kind() != "macro_invocation" {
        return None;
    }
    let macro_node = node.child_by_field_name("macro")?;
    let macro_text = node_text(source, macro_node)?;
    // #4645: resolve the macro name from the trailing identifier node of the
    // path instead of `rsplit("::")` over raw source text. Raw spellings
    // (`std::r#panic!`) and legal trivia between segments (`std :: panic!`,
    // `std::/* note */panic!`) are directly visible source syntax, and the
    // identifier node keeps the reported column at the written name.
    let name_node = trailing_identifier_node(macro_node)?;
    let raw_name = node_text(source, name_node)?;
    let kind = PanicMacroKind::from_name(raw_name.strip_prefix("r#").unwrap_or(raw_name))?;
    let start = name_node.start_position();
    Some((
        start.row as u32 + 1,
        PanicMacroInvocation {
            kind,
            column: source_column(line_index, source, start.row, start.column),
            macro_path: normalize_snippet(macro_text),
        },
    ))
}

/// The last `identifier` child of a macro or method path. Comments are
/// tree-sitter extras that land between path tokens, so raw-text splitting
/// cannot isolate the name; scanning child identifier nodes can, for direct
/// (`panic!`), qualified (`std::panic!`), and raw (`std::r#panic!`) paths.
fn trailing_identifier_node(path: Node<'_>) -> Option<Node<'_>> {
    if path.kind() == "identifier" {
        return Some(path);
    }
    let mut cursor = path.walk();
    let mut last: Option<Node<'_>> = None;
    for child in path.children(&mut cursor) {
        if child.kind() == "identifier" {
            last = Some(child);
        }
    }
    last
}

fn panic_method_call(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
) -> Option<(u32, PanicMethodCall)> {
    if node.kind() != "call_expression" {
        return None;
    }
    let function = node.child_by_field_name("function")?;

    match function.kind() {
        "field_expression" => {
            let field = function.child_by_field_name("field")?;
            let method_name = node_text(source, field)?;
            // #4645: method names may be raw identifiers (`v.r#unwrap()`);
            // the field token text carries the `r#` prefix.
            let kind =
                PanicMethodKind::from_name(method_name.strip_prefix("r#").unwrap_or(method_name))?;
            let receiver_fingerprint = function
                .child_by_field_name("value")
                .and_then(|r| structural_receiver_fingerprint(r, source));
            let s = field.start_position();
            Some((
                s.row as u32 + 1,
                PanicMethodCall {
                    kind,
                    column: source_column(line_index, source, s.row, s.column),
                    receiver_fingerprint,
                },
            ))
        }
        // Path-qualified: Type::unwrap(x) or <T>::unwrap(x) (#1880).
        // #4645: the method name is the trailing identifier node of the
        // qualified path — walk through a trailing turbofish wrapper
        // (`Vec::unwrap::<u8>(x)` is a generic_function around the path) and
        // ignore comment extras sitting between path tokens. Raw spellings
        // (`Type::r#unwrap(x)`) carry the `r#` prefix in the identifier text.
        // #4645 review: only a path-qualified callee is method-shaped — a
        // bare generic free call (`unwrap::<T>(x)`) wraps a plain
        // `identifier`, and a user-defined free function with that name is
        // not a panic method; the unqualified free-call form stays unscanned.
        "scoped_identifier" | "generic_function" => {
            let mut qualified = function;
            while qualified.kind() == "generic_function" {
                qualified = qualified.child_by_field_name("function")?;
            }
            if qualified.kind() != "scoped_identifier" {
                return None;
            }
            let name_node = trailing_identifier_node(qualified)?;
            let raw_name = node_text(source, name_node)?;
            let kind = PanicMethodKind::from_name(raw_name.strip_prefix("r#").unwrap_or(raw_name))?;
            let s = name_node.start_position();
            Some((
                s.row as u32 + 1,
                PanicMethodCall {
                    kind,
                    column: source_column(line_index, source, s.row, s.column),
                    receiver_fingerprint: None,
                },
            ))
        }
        _ => None,
    }
}
