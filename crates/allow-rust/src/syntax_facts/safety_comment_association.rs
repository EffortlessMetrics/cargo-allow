use std::collections::BTreeMap;

use tree_sitter::Node;

use crate::syntax_kinds::{RustSyntaxFacts, SafetyCommentAssociation, SafetyCommentFact};

pub(super) fn associate_safety_comments(
    root: Node<'_>,
    _source: &str,
    facts: &mut RustSyntaxFacts,
) {
    if facts.safety_comment_facts.is_empty() {
        return;
    }

    let mut anchor_candidates: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();

    for (line, constructs) in &facts.unsafe_constructs {
        for construct in constructs {
            let key = (*line, construct.column);
            let candidates = structural_comment_candidates(
                root,
                construct.start_byte,
                &facts.safety_comment_facts,
            );
            if !candidates.is_empty() {
                anchor_candidates.insert(key, candidates);
            }
        }
    }

    for (line, attributes) in &facts.unsafe_attributes {
        for attribute in attributes {
            let key = (*line, attribute.column);
            let candidates = structural_comment_candidates(
                root,
                attribute.start_byte,
                &facts.safety_comment_facts,
            );
            if !candidates.is_empty() {
                anchor_candidates.insert(key, candidates);
            }
        }
    }

    let mut comment_anchors: BTreeMap<usize, Vec<(u32, u32)>> = BTreeMap::new();
    for (anchor, candidates) in &anchor_candidates {
        for comment_index in candidates {
            comment_anchors
                .entry(*comment_index)
                .or_default()
                .push(*anchor);
        }
    }

    for (anchor, candidates) in anchor_candidates {
        let association = if candidates.len() == 1 {
            let comment_index = candidates[0];
            if comment_anchors
                .get(&comment_index)
                .is_some_and(|anchors| anchors.len() == 1)
            {
                SafetyCommentAssociation::Attached
            } else {
                SafetyCommentAssociation::NearbyAmbiguous
            }
        } else {
            SafetyCommentAssociation::NearbyAmbiguous
        };
        facts
            .safety_comment_associations
            .insert(anchor, association);
    }
}

