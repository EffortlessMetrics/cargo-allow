use tree_sitter::{Node, Point};

use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::node_text;
use crate::text::source_column;

pub(super) fn record_index_expression(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if node.kind() != "index_expression" {
        return;
    }

    let start = node.start_position();
    let text = node_text(source, node).unwrap_or_default();
    let bracket_point = direct_index_bracket_point(node).unwrap_or_else(|| {
        let bracket_offset = text.find('[').unwrap_or(0);
        let (row_delta, byte_column) = offset_position(text, bracket_offset);
        Point {
            row: start.row + row_delta,
            column: if row_delta == 0 {
                start.column + byte_column
            } else {
                byte_column
            },
        }
    });
    let line = bracket_point.row as u32 + 1;
    let column = source_column(source, bracket_point.row, bracket_point.column);
    facts
        .index_columns
        .entry(line)
        .and_modify(|existing| *existing = (*existing).min(column))
        .or_insert(column);
}

fn direct_index_bracket_point(node: Node<'_>) -> Option<Point> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "[")
        .map(|child| child.start_position())
}

fn offset_position(text: &str, offset: usize) -> (usize, usize) {
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
