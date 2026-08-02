use allow_core::normalize_snippet;
use tree_sitter::Node;

use crate::syntax_kinds::{RustSyntaxFacts, UnsafeSyntaxConstruct, UnsafeSyntaxKind};
use crate::syntax_tree::{extern_container_name, impl_container_name, node_text};
use crate::text::{SourceLineIndex, source_column};

pub(super) fn record_node_unsafe_construct(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    facts: &mut RustSyntaxFacts,
) {
    let Some((line, construct)) = unsafe_syntax_construct(node, source, line_index) else {
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

fn unsafe_syntax_construct(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
) -> Option<(u32, UnsafeSyntaxConstruct)> {
    match node.kind() {
        "function_item" | "function_signature_item" => unsafe_modifier_construct(
            node,
            "fn",
            UnsafeSyntaxKind::Fn,
            item_name(node, source),
            source,
            line_index,
        ),
        "impl_item" => unsafe_modifier_construct(
            node,
            "impl",
            UnsafeSyntaxKind::Impl,
            impl_container_name(node, source),
            source,
            line_index,
        ),
        "trait_item" => unsafe_modifier_construct(
            node,
            "trait",
            UnsafeSyntaxKind::Trait,
            item_name(node, source),
            source,
            line_index,
        ),
        "foreign_mod_item" => unsafe_modifier_construct(
            node,
            "extern_modifier",
            UnsafeSyntaxKind::ExternBlock,
            extern_block_symbol(node, source),
            source,
            line_index,
        ),
        // const unsafe / static unsafe items (#1794)
        "const_item" | "static_item" => unsafe_modifier_construct(
            node,
            "const",
            UnsafeSyntaxKind::Const,
            item_name(node, source),
            source,
            line_index,
        )
        .or_else(|| {
            unsafe_modifier_construct(
                node,
                "static",
                UnsafeSyntaxKind::Static,
                item_name(node, source),
                source,
                line_index,
            )
        }),
        "unsafe_block" => unsafe_child_point(node).map(|point| {
            located_construct(line_index, source, point, UnsafeSyntaxKind::Block, None)
        }),
        _ => None,
    }
}

fn unsafe_modifier_construct(
    node: Node<'_>,
    keyword_kind: &str,
    kind: UnsafeSyntaxKind,
    symbol: Option<String>,
    source: &str,
    line_index: &SourceLineIndex,
) -> Option<(u32, UnsafeSyntaxConstruct)> {
    let mut unsafe_point = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == keyword_kind {
            return unsafe_point
                .map(|point| located_construct(line_index, source, point, kind, symbol.clone()));
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

fn extern_block_symbol(node: Node<'_>, source: &str) -> Option<String> {
    let context = extern_container_name(node, source).unwrap_or_else(|| "extern".to_string());
    let mut item_names = Vec::new();
    collect_foreign_item_names(node, source, &mut item_names);
    let symbol = if item_names.is_empty() {
        context
    } else {
        format!("{context}:{}", item_names.join(","))
    };
    let symbol = normalize_snippet(&symbol);
    (!symbol.is_empty()).then_some(symbol)
}

fn collect_foreign_item_names(node: Node<'_>, source: &str, item_names: &mut Vec<String>) {
    if matches!(
        node.kind(),
        "function_signature_item" | "static_item" | "type_item"
    ) && let Some(name) = item_name(node, source)
    {
        item_names.push(name);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_foreign_item_names(child, source, item_names);
    }
}

fn located_construct(
    line_index: &SourceLineIndex,
    source: &str,
    point: tree_sitter::Point,
    kind: UnsafeSyntaxKind,
    symbol: Option<String>,
) -> (u32, UnsafeSyntaxConstruct) {
    (
        point.row as u32 + 1,
        UnsafeSyntaxConstruct {
            kind,
            column: source_column(line_index, source, point.row, point.column),
            symbol,
        },
    )
}
