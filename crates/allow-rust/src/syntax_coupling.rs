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
    scan_rust_source_coupling_with_posture(
        source,
        !rust_source_declares_no_std(source)?,
        !rust_source_shadows_path_macros(source)?,
    )
}

pub fn scan_rust_source_coupling_with_manifest_env(
    source: &str,
    manifest_env_is_unshadowed: bool,
) -> CargoAllowResult<RustSourceCouplingScan> {
    scan_rust_source_coupling_with_posture(source, manifest_env_is_unshadowed, true)
}

pub fn scan_rust_source_coupling_with_posture(
    source: &str,
    manifest_env_is_unshadowed: bool,
    _path_macros_are_unshadowed: bool,
) -> CargoAllowResult<RustSourceCouplingScan> {
    let tree = parse_rust_syntax(source)?;
    let line_index = SourceLineIndex::new(source);
    let mut facts = Vec::new();
    collect_coupling_facts(
        tree.tree.root_node(),
        source,
        &line_index,
        manifest_env_is_unshadowed,
        &mut facts,
    );
    Ok(RustSourceCouplingScan {
        facts,
        has_parse_error: tree.has_error(),
    })
}

pub fn rust_source_shadows_path_macros(source: &str) -> CargoAllowResult<bool> {
    let tree = parse_rust_syntax(source)?;
    let root = tree.tree.root_node();
    Ok(root_has_macro_use_extern(root, source) || node_shadows_path_macros(root, source))
}

fn root_has_macro_use_extern(root: Node<'_>, source: &str) -> bool {
    let uncommented = strip_rust_comments(source).unwrap_or_default();
    if uncommented.contains("#[macro_use") && uncommented.contains("extern crate") {
        return true;
    }
    let mut cursor = root.walk();
    let children: Vec<_> = root.named_children(&mut cursor).collect();
    children.windows(2).any(|pair| {
        pair.first().is_some_and(|attribute| {
            attribute.kind().contains("attribute")
                && node_text(source, *attribute).is_some_and(|text| {
                    strip_rust_comments(text).is_some_and(|text| {
                        text.chars()
                            .filter(|ch| !ch.is_whitespace())
                            .collect::<String>()
                            == "#[macro_use]"
                    })
                })
        }) && pair
            .get(1)
            .is_some_and(|item| item.kind() == "extern_crate_declaration")
    })
}

fn node_shadows_path_macros(node: Node<'_>, source: &str) -> bool {
    if node.kind() == "macro_definition"
        && node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
            .is_some_and(matches_path_macro_name)
    {
        return true;
    }
    if node.kind() == "use_declaration"
        && node_text(source, node).is_some_and(|text| !text.trim_start().starts_with("use super::"))
        && use_binds_path_macro(node, source)
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| node_shadows_path_macros(child, source))
}

