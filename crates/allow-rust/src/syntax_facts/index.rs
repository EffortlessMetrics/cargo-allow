use tree_sitter::Node;

use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::node_text;

pub(super) fn record_index_expression(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if node.kind() != "index_expression" {
        return;
    }

    let start = node.start_position();
    let text = node_text(source, node).unwrap_or_default();
    let bracket_offset =
        direct_index_bracket_offset(node).unwrap_or_else(|| text.find('[').unwrap_or(0));
    let (row_delta, column_delta) = offset_position(text, bracket_offset);
    let line = start.row as u32 + row_delta + 1;
    let column = if row_delta == 0 {
        start.column as u32 + column_delta + 1
    } else {
        column_delta + 1
    };
    facts
        .index_columns
        .entry(line)
        .and_modify(|existing| *existing = (*existing).min(column))
        .or_insert(column);
}

fn direct_index_bracket_offset(node: Node<'_>) -> Option<usize> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "[")
        .map(|child| child.start_byte().saturating_sub(node.start_byte()))
}

fn offset_position(text: &str, offset: usize) -> (u32, u32) {
    let mut row = 0;
    let mut column = 0;

    for byte in text.as_bytes().iter().take(offset) {
        if *byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    (row, column)
}
