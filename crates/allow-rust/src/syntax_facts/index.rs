use tree_sitter::Node;

use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::node_text;

pub(super) fn record_index_expression(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if node.kind() != "index_expression" {
        return;
    }

    let start = node.start_position();
    let bracket_offset = node_text(source, node)
        .and_then(|text| text.find('['))
        .unwrap_or(0);
    let line = start.row as u32 + 1;
    let column = start.column as u32 + bracket_offset as u32 + 1;
    facts
        .index_columns
        .entry(line)
        .and_modify(|existing| *existing = (*existing).min(column))
        .or_insert(column);
}
