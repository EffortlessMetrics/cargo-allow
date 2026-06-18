use allow_core::normalize_snippet;
use tree_sitter::Node;

use crate::syntax_tree::node_text;

const RECEIVER_FINGERPRINT_LIMIT: usize = 80;
const TARGET_FINGERPRINT_LIMIT: usize = 40;

pub(super) fn structural_receiver_fingerprint(receiver: Node<'_>, source: &str) -> Option<String> {
    if receiver.kind() == "identifier" {
        if let Some(name) = node_text(source, receiver) {
            if let Some(index) = parameter_slot_for_name(receiver, source, name) {
                return Some(format!("param:{index}"));
            }
        }
    }
    node_text(source, receiver).and_then(truncate_receiver_fingerprint)
}

pub(super) fn index_target_fingerprint(node: Node<'_>, source: &str) -> Option<String> {
    index_selector_text(node, source)
        .map(normalize_snippet)
        .as_deref()
        .and_then(truncate_target_fingerprint)
}

fn index_selector_text<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name("index")
        .and_then(|index| node_text(source, index))
        .or_else(|| direct_index_selector_text(node, source))
}

fn direct_index_selector_text<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    let mut start = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "[" {
            start = Some(child.end_byte());
            continue;
        }
        if child.kind() == "]" {
            let start = start?;
            return source.get(start..child.start_byte());
        }
    }
    None
}

fn parameter_slot_for_name(use_site: Node<'_>, source: &str, name: &str) -> Option<usize> {
    let function = enclosing_function(use_site)?;
    function_parameter_names(function, source)
        .iter()
        .position(|candidate| candidate == name)
}

fn enclosing_function(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if matches!(node.kind(), "function_item" | "function_signature_item") {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn function_parameter_names(function: Node<'_>, source: &str) -> Vec<String> {
    let Some(parameters) = function.child_by_field_name("parameters") else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut cursor = parameters.walk();
    for child in parameters.children(&mut cursor) {
        if child.kind() != "parameter" {
            continue;
        }
        if let Some(name) = child
            .child_by_field_name("pattern")
            .and_then(|pattern| binding_name(pattern, source))
        {
            names.push(name);
        }
    }
    names
}

fn binding_name(pattern: Node<'_>, source: &str) -> Option<String> {
    match pattern.kind() {
        "identifier" => node_text(source, pattern).map(str::to_string),
        "mut_pattern" | "ref_pattern" | "reference_pattern" => pattern
            .child(0)
            .and_then(|inner| binding_name(inner, source)),
        "self" => Some("self".to_string()),
        _ => {
            let mut cursor = pattern.walk();
            pattern
                .children(&mut cursor)
                .find_map(|child| binding_name(child, source))
        }
    }
}

fn truncate_receiver_fingerprint(text: &str) -> Option<String> {
    truncate_fingerprint(text, RECEIVER_FINGERPRINT_LIMIT)
}

fn truncate_target_fingerprint(text: &str) -> Option<String> {
    truncate_fingerprint(text, TARGET_FINGERPRINT_LIMIT)
}

fn truncate_fingerprint(text: &str, limit: usize) -> Option<String> {
    let fingerprint = normalize_snippet(text);
    if fingerprint.is_empty() {
        return None;
    }
    Some(
        fingerprint
            .chars()
            .rev()
            .take(limit)
            .collect::<String>()
            .chars()
            .rev()
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax_tree::parse_rust_syntax;

    #[test]
    fn structural_receiver_fingerprint_uses_parameter_slot_for_identifiers() {
        let source = r#"
            fn load(value: Result<(), ()>, other: Result<(), ()>) {
                value.unwrap();
                other.expect("loaded");
            }
        "#;
        let tree = parse_rust_syntax(source)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parse fixture: {err}")));
        let receivers = receiver_fingerprints(tree.tree.root_node(), source);
        assert_eq!(receivers, vec!["param:0", "param:1"]);
    }

    #[test]
    fn structural_receiver_fingerprint_preserves_expression_shape() {
        let source = r#"
            fn load(builder: Builder) {
                builder.step().unwrap();
            }
        "#;
        let tree = parse_rust_syntax(source)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parse fixture: {err}")));
        let receivers = receiver_fingerprints(tree.tree.root_node(), source);
        assert_eq!(receivers, vec!["builder.step()"]);
    }

    #[test]
    fn index_target_fingerprint_records_selector_not_receiver() {
        let source = r#"
            fn load(left: &[u8], right: &[u8]) -> u8 {
                left[0] + right[1]
            }
        "#;
        let tree = parse_rust_syntax(source)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parse fixture: {err}")));
        let targets = index_targets(tree.tree.root_node(), source);
        assert_eq!(targets, vec!["0", "1"]);
    }

    fn receiver_fingerprints(root: Node<'_>, source: &str) -> Vec<String> {
        let mut cursor = root.walk();
        let mut fingerprints = Vec::new();
        collect_receiver_fingerprints(root, source, &mut fingerprints, &mut cursor);
        fingerprints
    }

    fn collect_receiver_fingerprints(
        node: Node<'_>,
        source: &str,
        fingerprints: &mut Vec<String>,
        cursor: &mut tree_sitter::TreeCursor<'_>,
    ) {
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                if function.kind() == "field_expression" {
                    if let Some(field) = function.child_by_field_name("field") {
                        if let Some(method_name) = node_text(source, field) {
                            if matches!(method_name, "unwrap" | "expect") {
                                if let Some(receiver) = function.child_by_field_name("value") {
                                    if let Some(fingerprint) =
                                        structural_receiver_fingerprint(receiver, source)
                                    {
                                        fingerprints.push(fingerprint);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if cursor.goto_first_child() {
            loop {
                collect_receiver_fingerprints(cursor.node(), source, fingerprints, cursor);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn index_targets(root: Node<'_>, source: &str) -> Vec<String> {
        let mut cursor = root.walk();
        let mut targets = Vec::new();
        collect_index_targets(root, source, &mut targets, &mut cursor);
        targets
    }

    fn collect_index_targets(
        node: Node<'_>,
        source: &str,
        targets: &mut Vec<String>,
        cursor: &mut tree_sitter::TreeCursor<'_>,
    ) {
        if node.kind() == "index_expression" {
            if let Some(target) = index_target_fingerprint(node, source) {
                targets.push(target);
            }
        }
        if cursor.goto_first_child() {
            loop {
                collect_index_targets(cursor.node(), source, targets, cursor);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }
}
