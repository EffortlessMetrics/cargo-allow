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
    let base_name = macro_text.rsplit("::").next().unwrap_or(macro_text);
    let kind = PanicMacroKind::from_name(base_name)?;
    let start = macro_node.start_position();
    let base_offset = macro_text.len().saturating_sub(base_name.len());
    Some((
        start.row as u32 + 1,
        PanicMacroInvocation {
            kind,
            column: source_column(line_index, source, start.row, start.column + base_offset),
            macro_path: normalize_snippet(macro_text),
        },
    ))
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
            let kind = PanicMethodKind::from_name(method_name)?;
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
        // Path-qualified: Type::unwrap(x) or <T>::unwrap(x) (#1880)
        "scoped_identifier" | "generic_function" => {
            let mut last: Option<(Node, &str)> = None;
            let mut cur = function.walk();
            cur.reset(function);
            if cur.goto_first_child() {
                loop {
                    let ch = cur.node();
                    if let Some(t) = node_text(source, ch)
                        && t != "::"
                    {
                        last = Some((ch, t));
                    }
                    if !cur.goto_next_sibling() {
                        break;
                    }
                }
            }
            let (nn, mn) = last?;
            let kind = PanicMethodKind::from_name(mn)?;
            let s = nn.start_position();
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
