use tree_sitter::Node;

use crate::syntax_facts::{attributes, index, panic, safety_comments, unsafe_constructs};
use crate::syntax_kinds::RustSyntaxFacts;
use crate::text::SourceLineIndex;

pub(super) fn collect_syntax_facts(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    facts: &mut RustSyntaxFacts,
) {
    index::record_index_expression(node, source, line_index, facts);
    unsafe_constructs::record_node_unsafe_construct(node, source, line_index, facts);
    panic::record_node_panic_constructs(node, source, line_index, facts);
    attributes::record_node_attributes(node, source, line_index, facts);
    safety_comments::record_node_safety_comments(node, source, facts);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_syntax_facts(child, source, line_index, facts);
    }
}
