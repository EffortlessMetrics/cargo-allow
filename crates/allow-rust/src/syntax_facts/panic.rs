use allow_core::normalize_snippet;
use tree_sitter::Node;

use crate::syntax_kinds::{
    PanicMacroInvocation, PanicMacroKind, PanicMethodCall, PanicMethodKind, RustSyntaxFacts,
};
use crate::syntax_tree::node_text;
use crate::text::source_column;

pub(super) fn record_node_panic_constructs(
    node: Node<'_>,
    source: &str,
    facts: &mut RustSyntaxFacts,
) {
    if let Some((line, invocation)) = panic_macro_invocation(node, source) {
        facts.panic_macros.entry(line).or_default().push(invocation);
    }
    if let Some((line, method_call)) = panic_method_call(node, source) {
        facts
            .panic_methods
            .entry(line)
            .or_default()
            .push(method_call);
    }
}

fn panic_macro_invocation(node: Node<'_>, source: &str) -> Option<(u32, PanicMacroInvocation)> {
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
            column: source_column(source, start.row, start.column + base_offset),
        },
    ))
}

fn panic_method_call(node: Node<'_>, source: &str) -> Option<(u32, PanicMethodCall)> {
    if node.kind() != "call_expression" {
        return None;
    }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "field_expression" {
        return None;
    }
    let field = function.child_by_field_name("field")?;
    let method_name = node_text(source, field)?;
    let kind = PanicMethodKind::from_name(method_name)?;
    let receiver_fingerprint = function
        .child_by_field_name("value")
        .and_then(|receiver| node_text(source, receiver))
        .and_then(receiver_fingerprint);
    let start = field.start_position();
    Some((
        start.row as u32 + 1,
        PanicMethodCall {
            kind,
            column: source_column(source, start.row, start.column),
            receiver_fingerprint,
        },
    ))
}

fn receiver_fingerprint(text: &str) -> Option<String> {
    let fingerprint = normalize_snippet(text);
    if fingerprint.is_empty() {
        return None;
    }
    Some(
        fingerprint
            .chars()
            .rev()
            .take(80)
            .collect::<String>()
            .chars()
            .rev()
            .collect(),
    )
}
