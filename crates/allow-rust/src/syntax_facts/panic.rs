use tree_sitter::Node;

use crate::syntax_kinds::{PanicMacroInvocation, PanicMacroKind, PanicMethodCall, PanicMethodKind};
use crate::syntax_tree::node_text;

pub(super) fn panic_macro_invocation(
    node: Node<'_>,
    source: &str,
) -> Option<(u32, PanicMacroInvocation)> {
    if node.kind() != "macro_invocation" {
        return None;
    }
    let macro_node = node.child_by_field_name("macro")?;
    let macro_text = node_text(source, macro_node)?;
    let base_name = macro_text.rsplit("::").next().unwrap_or(macro_text);
    let kind = PanicMacroKind::from_name(base_name)?;
    let start = macro_node.start_position();
    let base_offset = macro_text.len().saturating_sub(base_name.len()) as u32;
    Some((
        start.row as u32 + 1,
        PanicMacroInvocation {
            kind,
            column: start.column as u32 + base_offset + 1,
        },
    ))
}

pub(super) fn panic_method_call(node: Node<'_>, source: &str) -> Option<(u32, PanicMethodCall)> {
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
    let start = field.start_position();
    Some((
        start.row as u32 + 1,
        PanicMethodCall {
            kind,
            column: start.column as u32 + 1,
        },
    ))
}
