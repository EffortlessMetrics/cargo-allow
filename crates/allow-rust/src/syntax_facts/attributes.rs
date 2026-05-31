use tree_sitter::Node;

use crate::syntax_kinds::{LintAttribute, LintAttributeKind, RustSyntaxFacts};
use crate::syntax_tree::node_text;
use crate::text::{detect_attr, source_column};

pub(super) fn record_node_attributes(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if !matches!(node.kind(), "attribute_item" | "inner_attribute_item") {
        return;
    }

    let Some(text) = node_text(source, node) else {
        return;
    };

    let start = node.start_position();
    let line = start.row as u32 + 1;
    if let Some(kind) = lint_attribute_kind(text) {
        facts
            .lint_attributes
            .entry(line)
            .or_default()
            .push(LintAttribute {
                kind,
                text: text.to_string(),
                column: source_column(source, start.row, start.column),
            });
    }
    if unsafe_attribute_text(text) {
        facts.unsafe_attribute_lines.insert(line);
    }
}

fn lint_attribute_kind(text: &str) -> Option<LintAttributeKind> {
    let trimmed = text.trim_start();
    if detect_attr(trimmed, "allow").is_some() {
        Some(LintAttributeKind::Allow)
    } else if detect_attr(trimmed, "expect").is_some() {
        Some(LintAttributeKind::Expect)
    } else {
        None
    }
}

fn unsafe_attribute_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("#[unsafe(") || trimmed.starts_with("#![unsafe(")
}
