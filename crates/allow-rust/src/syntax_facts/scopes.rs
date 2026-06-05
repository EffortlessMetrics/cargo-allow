use std::collections::BTreeMap;

use tree_sitter::Node;

use crate::syntax_kinds::RustLineScope;
use crate::syntax_tree::{extern_container_name, impl_container_name, node_text};

pub(super) fn collect_line_scopes(
    node: Node<'_>,
    source: &str,
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let mut paths = ScopePaths::default();
    collect_nested_line_scopes(node, source, &mut paths, &[], scopes);
}

#[derive(Default)]
struct ScopePaths {
    module_path: Vec<String>,
    impl_path: Vec<String>,
    trait_path: Vec<String>,
    extern_path: Vec<String>,
}

fn collect_nested_line_scopes(
    node: Node<'_>,
    source: &str,
    paths: &mut ScopePaths,
    outer_attribute_lines: &[(u32, u32)],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    if node.kind() == "mod_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            paths.module_path.push(name.to_string());
            record_module_scope(node, &paths.module_path, scopes);
            record_attribute_line_module_scopes(outer_attribute_lines, &paths.module_path, scopes);
            record_outer_module_attribute_scopes(node, &paths.module_path, scopes);
            visit_child_scopes(node, source, paths, scopes);
            paths.module_path.pop();
            return;
        }
    }

    if node.kind() == "impl_item" {
        if let Some(name) = impl_container_name(node, source) {
            record_container_scope(node, &name, &paths.module_path, scopes);
            record_attribute_line_scopes(outer_attribute_lines, &name, &paths.module_path, scopes);
            record_outer_attribute_scopes(node, &name, &paths.module_path, scopes);
            paths.impl_path.push(name);
            visit_child_scopes(node, source, paths, scopes);
            paths.impl_path.pop();
            return;
        }
    }

    if node.kind() == "trait_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            record_container_scope(node, name, &paths.module_path, scopes);
            record_attribute_line_scopes(outer_attribute_lines, name, &paths.module_path, scopes);
            record_outer_attribute_scopes(node, name, &paths.module_path, scopes);
            paths.trait_path.push(name.to_string());
            visit_child_scopes(node, source, paths, scopes);
            paths.trait_path.pop();
            return;
        }
    }

    if node.kind() == "foreign_mod_item" {
        if let Some(name) = extern_container_name(node, source) {
            record_container_scope(node, &name, &paths.module_path, scopes);
            record_attribute_line_scopes(outer_attribute_lines, &name, &paths.module_path, scopes);
            paths.extern_path.push(name);
            visit_child_scopes(node, source, paths, scopes);
            paths.extern_path.pop();
            return;
        }
    }

    if matches!(node.kind(), "function_item" | "function_signature_item") {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            let container = if let Some(impl_name) = paths.impl_path.last() {
                format!("{impl_name}::{name}")
            } else if let Some(trait_name) = paths.trait_path.last() {
                format!("{trait_name}::{name}")
            } else if let Some(extern_name) = paths.extern_path.last() {
                format!("{extern_name}::{name}")
            } else {
                name.to_string()
            };
            record_container_scope(node, &container, &paths.module_path, scopes);
            record_attribute_line_scopes(
                outer_attribute_lines,
                &container,
                &paths.module_path,
                scopes,
            );
            record_outer_attribute_scopes(node, &container, &paths.module_path, scopes);
        }
    }

    if let Some(name) = named_item_container_name(node, source) {
        record_container_scope(node, &name, &paths.module_path, scopes);
        record_attribute_line_scopes(outer_attribute_lines, &name, &paths.module_path, scopes);
        record_outer_attribute_scopes(node, &name, &paths.module_path, scopes);
    }

    if let Some(name) = use_declaration_container_name(node, source) {
        record_container_scope(node, &name, &paths.module_path, scopes);
        record_attribute_line_scopes(outer_attribute_lines, &name, &paths.module_path, scopes);
        record_outer_attribute_scopes(node, &name, &paths.module_path, scopes);
    }

    visit_child_scopes(node, source, paths, scopes);
}

