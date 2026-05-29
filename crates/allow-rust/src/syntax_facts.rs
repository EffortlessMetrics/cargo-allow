use tree_sitter::Node;

use crate::syntax_facts::attributes::{lint_attribute_kind, unsafe_attribute_text};
use crate::syntax_facts::panic::{panic_macro_invocation, panic_method_call};
use crate::syntax_facts::scopes::collect_line_scopes;
use crate::syntax_facts::unsafe_constructs::{record_unsafe_construct, unsafe_syntax_construct};
use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::{node_text, parse_rust_syntax};

mod attributes;
mod panic;
mod scopes;
mod unsafe_constructs;

pub(crate) fn syntax_facts(source: &str) -> RustSyntaxFacts {
    let Ok(tree) = parse_rust_syntax(source) else {
        return RustSyntaxFacts::default();
    };
    let mut facts = RustSyntaxFacts::default();
    collect_syntax_facts(tree.tree.root_node(), source, &mut facts);
    collect_line_scopes(tree.tree.root_node(), source, &mut facts.scopes);
    facts
}

fn collect_syntax_facts(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    collect_index_expression(node, source, facts);
    collect_unsafe_construct(node, facts);
    collect_panic_invocation(node, source, facts);
    collect_lint_attribute(node, source, facts);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_syntax_facts(child, source, facts);
    }
}

fn collect_index_expression(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
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

fn collect_unsafe_construct(node: Node<'_>, facts: &mut RustSyntaxFacts) {
    if let Some((line, construct)) = unsafe_syntax_construct(node) {
        record_unsafe_construct(facts, line, construct);
    }
}

fn collect_panic_invocation(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
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

fn collect_lint_attribute(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if !matches!(node.kind(), "attribute_item" | "inner_attribute_item") {
        return;
    }

    let Some(text) = node_text(source, node) else {
        return;
    };

    let line = node.start_position().row as u32 + 1;
    if let Some(kind) = lint_attribute_kind(text) {
        facts.lint_attributes.entry(line).or_default().push(kind);
    }
    if unsafe_attribute_text(text) {
        facts.unsafe_attribute_lines.insert(line);
    }
}
