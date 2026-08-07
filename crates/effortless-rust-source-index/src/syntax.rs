//! Minimal tree-sitter helpers for structural subject discovery (#2587-C).

use allow_core::{CargoAllowError, CargoAllowResult};
use tree_sitter::{Node, Parser, Tree};

pub(crate) struct RustSyntaxTree {
    tree: Tree,
}

impl RustSyntaxTree {
    pub(crate) fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    pub(crate) fn has_error(&self) -> bool {
        self.tree.root_node().has_error()
    }
}

pub(crate) fn parse_rust_syntax(source: &str) -> CargoAllowResult<RustSyntaxTree> {
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

pub(crate) fn source_column(source: &str, row: usize, byte_column: usize) -> u32 {
    source
        .lines()
        .nth(row)
        .map(|line| byte_column_to_char_column(line, byte_column))
        .unwrap_or(1)
}

fn byte_column_to_char_column(line: &str, byte_column: usize) -> u32 {
    line.char_indices()
        .take_while(|(idx, _)| *idx < byte_column)
        .count() as u32
        + 1
}