fn named_item_container_name(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "struct_item" | "enum_item" | "union_item" | "type_item" | "const_item" | "static_item"
    ) {
        return None;
    }
    node.child_by_field_name("name")
        .and_then(|name| node_text(source, name))
        .map(str::to_string)
}

fn use_declaration_container_name(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() != "use_declaration" {
        return None;
    }
    node_text(source, node).and_then(normalize_item_text)
}

fn normalize_item_text(text: &str) -> Option<String> {
    let normalized = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(';')
        .trim()
        .to_string();
    (!normalized.is_empty()).then_some(normalized)
}

fn visit_child_scopes(
    node: Node<'_>,
    source: &str,
    paths: &mut ScopePaths,
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let mut cursor = node.walk();
    let mut pending_outer_attribute_lines = Vec::new();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" {
            pending_outer_attribute_lines.push(node_line_range(child));
            continue;
        }
        collect_nested_line_scopes(child, source, paths, &pending_outer_attribute_lines, scopes);
        pending_outer_attribute_lines.clear();
    }
}

fn record_module_scope(
    node: Node<'_>,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    record_scope_lines(node, scopes, |span_len| RustLineScope {
        container: None,
        module_path: module_path.to_vec(),
        span_len,
    });
}

fn record_outer_module_attribute_scopes(
    node: Node<'_>,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" {
            record_scope_lines(child, scopes, |span_len| RustLineScope {
                container: None,
                module_path: module_path.to_vec(),
                span_len,
            });
        }
    }
}

fn record_container_scope(
    node: Node<'_>,
    name: &str,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    record_scope_lines(node, scopes, |span_len| RustLineScope {
        container: Some(name.to_string()),
        module_path: module_path.to_vec(),
        span_len,
    });
}

fn record_outer_attribute_scopes(
    node: Node<'_>,
    name: &str,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" {
            record_scope_lines(child, scopes, |span_len| RustLineScope {
                container: Some(name.to_string()),
                module_path: module_path.to_vec(),
                span_len,
            });
        }
    }
}

fn record_attribute_line_scopes(
    line_ranges: &[(u32, u32)],
    name: &str,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    for (start, end) in line_ranges {
        let span_len = end.saturating_sub(*start) + 1;
        for line in *start..=*end {
            merge_scope(
                scopes,
                line,
                RustLineScope {
                    container: Some(name.to_string()),
                    module_path: module_path.to_vec(),
                    span_len,
                },
            );
        }
    }
}

fn record_attribute_line_module_scopes(
    line_ranges: &[(u32, u32)],
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    for (start, end) in line_ranges {
        let span_len = end.saturating_sub(*start) + 1;
        for line in *start..=*end {
            merge_scope(
                scopes,
                line,
                RustLineScope {
                    container: None,
                    module_path: module_path.to_vec(),
                    span_len,
                },
            );
        }
    }
}

fn node_line_range(node: Node<'_>) -> (u32, u32) {
    (
        node.start_position().row as u32 + 1,
        node.end_position().row as u32 + 1,
    )
}

fn record_scope_lines(
    node: Node<'_>,
    scopes: &mut BTreeMap<u32, RustLineScope>,
    scope_for_span: impl Fn(u32) -> RustLineScope,
) {
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    let span_len = end_line.saturating_sub(start_line) + 1;
    for line in start_line..=end_line {
        merge_scope(scopes, line, scope_for_span(span_len));
    }
}

fn merge_scope(scopes: &mut BTreeMap<u32, RustLineScope>, line: u32, candidate: RustLineScope) {
    scopes
        .entry(line)
        .and_modify(|existing| {
            let candidate_has_container = candidate.container.is_some();
            let existing_has_container = existing.container.is_some();
            if (candidate_has_container && !existing_has_container)
                || (candidate_has_container == existing_has_container
                    && candidate.span_len < existing.span_len)
            {
                *existing = candidate.clone();
            }
        })
        .or_insert(candidate);
}
