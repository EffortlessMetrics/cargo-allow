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
    for (kind, offset) in lint_attribute_kinds(text) {
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
    let unsafe_attribute_offsets = unsafe_attribute_offsets(text);
    if !unsafe_attribute_offsets.is_empty() {
        let columns = facts.unsafe_attribute_columns.entry(line).or_default();
        for offset in unsafe_attribute_offsets {
            columns.push(source_column(source, start.row, start.column + offset));
        }
    }
}

fn lint_attribute_kinds(text: &str) -> Vec<(LintAttributeKind, usize)> {
    let trimmed = text.trim_start();
    if detect_attr(trimmed, "allow").is_some() {
        vec![(LintAttributeKind::Allow, text.len() - trimmed.len())]
    } else if detect_attr(trimmed, "expect").is_some() {
        vec![(LintAttributeKind::Expect, text.len() - trimmed.len())]
    } else if trimmed.starts_with("#[cfg_attr(") || trimmed.starts_with("#![cfg_attr(") {
        cfg_attr_lint_kinds(text)
    } else {
        Vec::new()
    }
}

fn unsafe_attribute_offsets(text: &str) -> Vec<usize> {
    let trimmed = text.trim_start();
    if trimmed.starts_with("#[unsafe(") || trimmed.starts_with("#![unsafe(") {
        return vec![text.len() - trimmed.len()];
    }
    if !(trimmed.starts_with("#[cfg_attr(") || trimmed.starts_with("#![cfg_attr(")) {
        return Vec::new();
    }
    find_tokens_outside_rust_strings(text, "unsafe(")
}

fn cfg_attr_lint_kinds(text: &str) -> Vec<(LintAttributeKind, usize)> {
    let mut attributes = find_tokens_outside_rust_strings(text, "allow(")
        .into_iter()
        .map(|offset| (LintAttributeKind::Allow, offset))
        .chain(
            find_tokens_outside_rust_strings(text, "expect(")
                .into_iter()
                .map(|offset| (LintAttributeKind::Expect, offset)),
        )
        .collect::<Vec<_>>();
    attributes.sort_by_key(|(_, offset)| *offset);
    attributes
}

fn find_tokens_outside_rust_strings(text: &str, token: &str) -> Vec<usize> {
    let mut matches = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        if text
            .get(cursor..)
            .is_some_and(|rest| rest.starts_with(token))
            && token_starts_at_attribute_boundary(text, cursor)
        {
            matches.push(cursor);
            cursor += token.len();
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
