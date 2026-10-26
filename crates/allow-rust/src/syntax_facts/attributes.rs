use allow_core::normalize_snippet;
use tree_sitter::Node;

use crate::syntax_kinds::{LintAttribute, LintAttributeKind, RustSyntaxFacts, UnsafeAttribute};
use crate::syntax_tree::node_text;
use crate::text::{
    SourceLineIndex, detect_attr, rust_comment_end, skip_rust_trivia, skip_rust_whitespace,
    source_column,
};

pub(super) fn record_node_attributes(
    node: Node<'_>,
    source: &str,
    line_index: &SourceLineIndex,
    facts: &mut RustSyntaxFacts,
) {
    if !matches!(node.kind(), "attribute_item" | "inner_attribute_item") {
        return;
    }

    let Some(text) = node_text(source, node) else {
        return;
    };

    // #4645 review: trivia (e.g. a comment followed by a newline) may displace
    // the written attribute name onto a later line than the attribute item's
    // opening tokens, so every finding takes its line and column from the
    // name's own absolute byte position instead of the node's start line.
    let node_start_byte = node.start_byte();
    for (kind, offset) in lint_attribute_kinds(text) {
        let attr_text = if offset == 0 {
            text.to_string()
        } else {
            text.get(offset..)
                .map(ToString::to_string)
                .unwrap_or_else(|| text.to_string())
        };
        let (row, byte_column) = line_index.source_position(node_start_byte + offset);
        facts
            .lint_attributes
            .entry(row as u32 + 1)
            .or_default()
            .push(LintAttribute {
                kind,
                text: attr_text,
                column: source_column(line_index, source, row, byte_column),
            });
    }
    let unsafe_attribute_offsets = unsafe_attribute_offsets(text);
    if !unsafe_attribute_offsets.is_empty() {
        for offset in unsafe_attribute_offsets {
            let (row, byte_column) = line_index.source_position(node_start_byte + offset);
            facts
                .unsafe_attributes
                .entry(row as u32 + 1)
                .or_default()
                .push(UnsafeAttribute {
                    column: source_column(line_index, source, row, byte_column),
                    symbol: unsafe_attribute_symbol(text, offset),
                });
        }
    }
}

fn lint_attribute_kinds(text: &str) -> Vec<(LintAttributeKind, usize)> {
    let trimmed = text.trim_start();
    if detect_attr(trimmed, "allow").is_some() {
        vec![(LintAttributeKind::Allow, text.len() - trimmed.len())]
    } else if detect_attr(trimmed, "expect").is_some() {
        vec![(LintAttributeKind::Expect, text.len() - trimmed.len())]
    } else if let Some(offset) = attribute_name_offset(text, "allow") {
        vec![(LintAttributeKind::Allow, offset)]
    } else if let Some(offset) = attribute_name_offset(text, "expect") {
        vec![(LintAttributeKind::Expect, offset)]
    } else if let Some(offset) = attribute_name_offset(text, "deny") {
        vec![(LintAttributeKind::Deny, offset)]
    } else if let Some(offset) = attribute_name_offset(text, "forbid") {
        vec![(LintAttributeKind::Forbid, offset)]
    } else if let Some(offset) = attribute_name_offset(text, "warn") {
        vec![(LintAttributeKind::Warn, offset)]
    } else if attribute_name_offset(text, "cfg_attr").is_some() {
        cfg_attr_lint_kinds(text)
    } else {
        Vec::new()
    }
}

fn unsafe_attribute_offsets(text: &str) -> Vec<usize> {
    if let Some(offset) = attribute_name_offset(text, "unsafe") {
        return vec![offset];
    }
    if attribute_name_offset(text, "cfg_attr").is_none() {
        return Vec::new();
    }
    find_attribute_invocations_outside_rust_strings(text, "unsafe")
}

fn unsafe_attribute_symbol(text: &str, offset: usize) -> Option<String> {
    let mut cursor = offset + "unsafe".len();
    cursor = skip_rust_whitespace(text, cursor);
    if !text.get(cursor..).is_some_and(|rest| rest.starts_with('(')) {
        return None;
    }
    cursor += '('.len_utf8();
    cursor = skip_rust_whitespace(text, cursor);
    let start = cursor;

    while let Some(ch) = text.get(cursor..).and_then(|rest| rest.chars().next()) {
        if !(ch == '_' || ch == ':' || ch.is_alphanumeric()) {
            break;
        }
        cursor += ch.len_utf8();
    }

    text.get(start..cursor)
        .map(normalize_snippet)
        .filter(|symbol| !symbol.is_empty())
}

fn cfg_attr_lint_kinds(text: &str) -> Vec<(LintAttributeKind, usize)> {
    // #2660: Scope scanning to cfg_attr arguments (after the first top-level
    // comma) to avoid false positives from cfg predicate content that happens
    // to contain attribute-shaped tokens like `allow(...)`.
    let scan_text = cfg_attr_arguments_after_predicate(text);
    let base_offset = text.len() - scan_text.len();
    let mut attributes = find_attribute_invocations_outside_rust_strings(scan_text, "allow")
        .into_iter()
        .map(|offset| (LintAttributeKind::Allow, offset + base_offset))
        .chain(
            find_attribute_invocations_outside_rust_strings(scan_text, "expect")
                .into_iter()
                .map(|offset| (LintAttributeKind::Expect, offset + base_offset)),
        )
        .chain(
            find_attribute_invocations_outside_rust_strings(scan_text, "deny")
                .into_iter()
                .map(|offset| (LintAttributeKind::Deny, offset + base_offset)),
        )
        .chain(
            find_attribute_invocations_outside_rust_strings(scan_text, "forbid")
                .into_iter()
                .map(|offset| (LintAttributeKind::Forbid, offset + base_offset)),
        )
        .chain(
            find_attribute_invocations_outside_rust_strings(scan_text, "warn")
                .into_iter()
                .map(|offset| (LintAttributeKind::Warn, offset + base_offset)),
        )
        .collect::<Vec<_>>();
    attributes.sort_by_key(|(_, offset)| *offset);
    attributes
}

