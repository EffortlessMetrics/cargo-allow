use tree_sitter::Node;

use crate::syntax_facts::{attributes, index, panic, safety_comments, unsafe_constructs};
use crate::syntax_kinds::RustSyntaxFacts;

pub(super) fn collect_syntax_facts(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    index::record_index_expression(node, source, facts);
    unsafe_constructs::record_node_unsafe_construct(node, source, facts);
    panic::record_node_panic_constructs(node, source, facts);
    attributes::record_node_attributes(node, source, facts);
    safety_comments::record_node_safety_comments(node, source, facts);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_syntax_facts(child, source, facts);
    }
}