fn use_binds_path_macro(node: Node<'_>, source: &str) -> bool {
    let Some(text) = node_text(source, node) else {
        return false;
    };
    let Some(text) = strip_rust_comments(text) else {
        return true;
    };
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.contains('*')
        && compact.contains("::")
        && !compact.contains("usesuper::")
        && !compact.contains("usecrate::")
        && !compact.contains("useself::")
    {
        return true;
    }
    if compact.contains('{') {
        if compact.matches('{').count() > 1 {
            return true;
        }
        let Some(group) = compact
            .split_once('{')
            .and_then(|(_, rest)| rest.rsplit_once('}').map(|(group, _)| group))
        else {
            return true;
        };
        return group.split(',').any(|leaf| {
            let binding = leaf
                .rsplit_once("as")
                .map(|(_, alias)| alias)
                .unwrap_or_else(|| leaf.rsplit("::").next().unwrap_or(leaf));
            matches_path_macro_name(binding)
        });
    }
    if let Some((_, alias)) = compact.rsplit_once("as") {
        return matches_path_macro_name(alias.trim_end_matches(';'));
    }
    let binding = compact
        .trim_end_matches(';')
        .rsplit([':', ',', '{'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('}');
    matches_path_macro_name(binding)
}

fn matches_path_macro_name(name: &str) -> bool {
    matches!(
        name.strip_prefix("r#").unwrap_or(name),
        "include" | "include_str" | "include_bytes" | "concat"
    )
}

pub fn rust_source_declares_no_std(source: &str) -> CargoAllowResult<bool> {
    let tree = parse_rust_syntax(source)?;
    let root = tree.tree.root_node();
    let mut cursor = root.walk();
    let has_shadowing_attribute = root.named_children(&mut cursor).any(|attribute| {
        attribute.kind().contains("attribute")
            && node_text(source, attribute).is_some_and(|text| {
                let Some(text) = strip_rust_comments(text) else {
                    return false;
                };
                let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
                compact == "#![no_std]"
                    || compact == "#![no_implicit_prelude]"
                    || (compact.starts_with("#![cfg_attr(")
                        && compact.ends_with(")]")
                        && compact
                            .split([',', ')'])
                            .any(|token| matches!(token, "no_std" | "no_implicit_prelude")))
            })
    });
    if has_shadowing_attribute {
        return Ok(true);
    }
    let mut cursor = root.walk();
    Ok(root.named_children(&mut cursor).any(|item| {
        (item.kind() == "extern_crate_declaration"
            && node_text(source, item).is_some_and(|text| {
                strip_rust_comments(text).is_some_and(|text| {
                    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
                    compact
                        .strip_prefix("externcrate")
                        .and_then(|declaration| declaration.strip_suffix(';'))
                        .is_some_and(|declaration| declaration.ends_with("asstd"))
                })
            }))
            || (item.kind() == "mod_item"
                && item
                    .child_by_field_name("name")
                    .and_then(|name| node_text(source, name))
                    .is_some_and(|name| name == "std"))
    }))
}

/// Whether the item's own attribute list gates it on `cfg(test)`
/// (directly or through `cfg(all(..., test, ...))`). `cfg(not(test))`
/// and other cfg forms are not test gating.
fn item_is_test_cfg_gated(node: Node<'_>, source: &str) -> bool {
    let mut current = node.prev_sibling();
    while let Some(sibling) = current {
        if sibling.kind() != "attribute_item" {
            break;
        }
        if let Some(text) = node_text(source, sibling)
            && let Some(inner) = text
                .trim()
                .strip_prefix("#[cfg(")
                .and_then(|inner| inner.strip_suffix(")]"))
        {
            let compact: String = inner.chars().filter(|ch| !ch.is_whitespace()).collect();
            if cfg_predicate_posture(&compact) == CfgPredicatePosture::RequiresTest {
                return true;
            }
        }
        current = sibling.prev_sibling();
    }
    false
}

/// What a cfg predicate implies about the `test` configuration (#3646
/// hardening). The scanner gates an item only when the predicate
/// REQUIRES test — the item cannot compile into a production build.
/// Identifiers are parsed structurally, so a feature or value string
/// merely similar to `test` (for example `feature = "testing"`) never
/// gates an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CfgPredicatePosture {
    /// The predicate can only hold when cfg(test) holds: dev-scope.
    RequiresTest,
    /// The predicate cannot hold when cfg(test) holds: the item exists
    /// only in non-test builds and must contribute coupling facts.
    ExcludesTest,
    /// No test implication either way.
    Independent,
}

fn cfg_predicate_posture(predicate: &str) -> CfgPredicatePosture {
    let Some(head) = cfg_predicate_head(predicate) else {
        return CfgPredicatePosture::Independent;
    };
    let Some(inner) = head.group else {
        // Bare identifier or `key = "value"`: only the exact identifier
        // `test` requires test; values are never inspected.
        return if head.name == "test" {
            CfgPredicatePosture::RequiresTest
        } else {
            CfgPredicatePosture::Independent
        };
    };
    let arms = cfg_predicate_arms(inner);
    match head.name {
        "not" => match arms.first() {
            Some(only) => match cfg_predicate_posture(only) {
                CfgPredicatePosture::RequiresTest => CfgPredicatePosture::ExcludesTest,
                CfgPredicatePosture::ExcludesTest => CfgPredicatePosture::RequiresTest,
                CfgPredicatePosture::Independent => CfgPredicatePosture::Independent,
            },
            None => CfgPredicatePosture::Independent,
        },
        "all" => {
            if arms.is_empty() {
                return CfgPredicatePosture::Independent;
            }
            let postures = arms
                .iter()
                .map(|arm| cfg_predicate_posture(arm))
                .collect::<Vec<_>>();
            if postures.contains(&CfgPredicatePosture::ExcludesTest) {
                return CfgPredicatePosture::ExcludesTest;
            }
            if postures.contains(&CfgPredicatePosture::RequiresTest) {
                return CfgPredicatePosture::RequiresTest;
            }
            CfgPredicatePosture::Independent
        }
        "any" => {
            // A disjunction requires test only when EVERY arm requires
            // it. `any(test, feature = "x")` deliberately does not gate:
            // the item still compiles into production when the other
            // arm holds, so dropping its coupling facts would
            // under-enforce.
            if !arms.is_empty()
                && arms
                    .iter()
                    .all(|arm| cfg_predicate_posture(arm) == CfgPredicatePosture::RequiresTest)
            {
                CfgPredicatePosture::RequiresTest
            } else {
                CfgPredicatePosture::Independent
            }
        }
        _ => CfgPredicatePosture::Independent,
    }
}

struct CfgPredicateHead<'a> {
    name: &'a str,
    group: Option<&'a str>,
}

