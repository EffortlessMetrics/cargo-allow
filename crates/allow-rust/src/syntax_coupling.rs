use crate::syntax_tree::{node_text, parse_rust_syntax};
use crate::text::{SourceLineIndex, source_column};
use allow_core::CargoAllowResult;
use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustSourceCouplingKind {
    UseDeclaration,
    InlineModule,
    PathRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustSourceCouplingPathBase {
    SourceFile,
    ManifestDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceCoupling {
    pub kind: RustSourceCouplingKind,
    pub path_base: RustSourceCouplingPathBase,
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
        "macro_invocation" => path_read_macro_kind(node, source),
        _ => None,
    };
    if let Some(kind) = kind
        && (kind != RustSourceCouplingKind::InlineModule
            || node.child_by_field_name("body").is_some())
        && let Some(text) = node_text(source, node)
    {
        let (paths, path_base) = match kind {
            RustSourceCouplingKind::UseDeclaration => node
                .child_by_field_name("argument")
                .map(|argument| use_clause_paths(argument, source))
                .map(|paths| (paths, RustSourceCouplingPathBase::SourceFile))
                .unwrap_or((Vec::new(), RustSourceCouplingPathBase::SourceFile)),
            RustSourceCouplingKind::InlineModule => node
                .child_by_field_name("name")
                .and_then(|name| node_text(source, name))
                .map(|path| vec![path.trim().to_string()])
                .map(|paths| (paths, RustSourceCouplingPathBase::SourceFile))
                .unwrap_or((Vec::new(), RustSourceCouplingPathBase::SourceFile)),
            RustSourceCouplingKind::PathRead => {
                let mut cursor = node.walk();
                let token_tree = node
                    .named_children(&mut cursor)
                    .find(|child| child.kind() == "token_tree")
                    .and_then(|token_tree| path_read_argument(token_tree, source));
                token_tree.unwrap_or((vec![String::new()], RustSourceCouplingPathBase::SourceFile))
            }
        };
        let start = node.start_position();
        for path in paths {
            if path.is_empty() && kind != RustSourceCouplingKind::PathRead {
                continue;
            }
            facts.push(RustSourceCoupling {
                kind,
                path_base,
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

fn path_read_macro_kind(node: Node<'_>, source: &str) -> Option<RustSourceCouplingKind> {
    let macro_name = node
        .child_by_field_name("macro")
        .and_then(|macro_node| node_text(source, macro_node))?
        .rsplit("::")
        .next()?;
    matches!(macro_name, "include" | "include_str" | "include_bytes")
        .then_some(RustSourceCouplingKind::PathRead)
}

fn path_read_argument(
    node: Node<'_>,
    source: &str,
) -> Option<(Vec<String>, RustSourceCouplingPathBase)> {
    if node_text(source, node).is_some_and(|text| text.contains("CARGO_MANIFEST_DIR")) {
        let path = evaluate_manifest_concat_text(node_text(source, node)?)?;
        return Some((vec![path], RustSourceCouplingPathBase::ManifestDirectory));
    }
    let mut cursor = node.walk();
    let child = node.named_children(&mut cursor).next()?;
    if matches!(child.kind(), "string_literal" | "raw_string_literal") {
        return Some((
            vec![
                node_text(source, child)
                    .and_then(decode_path_literal)
                    .unwrap_or_default(),
            ],
            RustSourceCouplingPathBase::SourceFile,
        ));
    }
    let child = if child.kind() == "macro_invocation" {
        child
    } else {
        find_macro_invocation(child, source, "concat")?
    };
    let macro_name = child
        .child_by_field_name("macro")
        .and_then(|macro_node| node_text(source, macro_node))?
        .rsplit("::")
        .next()?;
    if macro_name != "concat" {
        return None;
    }
    if !node_text(source, child).is_some_and(|text| text.contains("CARGO_MANIFEST_DIR")) {
        return None;
    }
    let path = evaluate_manifest_concat_text(node_text(source, child)?)?;
    Some((vec![path], RustSourceCouplingPathBase::ManifestDirectory))
}

fn find_macro_invocation<'a>(node: Node<'a>, source: &str, name: &str) -> Option<Node<'a>> {
    if node.kind() == "macro_invocation"
        && node
            .child_by_field_name("macro")
            .and_then(|macro_node| node_text(source, macro_node))
            .is_some_and(|macro_name| macro_name.rsplit("::").next() == Some(name))
    {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| find_macro_invocation(child, source, name))
}

fn evaluate_manifest_concat_text(text: &str) -> Option<String> {
    let start = text.find("concat!")? + "concat!".len();
    let open = text.get(start..)?.find('(')? + start;
    let mut depth = 0;
    let mut close = None;
    for (index, ch) in text.get(open..)?.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + index);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let args = text.get(open + 1..close)?;
    let mut saw_manifest_dir = false;
    let mut path = String::new();
    for arg in split_concat_args(args)? {
        let arg = arg.trim();
        if let Some(value) = arg.strip_prefix("env!") {
            let value = value.trim().strip_prefix('(')?.strip_suffix(')')?.trim();
            if decode_path_literal(value)?.as_str() != "CARGO_MANIFEST_DIR" {
                return None;
            }
            saw_manifest_dir = true;
        } else if let Some(value) = decode_path_literal(arg) {
            path.push_str(&value);
        } else {
            return None;
        }
    }
    saw_manifest_dir.then_some(path)
}

fn split_concat_args(args: &str) -> Option<Vec<&str>> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    for (index, ch) in args.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                result.push(args.get(start..index)?);
                start = index + 1;
            }
            _ => {}
        }
    }
    result.push(args.get(start..)?);
    Some(result)
}

fn decode_path_literal(text: &str) -> Option<String> {
    if text.starts_with('"') {
        return text
            .strip_prefix('"')
            .and_then(|text| text.strip_suffix('"'))
            .filter(|text| !text.contains('\\'))
            .map(ToString::to_string);
    }
    if !text.starts_with('r') {
        return None;
    }
    let bytes = text.as_bytes();
    let mut hashes = 0usize;
    while bytes.get(1 + hashes) == Some(&b'#') {
        hashes += 1;
    }
    if bytes.get(1 + hashes) != Some(&b'"') {
        return None;
    }
    let prefix_len = 2 + hashes;
    let suffix = format!("\"{}", "#".repeat(hashes));
    text.get(prefix_len..text.len().saturating_sub(suffix.len()))
        .filter(|_| text.ends_with(&suffix))
        .map(ToString::to_string)
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
