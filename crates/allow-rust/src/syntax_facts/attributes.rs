use tree_sitter::Node;

use crate::syntax_kinds::{LintAttribute, LintAttributeKind, RustSyntaxFacts};
use crate::syntax_tree::node_text;
use crate::text::{detect_attr, source_column};

pub(super) fn record_node_attributes(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if !matches!(node.kind(), "attribute_item" | "inner_attribute_item") {
        return;
    }

    let Some(text) = node_text(source, node) else {
        return;
    };

    let start = node.start_position();
    let line = start.row as u32 + 1;
    if let Some((kind, offset)) = lint_attribute_kind(text) {
        let attr_text = if offset == 0 {
            text.to_string()
        } else {
            text.get(offset..)
                .map(ToString::to_string)
                .unwrap_or_else(|| text.to_string())
        };
        facts
            .lint_attributes
            .entry(line)
            .or_default()
            .push(LintAttribute {
                kind,
                text: attr_text,
                column: source_column(source, start.row, start.column + offset),
            });
    }
    if unsafe_attribute_text(text) {
        facts.unsafe_attribute_lines.insert(line);
    }
}

fn lint_attribute_kind(text: &str) -> Option<(LintAttributeKind, usize)> {
    let trimmed = text.trim_start();
    if detect_attr(trimmed, "allow").is_some() {
        Some((LintAttributeKind::Allow, text.len() - trimmed.len()))
    } else if detect_attr(trimmed, "expect").is_some() {
        Some((LintAttributeKind::Expect, text.len() - trimmed.len()))
    } else if trimmed.starts_with("#[cfg_attr(") || trimmed.starts_with("#![cfg_attr(") {
        cfg_attr_lint_kind(text)
    } else {
        None
    }
}

fn unsafe_attribute_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.starts_with("#[unsafe(") || trimmed.starts_with("#![unsafe(") {
        return true;
    }
    if !(trimmed.starts_with("#[cfg_attr(") || trimmed.starts_with("#![cfg_attr(")) {
        return false;
    }
    find_token_outside_rust_strings(trimmed, "unsafe(").is_some()
}

fn cfg_attr_lint_kind(text: &str) -> Option<(LintAttributeKind, usize)> {
    let allow = find_token_outside_rust_strings(text, "allow(");
    let expect = find_token_outside_rust_strings(text, "expect(");
    match (allow, expect) {
        (Some(allow), Some(expect)) if allow <= expect => Some((LintAttributeKind::Allow, allow)),
        (Some(_), Some(expect)) => Some((LintAttributeKind::Expect, expect)),
        (Some(allow), None) => Some((LintAttributeKind::Allow, allow)),
        (None, Some(expect)) => Some((LintAttributeKind::Expect, expect)),
        (None, None) => None,
    }
}

fn find_token_outside_rust_strings(text: &str, token: &str) -> Option<usize> {
    let mut cursor = 0;
    while cursor < text.len() {
        if text
            .get(cursor..)
            .is_some_and(|rest| rest.starts_with(token))
            && token_starts_at_attribute_boundary(text, cursor)
        {
            return Some(cursor);
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
    None
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
