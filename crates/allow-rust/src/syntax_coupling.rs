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
    let macro_name = macro_name.strip_prefix("r#").unwrap_or(macro_name);
    matches!(macro_name, "include" | "include_str" | "include_bytes")
        .then_some(RustSourceCouplingKind::PathRead)
}

fn path_read_argument(
    node: Node<'_>,
    source: &str,
) -> Option<(Vec<String>, RustSourceCouplingPathBase)> {
    let mut cursor = node.walk();
    let mut children = node
        .named_children(&mut cursor)
        .filter(|child| !matches!(child.kind(), "line_comment" | "block_comment"));
    let child = children.next()?;
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
    let concat_text = if child.kind() == "identifier" && node_text(source, child) == Some("concat")
    {
        let arguments = children.next()?;
        source.get(child.start_byte()..arguments.end_byte())?
    } else {
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
        node_text(source, child)?
    };
    let path = evaluate_manifest_concat_text(concat_text)?;
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
    // Build-output bases such as env!("OUT_DIR") remain unresolved: resolving
    // them would require build metadata, outside cargo-allow's source-tree scan.
    let start = macro_bang_end(text, "concat")?;
    let open = text
        .get(start..)?
        .find(['(', '[', '{'])?
        .checked_add(start)?;
    let close = matching_delimiter(text, open)?;
    let args = text.get(open + 1..close)?;
    let mut saw_manifest_dir = false;
    let mut path = String::new();
    for arg in split_concat_args(args)? {
        let arg = strip_surrounding_comments(arg)?;
        if arg.trim_start().starts_with("env") {
            let value =
                strip_macro_delimiters(arg.get(macro_bang_end(arg, "env")?..)?.trim_start())?
                    .trim();
            if saw_manifest_dir
                || !path.is_empty()
                || decode_path_literal(value)?.as_str() != "CARGO_MANIFEST_DIR"
            {
                return None;
            }
            saw_manifest_dir = true;
        } else {
            let value = decode_path_literal(arg)?;
            path.push_str(&value);
        }
    }
    saw_manifest_dir.then_some(path)
}

fn split_concat_args(args: &str) -> Option<Vec<&str>> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut delimiters = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if let Some(end) = rust_string_end(args, index)? {
            index = end;
            continue;
        }
        if let Some(end) = rust_comment_end(args, index)? {
            index = end;
            continue;
        }
        match *args.as_bytes().get(index)? {
            b'(' => delimiters.push(b')'),
            b'[' => delimiters.push(b']'),
            b'{' => delimiters.push(b'}'),
            close @ (b')' | b']' | b'}') if delimiters.pop() != Some(close) => return None,
            b',' if delimiters.is_empty() => {
                result.push(args.get(start..index)?);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    result.push(args.get(start..)?);
    Some(result)
}

fn macro_bang_end(text: &str, name: &str) -> Option<usize> {
    let mut index = text.find(name)? + name.len();
    loop {
        let suffix = text.get(index..)?;
        index += suffix.len() - suffix.trim_start().len();
        if let Some(end) = rust_comment_end(text, index)? {
            index = end;
            continue;
        }
        return (text.as_bytes().get(index) == Some(&b'!')).then_some(index + 1);
    }
}

fn strip_leading_comments(mut text: &str) -> Option<&str> {
    loop {
        text = text.trim_start();
        if let Some(end) = rust_comment_end(text, 0)? {
            text = text.get(end..)?;
        } else {
            return Some(text);
        }
    }
}

fn strip_surrounding_comments(text: &str) -> Option<&str> {
    let text = strip_leading_comments(text)?.trim_end();
    let mut index = 0;
    while index < text.len() {
        if let Some(end) = rust_string_end(text, index)? {
            index = end;
            continue;
        }
        if let Some(end) = rust_comment_end(text, index)? {
            if text.get(end..)?.trim().is_empty() {
                return strip_surrounding_comments(text.get(..index)?);
            }
            index = end;
            continue;
        }
        index += text.get(index..)?.chars().next()?.len_utf8();
    }
    Some(text.trim())
}

fn matching_delimiter(text: &str, open: usize) -> Option<usize> {
    let mut delimiters = Vec::new();
    let mut index = open;
    while index < text.len() {
        if let Some(end) = rust_string_end(text, index)? {
            index = end;
            continue;
        }
        if let Some(end) = rust_comment_end(text, index)? {
            index = end;
            continue;
        }
        match *text.as_bytes().get(index)? {
            b'(' => delimiters.push(b')'),
            b'[' => delimiters.push(b']'),
            b'{' => delimiters.push(b'}'),
            close @ (b')' | b']' | b'}') => {
                if delimiters.pop() != Some(close) {
                    return None;
                }
                if delimiters.is_empty() {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn rust_comment_end(text: &str, start: usize) -> Option<Option<usize>> {
    let bytes = text.as_bytes();
    if bytes.get(start..start + 2) == Some(b"//") {
        return Some(Some(
            text.get(start + 2..)?
                .find('\n')
                .map_or(text.len(), |offset| start + 3 + offset),
        ));
    }
    if bytes.get(start..start + 2) != Some(b"/*") {
        return Some(None);
    }
    let mut depth = 1usize;
    let mut index = start + 2;
    while index < bytes.len() {
        match bytes.get(index..index + 2) {
            Some(b"/*") => {
                depth += 1;
                index += 2;
            }
            Some(b"*/") => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    return Some(Some(index));
                }
            }
            _ => index += 1,
        }
    }
    None
}

fn strip_macro_delimiters(text: &str) -> Option<&str> {
    let open = *text.as_bytes().first()?;
    let close = match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        _ => return None,
    };
    (text.as_bytes().last() == Some(&close)).then(|| text.get(1..text.len() - 1))?
}

fn rust_string_end(text: &str, start: usize) -> Option<Option<usize>> {
    let bytes = text.as_bytes();
    if bytes.get(start) == Some(&b'"') {
        let mut index = start + 1;
        while index < bytes.len() {
            match *bytes.get(index)? {
                b'\\' => index += 2,
                b'"' => return Some(Some(index + 1)),
                _ => index += 1,
            }
        }
        return None;
    }
    if bytes.get(start) != Some(&b'r') {
        return Some(None);
    }
    let mut quote = start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return Some(None);
    }
    let hashes = quote - start - 1;
    let mut index = quote + 1;
    while index < bytes.len() {
        if bytes.get(index) == Some(&b'"')
            && bytes.get(index + 1..index + 1 + hashes) == bytes.get(start + 1..quote)
        {
            return Some(Some(index + 1 + hashes));
        }
        index += 1;
    }
    None
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
