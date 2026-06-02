use std::collections::BTreeMap;

use tree_sitter::Node;

use crate::syntax_kinds::RustLineScope;
use crate::syntax_tree::{impl_container_name, node_text};

pub(super) fn collect_line_scopes(
    node: Node<'_>,
    source: &str,
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let mut module_path = Vec::new();
    let mut impl_path = Vec::new();
    collect_nested_line_scopes(node, source, &mut module_path, &mut impl_path, &[], scopes);
}

fn collect_nested_line_scopes(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    impl_path: &mut Vec<String>,
    outer_attribute_lines: &[(u32, u32)],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    if node.kind() == "mod_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            module_path.push(name.to_string());
            record_module_scope(node, module_path, scopes);
            visit_child_scopes(node, source, module_path, impl_path, scopes);
            module_path.pop();
            return;
        }
    }

    if node.kind() == "impl_item" {
        if let Some(name) = impl_container_name(node, source) {
            impl_path.push(name);
            visit_child_scopes(node, source, module_path, impl_path, scopes);
            impl_path.pop();
            return;
        }
    }

    if matches!(node.kind(), "function_item" | "function_signature_item") {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            let container = if let Some(impl_name) = impl_path.last() {
                format!("{impl_name}::{name}")
            } else {
                name.to_string()
            };
            record_container_scope(node, &container, module_path, scopes);
            record_attribute_line_scopes(outer_attribute_lines, &container, module_path, scopes);
            record_outer_attribute_scopes(node, &container, module_path, scopes);
        }
    }

    visit_child_scopes(node, source, module_path, impl_path, scopes);
}

fn visit_child_scopes(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    impl_path: &mut Vec<String>,
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let mut cursor = node.walk();
    let mut pending_outer_attribute_lines = Vec::new();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" {
            pending_outer_attribute_lines.push(node_line_range(child));
            continue;
        }
        collect_nested_line_scopes(
            child,
            source,
            module_path,
            impl_path,
            &pending_outer_attribute_lines,
            scopes,
        );
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