fn structural_comment_candidates(
    root: Node<'_>,
    anchor_byte: usize,
    comments: &[SafetyCommentFact],
) -> Vec<usize> {
    let Some(anchor_node) = smallest_node_at_byte(root, anchor_byte) else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    candidates.extend(preceding_safety_comment_indices(anchor_node, comments));
    candidates.extend(trailing_safety_comment_indices(
        anchor_node,
        anchor_byte,
        comments,
    ));
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn preceding_safety_comment_indices(
    anchor_node: Node<'_>,
    comments: &[SafetyCommentFact],
) -> Vec<usize> {
    let Some(statement) = statement_or_item_ancestor(anchor_node) else {
        return Vec::new();
    };
    let Some(parent) = statement.parent() else {
        return Vec::new();
    };
    let siblings = parent_children(parent);
    let Some(statement_index) = siblings
        .iter()
        .position(|child| child.id() == statement.id())
    else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    for child in siblings[..statement_index].iter().rev() {
        if is_comment_node(child.kind()) {
            if let Some(index) = comment_index_for_node(comments, *child) {
                candidates.push(index);
            }
            continue;
        }
        if is_insignificant_sibling(child.kind()) {
            continue;
        }
        break;
    }
    candidates.reverse();
    candidates
}

fn trailing_safety_comment_indices(
    anchor_node: Node<'_>,
    anchor_byte: usize,
    comments: &[SafetyCommentFact],
) -> Vec<usize> {
    let Some(statement) = statement_or_item_ancestor(anchor_node) else {
        return Vec::new();
    };
    let anchor_line = anchor_node.start_position().row as u32 + 1;
    let mut candidates = Vec::new();

    for (index, comment) in comments.iter().enumerate() {
        if comment.start_line == anchor_line && comment.start_byte >= anchor_byte {
            candidates.push(index);
        }
    }

    let mut cursor = statement.walk();
    for child in statement.children(&mut cursor) {
        if is_comment_node(child.kind()) && child.start_byte() >= anchor_byte {
            if let Some(index) = comment_index_for_node(comments, child) {
                candidates.push(index);
            }
        }
    }

    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn statement_or_item_ancestor(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        let kind = node.kind();
        if is_statement_or_item(kind) {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn is_statement_or_item(kind: &str) -> bool {
    kind.ends_with("_item")
        || matches!(
            kind,
            "expression_statement"
                | "let_declaration"
                | "assignment_expression"
                | "use_declaration"
                | "extern_crate_declaration"
                | "macro_invocation"
                | "macro_definition"
        )
}

fn is_comment_node(kind: &str) -> bool {
    matches!(kind, "line_comment" | "block_comment")
}

fn is_insignificant_sibling(kind: &str) -> bool {
    matches!(kind, "{" | "}" | "(" | ")" | "[" | "]" | ";" | ",")
}

fn parent_children(parent: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = parent.walk();
    parent.children(&mut cursor).collect()
}

fn smallest_node_at_byte(node: Node<'_>, byte: usize) -> Option<Node<'_>> {
    if byte < node.start_byte() || byte >= node.end_byte() {
        return None;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = smallest_node_at_byte(child, byte) {
            return Some(found);
        }
    }
    Some(node)
}

fn comment_index_for_node(comments: &[SafetyCommentFact], node: Node<'_>) -> Option<usize> {
    let start = node.start_byte();
    let end = node.end_byte();
    comments
        .iter()
        .position(|comment| comment.start_byte == start && comment.end_byte == end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax_facts::syntax_facts_with_outcome;

    fn association_for(source: &str, family: &str) -> Option<SafetyCommentAssociation> {
        let findings = crate::scan_rust_source("src/lib.rs", source);
        let fingerprint = findings
            .iter()
            .find(|finding| finding.family.as_deref() == Some(family))?
            .identity
            .target_fingerprint
            .as_deref();
        match fingerprint {
            Some("safety-comment:present") => Some(SafetyCommentAssociation::Attached),
            Some("safety-comment:nearby-ambiguous") => {
                Some(SafetyCommentAssociation::NearbyAmbiguous)
            }
            _ => None,
        }
    }

    #[test]
    fn safety_comment_association_rejects_intervening_statement() {
        let source = r#"
fn read(ptr: *const u8) -> u8 {
    // SAFETY: caller validates the pointer.
    let guard = true;
    unsafe { core::ptr::read(ptr) }
}
"#;
        assert_eq!(
            association_for(source, "unsafe_block"),
            None,
            "intervening statement must block structural attachment"
        );
    }

    #[test]
    fn safety_comment_association_marks_multiple_markers_ambiguous() {
        let source = r#"
fn read(ptr: *const u8) -> u8 {
    // SAFETY: first proof
    // SAFETY: second proof
    unsafe { core::ptr::read(ptr) }
}
"#;
        assert_eq!(
            association_for(source, "unsafe_block"),
            Some(SafetyCommentAssociation::NearbyAmbiguous)
        );
    }

    #[test]
    fn safety_comment_association_does_not_attach_across_sibling_items() {
        let source = r#"
// SAFETY: only for the first function
unsafe fn first() {}

unsafe fn second() {}
"#;
        let findings = crate::scan_rust_source("src/lib.rs", source);
        let first = findings
            .iter()
            .find(|finding| finding.identity.symbol.as_deref() == Some("first"))
            .expect("first unsafe fn should be found");
        let second = findings
            .iter()
            .find(|finding| finding.identity.symbol.as_deref() == Some("second"))
            .expect("second unsafe fn should be found");
        assert_eq!(
            first.identity.target_fingerprint.as_deref(),
            Some("safety-comment:present")
        );
        assert_ne!(
            second.identity.target_fingerprint.as_deref(),
            Some("safety-comment:present")
        );

        let outcome = syntax_facts_with_outcome(source);
        let first_anchor = outcome
            .facts
            .unsafe_constructs
            .iter()
            .find_map(|(line, constructs)| {
                constructs
                    .iter()
                    .find(|construct| construct.symbol.as_deref() == Some("first"))
                    .map(|construct| (*line, construct.column))
            })
            .expect("first unsafe fn should be recorded");
        assert_eq!(
            outcome.facts.safety_comment_associations.get(&first_anchor),
            Some(&SafetyCommentAssociation::Attached)
        );

        let second_anchor = outcome
            .facts
            .unsafe_constructs
            .iter()
            .find_map(|(line, constructs)| {
                constructs
                    .iter()
                    .find(|construct| construct.symbol.as_deref() == Some("second"))
                    .map(|construct| (*line, construct.column))
            })
            .expect("second unsafe fn should be recorded");
        assert!(
            !outcome
                .facts
                .safety_comment_associations
                .contains_key(&second_anchor)
        );
    }

    #[test]
    fn safety_comment_association_keeps_immediate_predecessor_attachment() {
        let source = r#"
fn read(ptr: *const u8) -> u8 {
    // SAFETY: caller validates the pointer.
    unsafe { core::ptr::read(ptr) }
}
"#;
        assert_eq!(
            association_for(source, "unsafe_block"),
            Some(SafetyCommentAssociation::Attached)
        );
    }
}
