use allow_core::normalize_snippet;
use tree_sitter::Node;

use crate::syntax_kinds::{RustSyntaxFacts, UnsafeSyntaxConstruct, UnsafeSyntaxKind};
use crate::syntax_tree::node_text;
use crate::text::source_column;

pub(super) fn record_node_unsafe_construct(
    node: Node<'_>,
    source: &str,
    facts: &mut RustSyntaxFacts,
) {
    let Some((line, construct)) = unsafe_syntax_construct(node, source) else {
        return;
    };

    let constructs = facts.unsafe_constructs.entry(line).or_default();
    if !constructs
        .iter()
        .any(|existing| existing.kind == construct.kind && existing.column == construct.column)
    {
        constructs.push(construct);
        constructs.sort_by_key(|construct| (construct.column, construct.kind.priority()));
    }
}

fn unsafe_syntax_construct(node: Node<'_>, source: &str) -> Option<(u32, UnsafeSyntaxConstruct)> {
    match node.kind() {
        "function_item" | "function_signature_item" => unsafe_modifier_construct(
            node,
            "fn",
            UnsafeSyntaxKind::Fn,
            item_name(node, source),
            source,
        ),
        "impl_item" => {
            unsafe_modifier_construct(node, "impl", UnsafeSyntaxKind::Impl, None, source)
        }
        "trait_item" => {
            unsafe_modifier_construct(node, "trait", UnsafeSyntaxKind::Trait, None, source)
        }
        "foreign_mod_item" => unsafe_modifier_construct(
            node,
            "extern_modifier",
            UnsafeSyntaxKind::ExternBlock,
            None,
            source,
        ),
        "unsafe_block" => unsafe_child_point(node)
            .map(|point| located_construct(source, point, UnsafeSyntaxKind::Block, None)),
        _ => None,
    }
}

fn unsafe_modifier_construct(
    node: Node<'_>,
    keyword_kind: &str,
    kind: UnsafeSyntaxKind,
    symbol: Option<String>,
    source: &str,
) -> Option<(u32, UnsafeSyntaxConstruct)> {
    let mut unsafe_point = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == keyword_kind {
            return unsafe_point
                .map(|point| located_construct(source, point, kind, symbol.clone()));
        }
        if unsafe_point.is_none() {
            unsafe_point = unsafe_child_point(child);
        }
    }
    None
}

fn unsafe_child_point(node: Node<'_>) -> Option<tree_sitter::Point> {
    if node.kind() == "unsafe" {
        return Some(node.start_position());
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find_map(|child| (child.kind() == "unsafe").then(|| child.start_position()))
}

fn item_name(node: Node<'_>, source: &str) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    let symbol = normalize_snippet(node_text(source, name)?);
    (!symbol.is_empty()).then_some(symbol)
}

fn located_construct(
    source: &str,
    point: tree_sitter::Point,
    kind: UnsafeSyntaxKind,
    symbol: Option<String>,
) -> (u32, UnsafeSyntaxConstruct) {
    (
        point.row as u32 + 1,
        UnsafeSyntaxConstruct {
            kind,
            column: source_column(source, point.row, point.column),
            symbol,
        },
    )
}
