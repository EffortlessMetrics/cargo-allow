use crate::syntax_tree::{node_text, parse_rust_syntax};
use crate::text::{SourceLineIndex, source_column};
use allow_core::CargoAllowResult;
use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustSourceCouplingKind {
    UseDeclaration,
    InlineModule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceCoupling {
    pub kind: RustSourceCouplingKind,
    pub path: String,
    pub text: String,
    pub start_line: u32,
    pub start_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceCouplingScan {
    pub facts: Vec<RustSourceCoupling>,
    pub has_parse_error: bool,
}

pub fn scan_rust_source_coupling(source: &str) -> CargoAllowResult<RustSourceCouplingScan> {
    let tree = parse_rust_syntax(source)?;
    let line_index = SourceLineIndex::new(source);
    let mut facts = Vec::new();
    collect_coupling_facts(tree.tree.root_node(), source, &line_index, &mut facts);
    Ok(RustSourceCouplingScan {
        facts,
        has_parse_error: tree.has_error(),
    })
}

fn collect_coupling_facts(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    facts: &mut Vec<RustSourceCoupling>,
) {
    let kind = match node.kind() {
        "use_declaration" => Some(RustSourceCouplingKind::UseDeclaration),
        "mod_item" => Some(RustSourceCouplingKind::InlineModule),
        _ => None,
    };
    if let Some(kind) = kind
        && (kind != RustSourceCouplingKind::InlineModule
            || node.child_by_field_name("body").is_some())
        && let Some(text) = node_text(source, node)
    {
        let paths = match kind {
            RustSourceCouplingKind::UseDeclaration => node
                .child_by_field_name("argument")
                .map(|argument| use_clause_paths(argument, source))
                .unwrap_or_default(),
            RustSourceCouplingKind::InlineModule => node
                .child_by_field_name("name")
                .and_then(|name| node_text(source, name))
                .map(|path| vec![path.trim().to_string()])
                .unwrap_or_default(),
        };
        let start = node.start_position();
        for path in paths {
            if path.is_empty() {
                continue;
            }
            facts.push(RustSourceCoupling {
                kind,
                path,
                text: text.trim().to_string(),
                start_line: start.row as u32 + 1,
                start_column: source_column(line_index, source, start.row, start.column),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_coupling_facts(child, source, line_index, facts);
    }
}

fn use_clause_paths(node: Node<'_>, source: &str) -> Vec<String> {
    match node.kind() {
        "use_list" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .flat_map(|child| use_clause_paths(child, source))
                .collect()
        }
        "scoped_use_list" => {
            let prefix = node
                .child_by_field_name("path")
                .and_then(|path| node_text(source, path))
                .map(str::trim)
                .filter(|path| !path.is_empty());
            let Some(list) = node.child_by_field_name("list") else {
                return prefix.into_iter().map(str::to_string).collect();
            };
            let paths = use_clause_paths(list, source);
            match prefix {
                Some(prefix) => paths
                    .into_iter()
                    .map(|path| format!("{prefix}::{path}"))
                    .collect(),
                None => paths,
            }
        }
        "use_as_clause" => node
            .child_by_field_name("path")
            .and_then(|path| node_text(source, path))
            .map(|path| vec![path.trim().to_string()])
            .unwrap_or_default(),
        _ => node_text(source, node)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|path| vec![path.to_string()])
            .unwrap_or_default(),
    }
}
