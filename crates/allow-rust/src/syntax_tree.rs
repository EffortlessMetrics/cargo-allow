use allow_core::{CargoAllowError, CargoAllowResult};
use tree_sitter::{Node, Parser, Tree};

use crate::text::{SourceLineIndex, source_column};

pub struct RustSyntaxTree {
    pub(crate) tree: Tree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSyntaxContainer {
    pub kind: String,
    pub name: String,
    pub module_path: Vec<String>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

struct ContainerState<'a> {
    module_path: &'a mut Vec<String>,
    impl_path: &'a mut Vec<String>,
    trait_path: &'a mut Vec<String>,
    extern_path: &'a mut Vec<String>,
    containers: &'a mut Vec<RustSyntaxContainer>,
}

impl RustSyntaxContainer {
    pub fn module(&self) -> Option<String> {
        if self.module_path.is_empty() {
            None
        } else {
            Some(self.module_path.join("::"))
        }
    }
}

impl RustSyntaxTree {
    pub fn root_kind(&self) -> &'static str {
        self.tree.root_node().kind()
    }

    pub fn has_error(&self) -> bool {
        self.tree.root_node().has_error()
    }

    pub fn named_node_count(&self) -> usize {
        named_node_count(self.tree.root_node())
    }

    pub fn containers(&self, source: &str) -> Vec<RustSyntaxContainer> {
        let mut containers = Vec::new();
        let mut module_path = Vec::new();
        let mut impl_path = Vec::new();
        let mut trait_path = Vec::new();
        let mut extern_path = Vec::new();
        let line_index = SourceLineIndex::new(source);
        let mut state = ContainerState {
            module_path: &mut module_path,
            impl_path: &mut impl_path,
            trait_path: &mut trait_path,
            extern_path: &mut extern_path,
            containers: &mut containers,
        };
        collect_containers(self.tree.root_node(), source, &line_index, &mut state);
        containers
    }
}

pub fn parse_rust_syntax(source: &str) -> CargoAllowResult<RustSyntaxTree> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|e| CargoAllowError::new(format!("failed to load Rust parser: {e}")))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CargoAllowError::new("failed to parse Rust source"))?;
    Ok(RustSyntaxTree { tree })
}

pub(crate) fn node_text<'a>(source: &'a str, node: Node<'a>) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}

pub(crate) fn impl_container_name(node: Node<'_>, source: &str) -> Option<String> {
    let impl_type = node
        .child_by_field_name("type")
        .and_then(|type_node| node_text(source, type_node))
        .map(normalize_scope_text)?;
    if let Some(trait_name) = node
        .child_by_field_name("trait")
        .and_then(|trait_node| node_text(source, trait_node))
        .map(normalize_scope_text)
    {
        Some(format!("<{impl_type} as {trait_name}>"))
    } else {
        Some(impl_type)
    }
}

pub(crate) fn extern_container_name(node: Node<'_>, source: &str) -> Option<String> {
    let name = if let Some(abi) = extern_abi(node, source) {
        format!("extern {abi}")
    } else {
        "extern".to_string()
    };
    let name = normalize_scope_text(&name);
    (!name.is_empty()).then_some(name)
}

fn extern_abi(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() == "string_literal" {
        return node_text(source, node)
            .map(normalize_scope_text)
            .filter(|abi| !abi.is_empty());
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find_map(|child| extern_abi(child, source))
}

fn named_node_count(node: Node<'_>) -> usize {
    let mut cursor = node.walk();
    let children = node
        .children(&mut cursor)
        .map(named_node_count)
        .sum::<usize>();
    if node.is_named() {
        children + 1
    } else {
        children
    }
}

fn collect_containers(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    state: &mut ContainerState<'_>,
) {
    if node.kind() == "mod_item"
        && let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
    {
        state.module_path.push(name.to_string());
        visit_child_containers(node, source, line_index, state);
        state.module_path.pop();
        return;
    }

    if node.kind() == "impl_item"
        && let Some(name) = impl_container_name(node, source)
    {
        state.impl_path.push(name);
        visit_child_containers(node, source, line_index, state);
        state.impl_path.pop();
        return;
    }

    if node.kind() == "trait_item"
        && let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
            .map(normalize_scope_text)
    {
        state.trait_path.push(name);
        visit_child_containers(node, source, line_index, state);
        state.trait_path.pop();
        return;
    }

    if node.kind() == "foreign_mod_item"
        && let Some(name) = extern_container_name(node, source)
    {
        state.extern_path.push(name);
        visit_child_containers(node, source, line_index, state);
        state.extern_path.pop();
        return;
    }

    if matches!(node.kind(), "function_item" | "function_signature_item")
        && let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
    {
        let (kind, name) = if let Some(impl_name) = state.impl_path.last() {
            ("method", format!("{impl_name}::{name}"))
        } else if let Some(trait_name) = state.trait_path.last() {
            ("method", format!("{trait_name}::{name}"))
        } else if let Some(extern_name) = state.extern_path.last() {
            ("function", format!("{extern_name}::{name}"))
        } else {
            ("function", name.to_string())
        };
        let start = node.start_position();
        let end = node.end_position();
        state.containers.push(RustSyntaxContainer {
            kind: kind.to_string(),
            name,
            module_path: state.module_path.clone(),
            start_line: start.row as u32 + 1,
            start_column: source_column(line_index, source, start.row, start.column),
            end_line: end.row as u32 + 1,
            end_column: source_column(line_index, source, end.row, end.column),
        });
    }

    visit_child_containers(node, source, line_index, state);
}

fn visit_child_containers(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    state: &mut ContainerState<'_>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_containers(child, source, line_index, state);
    }
}

fn normalize_scope_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