/// Extract the cfg_attr arguments portion: everything after the first top-level
/// comma in the attribute text. The cfg predicate is before the comma; the
/// actual attributes (allow, deny, etc.) are after it.
fn cfg_attr_arguments_after_predicate(text: &str) -> &str {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut string_char = '\0';
    let mut cursor = 0;
    while cursor < text.len() {
        let Some(ch) = text.get(cursor..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        if in_string {
            if ch == string_char && text.get(cursor.wrapping_sub(1)..cursor) != Some("\\") {
                in_string = false;
            }
            cursor += ch.len_utf8();
            continue;
        }
        match ch {
            '"' | '\'' => {
                in_string = true;
                string_char = ch;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                return text.get(cursor + 1..).unwrap_or("");
            }
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    text
}

fn attribute_name_offset(text: &str, name: &str) -> Option<usize> {
    let mut cursor = skip_rust_trivia(text, 0);
    if !text.get(cursor..).is_some_and(|rest| rest.starts_with('#')) {
        return None;
    }
    cursor += '#'.len_utf8();
    cursor = skip_rust_trivia(text, cursor);
    if text.get(cursor..).is_some_and(|rest| rest.starts_with('!')) {
        cursor += '!'.len_utf8();
        cursor = skip_rust_trivia(text, cursor);
    }
    if !text.get(cursor..).is_some_and(|rest| rest.starts_with('[')) {
        return None;
    }
    cursor += '['.len_utf8();
    cursor = skip_rust_trivia(text, cursor);
    // #4645: attribute names may be spelled as raw identifiers
    // (`#[r#allow(...)]`). Keep the offset at the start of the written name
    // token so the reported column and attribute text include the `r#`.
    let name_start = cursor;
    if text.get(cursor..).is_some_and(|rest| {
        rest.starts_with("r#") && rest.get(2..).is_some_and(|after| after.starts_with(name))
    }) {
        cursor += "r#".len();
    }
    if text
        .get(cursor..)
        .is_some_and(|rest| rest.starts_with(name))
        && invocation_name_followed_by_parens(text, cursor + name.len())
    {
        Some(name_start)
    } else {
        None
    }
}

fn find_attribute_invocations_outside_rust_strings(text: &str, name: &str) -> Vec<usize> {
    let mut matches = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        // #4645 review: comment contents are not source syntax — a lint-name
        // token inside a comment must not produce an attribute finding. Skip
        // complete line and (nested) block comments before name matching;
        // an unterminated block comment consumes the remaining text.
        match rust_comment_end(text, cursor) {
            Some(Some(end)) => {
                cursor = end;
                continue;
            }
            Some(None) => {}
            None => break,
        }
        if text
            .get(cursor..)
            .is_some_and(|rest| rest.starts_with(name))
            && token_starts_at_attribute_boundary(text, cursor)
            && invocation_name_followed_by_parens(text, cursor + name.len())
        {
            matches.push(cursor);
            cursor += name.len();
            continue;
        }
        if let Some(end) = raw_string_end(text, cursor) {
            cursor = end;
            continue;
        }
        let Some(ch) = text.get(cursor..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        if matches!(ch, '"' | '\'') {
            cursor = quoted_literal_end(text, cursor, ch);
            continue;
        }
        cursor += ch.len_utf8();
    }
    matches
}

fn invocation_name_followed_by_parens(text: &str, cursor: usize) -> bool {
    // #4645: comments as well as whitespace may sit between an attribute or
    // nested attribute-invocation name and its `(`.
    text.get(skip_rust_trivia(text, cursor)..)
        .is_some_and(|rest| rest.starts_with('('))
}

fn token_starts_at_attribute_boundary(text: &str, cursor: usize) -> bool {
    text.get(..cursor)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|ch| !(ch == '_' || ch == ':' || ch.is_alphanumeric()))
}

fn raw_string_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start).copied() != Some(b'r') {
        return None;
    }
    let mut cursor = start + 1;
    while bytes.get(cursor).copied() == Some(b'#') {
        cursor += 1;
    }
    if bytes.get(cursor).copied() != Some(b'"') {
        return None;
    }
    let hashes = cursor.saturating_sub(start + 1);
    let close = format!("\"{}", "#".repeat(hashes));
    let content_start = cursor + 1;
    text.get(content_start..)
        .and_then(|rest| rest.find(&close))
        .map(|offset| content_start + offset + close.len())
        .or(Some(text.len()))
}

fn quoted_literal_end(text: &str, start: usize, quote: char) -> usize {
    let mut cursor = start + quote.len_utf8();
    let mut escaped = false;
    while cursor < text.len() {
        let Some(ch) = text.get(cursor..).and_then(|rest| rest.chars().next()) else {
            return text.len();
        };
        cursor += ch.len_utf8();
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return cursor;
        }
    }
    text.len()
}
