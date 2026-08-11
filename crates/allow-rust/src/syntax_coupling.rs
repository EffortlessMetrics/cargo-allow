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
        && let Some(path) = coupling_path(node, source, kind)
        && let Some(text) = node_text(source, node)
    {
        let start = node.start_position();
        facts.push(RustSourceCoupling {
            kind,
            path,
            text: text.trim().to_string(),
            start_line: start.row as u32 + 1,
            start_column: source_column(line_index, source, start.row, start.column),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_coupling_facts(child, source, line_index, facts);
    }
}

fn coupling_path(node: Node<'_>, source: &str, kind: RustSourceCouplingKind) -> Option<String> {
    let path_node = match kind {
        RustSourceCouplingKind::UseDeclaration => node
            .child_by_field_name("argument")
            .or_else(|| node.child_by_field_name("path"))?,
        RustSourceCouplingKind::InlineModule => node.child_by_field_name("name")?,
    };
    node_text(source, path_node)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}
