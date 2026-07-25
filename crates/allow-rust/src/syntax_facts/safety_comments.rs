use tree_sitter::Node;

use crate::safety_comments::comment_node_text_is_safety_marker;
use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::node_text;

pub(super) fn record_node_safety_comments(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if !matches!(node.kind(), "line_comment" | "block_comment") {
        return;
    }

    let Some(text) = node_text(source, node) else {
        return;
    };
    if !comment_node_text_is_safety_marker(text, node.kind()) {
        return;
    }

    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    for line in start_line..=end_line {
        facts.safety_comment_lines.insert(line);
    }
}
