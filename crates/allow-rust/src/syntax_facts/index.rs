use allow_core::normalize_snippet;
use tree_sitter::{Node, Point};

use crate::syntax_facts::fingerprint::{index_target_fingerprint, structural_receiver_fingerprint};
use crate::syntax_kinds::{IndexExpression, RustSyntaxFacts};
use crate::syntax_tree::node_text;
use crate::text::{SourceLineIndex, source_column};

pub(super) fn record_index_expression(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    facts: &mut RustSyntaxFacts,
) {
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
    let column = source_column(line_index, source, bracket_point.row, bracket_point.column);
    let receiver_node = node
        .child_by_field_name("value")
        .or_else(|| direct_index_receiver_node(node));
    let receiver_fingerprint =
        receiver_node.and_then(|receiver| structural_receiver_fingerprint(receiver, source));
    let expression = IndexExpression {
        column,
        symbol: index_expression_symbol(node, source),
        target_fingerprint: index_target_fingerprint(node, source),
        receiver_fingerprint,
        is_slice: index_selector_is_slice(node, source),
    };
    let expressions = facts.index_expressions.entry(line).or_default();
    if !expressions.contains(&expression) {
        expressions.push(expression);
        expressions.sort_by_key(|expression| expression.column);
    }
}

fn index_expression_symbol(node: Node<'_>, source: &str) -> String {
    node_text(source, node)
        .map(normalize_snippet)
        .map(|symbol| symbol.chars().take(100).collect())
        .unwrap_or_default()
}

fn index_selector_is_slice(node: Node<'_>, source: &str) -> bool {
    node.child_by_field_name("index")
        .and_then(|index| node_text(source, index))
        .or_else(|| direct_index_selector_text(node, source))
        .is_some_and(|selector| selector.contains(".."))
}

fn direct_index_selector_text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    let mut start = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "[" {
            start = Some(child.end_byte());
            continue;
        }
        if child.kind() == "]" {
            let start = start?;
            return source.get(start..child.start_byte());
        }
    }
    None
}

fn direct_index_bracket_point(node: Node<'_>) -> Option<Point> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "[")
        .map(|child| child.start_position())
}

fn direct_index_receiver_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .take_while(|child| child.kind() != "[")
        .find(|child| child.is_named())
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