/// Parse a cfg predicate into its head: `name(inner)`, a bare
/// identifier, or `key = "value"` (group is None for the latter two).
fn cfg_predicate_head(predicate: &str) -> Option<CfgPredicateHead<'_>> {
    let Some(open) = predicate.find('(') else {
        return Some(CfgPredicateHead {
            name: predicate,
            group: None,
        });
    };
    if !predicate.ends_with(')') {
        return None;
    }
    let name = predicate.get(..open)?;
    if name.is_empty() || !name.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
        return None;
    }
    let close = predicate.len().checked_sub(1)?;
    let inner = predicate.get(open + 1..close)?;
    Some(CfgPredicateHead {
        name,
        group: Some(inner),
    })
}

/// Split a group body into top-level comma-separated arms, respecting
/// nesting. Malformed nesting yields no arms.
fn cfg_predicate_arms(inner: &str) -> Vec<&str> {
    let mut arms = Vec::new();
    let mut depth: usize = 0;
    let mut start: usize = 0;
    for (index, ch) in inner.char_indices() {
        match ch {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(arm) = inner.get(start..index) {
                    arms.push(arm);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Vec::new();
    }
    if let Some(arm) = inner.get(start..) {
        arms.push(arm);
    }
    arms.retain(|arm| !arm.is_empty());
    arms
}

fn strip_rust_comments(text: &str) -> Option<String> {
    let mut output = String::new();
    let mut index = 0;
    while index < text.len() {
        if let Some(end) = rust_comment_end(text, index)? {
            output.push(' ');
            index = end;
        } else {
            let ch = text.get(index..)?.chars().next()?;
            output.push(ch);
            index += ch.len_utf8();
        }
    }
    Some(output)
}

fn collect_coupling_facts(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    manifest_env_is_unshadowed: bool,
    facts: &mut Vec<RustSourceCoupling>,
) {
    // Test-gated items are dev-scope (#3646): a `#[cfg(test)]` use or a
    // `#[cfg(test)]` module (typically `mod tests`) does not contribute
    // production coupling facts, and the module's whole subtree is
    // pruned. This mirrors the *_tests.rs file exemption at item level
    // and keeps the guard honest about cfg-gating instead of relying on
    // fully-qualified paths being invisible to the use-declaration scan.
    if matches!(node.kind(), "use_declaration" | "mod_item") && item_is_test_cfg_gated(node, source)
    {
        return;
    }
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
            RustSourceCouplingKind::PathRead
                if path_read_macro_is_trusted(node, source, manifest_env_is_unshadowed) =>
            {
                let mut cursor = node.walk();
                let token_tree = node
                    .named_children(&mut cursor)
                    .find(|child| child.kind() == "token_tree")
                    .and_then(|token_tree| {
                        path_read_argument(token_tree, source, manifest_env_is_unshadowed)
                    });
                token_tree.unwrap_or((vec![String::new()], RustSourceCouplingPathBase::SourceFile))
            }
            RustSourceCouplingKind::PathRead => {
                (vec![String::new()], RustSourceCouplingPathBase::SourceFile)
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
        collect_coupling_facts(child, source, line_index, manifest_env_is_unshadowed, facts);
    }
}

fn path_read_macro_kind(node: Node<'_>, source: &str) -> Option<RustSourceCouplingKind> {
    let macro_path = node
        .child_by_field_name("macro")
        .and_then(|macro_node| node_text(source, macro_node))?;
    let macro_path = normalized_macro_path(macro_path)?;
    let macro_name = macro_path.rsplit("::").next()?;
    let macro_name = macro_name.strip_prefix("r#").unwrap_or(macro_name);
    matches!(macro_name, "include" | "include_str" | "include_bytes")
        .then_some(RustSourceCouplingKind::PathRead)
}

fn path_read_macro_is_trusted(node: Node<'_>, source: &str, std_is_unshadowed: bool) -> bool {
    std_is_unshadowed
        && node
            .child_by_field_name("macro")
            .and_then(|macro_node| node_text(source, macro_node))
            .is_some_and(is_standard_path_read_macro)
}

fn is_standard_path_read_macro(path: &str) -> bool {
    let Some(path) = normalized_macro_path(path) else {
        return false;
    };
    matches!(
        path.as_str(),
        "::std::include"
            | "::std::include_str"
            | "::std::include_bytes"
            | "::std::r#include"
            | "::std::r#include_str"
            | "::std::r#include_bytes"
    )
}

fn normalized_macro_path(path: &str) -> Option<String> {
    strip_rust_comments(path).map(|path| path.chars().filter(|ch| !ch.is_whitespace()).collect())
}

fn standard_concat_invocation_text(text: &str) -> Option<String> {
    let mut index = 0;
    let bang = loop {
        if index >= text.len() {
            return None;
        }
        if let Some(end) = rust_comment_end(text, index).flatten() {
            index = end;
            continue;
        }
        if text.as_bytes().get(index) == Some(&b'!') {
            break index;
        }
        index += 1;
    };
    let path = normalized_macro_path(text.get(..bang).unwrap_or_default())?;
    matches!(path.as_str(), "::std::concat" | "::std::r#concat")
        .then(|| format!("concat{}", text.get(bang..).unwrap_or_default()))
}

fn path_read_argument(
    node: Node<'_>,
    source: &str,
    manifest_env_is_unshadowed: bool,
) -> Option<(Vec<String>, RustSourceCouplingPathBase)> {
    let full_argument = node_text(source, node)?;
    let trimmed_argument = strip_macro_delimiters(full_argument)?.trim();
    if let Some(concat_text) = standard_concat_invocation_text(trimmed_argument) {
        let path =
            manifest_env_is_unshadowed.then(|| evaluate_manifest_concat_text(&concat_text))??;
        return Some((vec![path], RustSourceCouplingPathBase::ManifestDirectory));
    }
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
    let concat_text = if child.kind() == "identifier"
        && node_text(source, child)
            .is_some_and(|name| name.strip_prefix("r#").unwrap_or(name) == "concat")
    {
        let arguments = children.next()?;
        source.get(child.start_byte()..arguments.end_byte())?
    } else {
        if child.kind() != "macro_invocation" {
            let concat_text = find_token_tree_macro_text(child, source, "concat")?;
            let path = manifest_env_is_unshadowed
                .then(|| evaluate_manifest_concat_text(concat_text))??;
            return Some((vec![path], RustSourceCouplingPathBase::ManifestDirectory));
        }
        let macro_path = child
            .child_by_field_name("macro")
            .and_then(|macro_node| node_text(source, macro_node))?;
        let macro_name = macro_path.rsplit("::").next()?;
        if (macro_path.contains("::")
            && (!matches!(macro_path, "std::concat" | "std::r#concat")
                || !node_text(source, child)?
                    .trim_start()
                    .starts_with("::std::")))
            || macro_name.strip_prefix("r#").unwrap_or(macro_name) != "concat"
        {
            return None;
        }
        node_text(source, child)?
    };
    let path = manifest_env_is_unshadowed.then(|| evaluate_manifest_concat_text(concat_text))??;
    Some((vec![path], RustSourceCouplingPathBase::ManifestDirectory))
}

fn find_token_tree_macro_text<'a>(
    node: Node<'a>,
    source: &'a str,
    expected: &str,
) -> Option<&'a str> {
    let mut cursor = node.walk();
    let mut children = node
        .named_children(&mut cursor)
        .filter(|child| !matches!(child.kind(), "line_comment" | "block_comment"));
    if let Some(name) = children.next() {
        if name.kind() == "identifier" {
            if node_text(source, name)
                .is_some_and(|name| name.strip_prefix("r#").unwrap_or(name) == expected)
            {
                let arguments = children.next()?;
                return source.get(name.start_byte()..arguments.end_byte());
            }
            return None;
        }
        if children.next().is_none() {
            return find_token_tree_macro_text(name, source, expected);
        }
    }
    None
}

