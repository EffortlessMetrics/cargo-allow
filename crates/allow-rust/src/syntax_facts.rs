use std::collections::BTreeMap;

use tree_sitter::Node;

use crate::syntax_kinds::{
    LintAttributeKind, PanicMacroInvocation, PanicMacroKind, PanicMethodCall, PanicMethodKind,
    RustLineScope, RustSyntaxFacts, UnsafeSyntaxConstruct, UnsafeSyntaxKind,
};
use crate::syntax_tree::{impl_container_name, node_text, parse_rust_syntax};
use crate::text::detect_attr;

pub(crate) fn syntax_facts(source: &str) -> RustSyntaxFacts {
    let Ok(tree) = parse_rust_syntax(source) else {
        return RustSyntaxFacts::default();
    };
    let mut facts = RustSyntaxFacts::default();
    collect_syntax_facts(tree.tree.root_node(), source, &mut facts);
    let mut module_path = Vec::new();
    let mut impl_path = Vec::new();
    collect_line_scopes(
        tree.tree.root_node(),
        source,
        &mut module_path,
        &mut impl_path,
        &mut facts.scopes,
    );
    facts
}

fn collect_syntax_facts(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if node.kind() == "index_expression" {
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
    if let Some((line, construct)) = unsafe_syntax_construct(node) {
        record_unsafe_construct(facts, line, construct);
    }
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
    if matches!(node.kind(), "attribute_item" | "inner_attribute_item") {
        if let Some(text) = node_text(source, node) {
            let line = node.start_position().row as u32 + 1;
            if let Some(kind) = lint_attribute_kind(text) {
                facts.lint_attributes.entry(line).or_default().push(kind);
            }
            if unsafe_attribute_text(text) {
                facts.unsafe_attribute_lines.insert(line);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_syntax_facts(child, source, facts);
    }
}

fn collect_line_scopes(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    impl_path: &mut Vec<String>,
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

    if node.kind() == "function_item" {
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
    for child in node.children(&mut cursor) {
        collect_line_scopes(child, source, module_path, impl_path, scopes);
    }
}

fn record_module_scope(
    node: Node<'_>,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    let span_len = end_line.saturating_sub(start_line) + 1;
    for line in start_line..=end_line {
        let candidate = RustLineScope {
            container: None,
            module_path: module_path.to_vec(),
            span_len,
        };
        merge_scope(scopes, line, candidate);
    }
}

fn record_container_scope(
    node: Node<'_>,
    name: &str,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    let span_len = end_line.saturating_sub(start_line) + 1;
    for line in start_line..=end_line {
        let candidate = RustLineScope {
            container: Some(name.to_string()),
            module_path: module_path.to_vec(),
            span_len,
        };
        merge_scope(scopes, line, candidate);
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

fn panic_macro_invocation(node: Node<'_>, source: &str) -> Option<(u32, PanicMacroInvocation)> {
    if node.kind() != "macro_invocation" {
        return None;
    }
    let macro_node = node.child_by_field_name("macro")?;
    let macro_text = node_text(source, macro_node)?;
    let base_name = macro_text.rsplit("::").next().unwrap_or(macro_text);
    let kind = PanicMacroKind::from_name(base_name)?;
    let start = macro_node.start_position();
    let base_offset = macro_text.len().saturating_sub(base_name.len()) as u32;
    Some((
        start.row as u32 + 1,
        PanicMacroInvocation {
            kind,
            column: start.column as u32 + base_offset + 1,
        },
    ))
}

fn panic_method_call(node: Node<'_>, source: &str) -> Option<(u32, PanicMethodCall)> {
    if node.kind() != "call_expression" {
        return None;
    }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "field_expression" {
        return None;
    }
    let field = function.child_by_field_name("field")?;
    let method_name = node_text(source, field)?;
    let kind = PanicMethodKind::from_name(method_name)?;
    let start = field.start_position();
    Some((
        start.row as u32 + 1,
        PanicMethodCall {
            kind,
            column: start.column as u32 + 1,
        },
    ))
}

fn unsafe_syntax_construct(node: Node<'_>) -> Option<(u32, UnsafeSyntaxConstruct)> {
    match node.kind() {
        "function_item" | "function_signature_item" => {
            unsafe_modifier_construct(node, "fn", UnsafeSyntaxKind::Fn)
        }
        "impl_item" => unsafe_modifier_construct(node, "impl", UnsafeSyntaxKind::Impl),
        "trait_item" => unsafe_modifier_construct(node, "trait", UnsafeSyntaxKind::Trait),
        "foreign_mod_item" => {
            unsafe_modifier_construct(node, "extern_modifier", UnsafeSyntaxKind::ExternBlock)
        }
        "unsafe_block" => {
            unsafe_child_point(node).map(|point| located_construct(point, UnsafeSyntaxKind::Block))
        }
        _ => None,
    }
}

fn unsafe_modifier_construct(
    node: Node<'_>,
    keyword_kind: &str,
    kind: UnsafeSyntaxKind,
) -> Option<(u32, UnsafeSyntaxConstruct)> {
    let mut unsafe_point = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == keyword_kind {
            return unsafe_point.map(|point| located_construct(point, kind));
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
    point: tree_sitter::Point,
    kind: UnsafeSyntaxKind,
) -> (u32, UnsafeSyntaxConstruct) {
    (
        point.row as u32 + 1,
        UnsafeSyntaxConstruct {
            kind,
            column: point.column as u32 + 1,
        },
    )
}

fn record_unsafe_construct(
    facts: &mut RustSyntaxFacts,
    line: u32,
    construct: UnsafeSyntaxConstruct,
) {
    let constructs = facts.unsafe_constructs.entry(line).or_default();
    if !constructs
        .iter()
        .any(|existing| existing.kind == construct.kind && existing.column == construct.column)
    {
        constructs.push(construct);
        constructs.sort_by_key(|construct| (construct.column, construct.kind.priority()));
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
