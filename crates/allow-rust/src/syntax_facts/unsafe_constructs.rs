use tree_sitter::Node;

use crate::syntax_kinds::{RustSyntaxFacts, UnsafeSyntaxConstruct, UnsafeSyntaxKind};
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
        "function_item" | "function_signature_item" => {
            unsafe_modifier_construct(node, "fn", UnsafeSyntaxKind::Fn, source)
        }
        "impl_item" => unsafe_modifier_construct(node, "impl", UnsafeSyntaxKind::Impl, source),
        "trait_item" => unsafe_modifier_construct(node, "trait", UnsafeSyntaxKind::Trait, source),
        "foreign_mod_item" => unsafe_modifier_construct(
            node,
            "extern_modifier",
            UnsafeSyntaxKind::ExternBlock,
            source,
        ),
        "unsafe_block" => unsafe_child_point(node)
            .map(|point| located_construct(source, point, UnsafeSyntaxKind::Block)),
        _ => None,
    }
}

fn unsafe_modifier_construct(
    node: Node<'_>,
    keyword_kind: &str,
    kind: UnsafeSyntaxKind,
    source: &str,
) -> Option<(u32, UnsafeSyntaxConstruct)> {
    let mut unsafe_point = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == keyword_kind {
            return unsafe_point.map(|point| located_construct(source, point, kind));
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

fn located_construct(
    source: &str,
    point: tree_sitter::Point,
    kind: UnsafeSyntaxKind,
) -> (u32, UnsafeSyntaxConstruct) {
    (
        point.row as u32 + 1,
        UnsafeSyntaxConstruct {
            kind,
            column: source_column(source, point.row, point.column),
        },
    )
}