fn evaluate_manifest_concat_text(text: &str) -> Option<String> {
    // Build-output bases such as env!("OUT_DIR") remain unresolved: resolving
    // them would require build metadata, outside cargo-allow's source-tree scan.
    let text = text.trim_start().strip_prefix("::std::").unwrap_or(text);
    let concat_name = if text.trim_start().starts_with("r#concat") {
        "r#concat"
    } else {
        "concat"
    };
    let start = macro_bang_end(text, concat_name)?;
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
        if let Some(env_start) = manifest_env_bang_end(arg) {
            let value = strip_macro_delimiters(arg.get(env_start..)?.trim_start())?;
            let value = strip_surrounding_comments(value)?;
            if saw_manifest_dir
                || !path.is_empty()
                || decode_path_literal(value)?.as_str() != "CARGO_MANIFEST_DIR"
            {
                return None;
            }
            saw_manifest_dir = true;
        } else {
            let value = decode_path_literal(arg)?;
            #[cfg(windows)]
            let begins_with_separator = value.starts_with(['/', '\\']);
            #[cfg(not(windows))]
            let begins_with_separator = value.starts_with('/');
            if !saw_manifest_dir || (path.is_empty() && !begins_with_separator) {
                return None;
            }
            path.push_str(&value);
        }
    }
    saw_manifest_dir.then_some(path)
}

fn manifest_env_bang_end(text: &str) -> Option<usize> {
    // Bare `env!` can be shadowed by a source macro. Only the absolute standard
    // macro path is strong enough for a source-only scanner to reconstruct.
    let text = strip_leading_comments(text)?;
    let mut index = 0;
    for token in ["::", "std", "::"] {
        index = skip_rust_trivia(text, index)?;
        if !text.get(index..)?.starts_with(token) {
            return None;
        }
        index += token.len();
    }
    index = skip_rust_trivia(text, index)?;
    let env_len = if text.get(index..)?.starts_with("r#env") {
        "r#env".len()
    } else if text.get(index..)?.starts_with("env") {
        "env".len()
    } else {
        return None;
    };
    index += env_len;
    index = skip_rust_trivia(text, index)?;
    (text.as_bytes().get(index) == Some(&b'!')).then_some(index + 1)
}

fn skip_rust_trivia(text: &str, mut index: usize) -> Option<usize> {
    loop {
        let suffix = text.get(index..)?;
        index += suffix.len() - suffix.trim_start().len();
        if let Some(end) = rust_comment_end(text, index)? {
            index = end;
        } else {
            return Some(index);
        }
    }
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
