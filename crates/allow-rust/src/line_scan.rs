use allow_core::{Finding, FindingKind, normalize_snippet};
use std::path::Path;

use crate::finding_builder::{FindingSite, push_finding};
use crate::line_facts::SyntaxLineFacts;
use crate::safety_comments::{has_nearby_safety_comment, safety_comment_lines};
use crate::syntax_kinds::{LintAttributeKind, RustSyntaxFacts};
use crate::text::{
    attribute_column, column, detect_attr, extract_first_lint, index_symbol, lint_policy_reference,
    receiver_before_method_column,
};

pub(crate) fn scan_source_lines(
    path: &Path,
    source: &str,
    syntax: RustSyntaxFacts,
) -> Vec<Finding> {
    let safety_comments = safety_comment_lines(source);
    let mut findings = Vec::new();

    for (line_idx, raw_line) in source.lines().enumerate() {
        let line_no = (line_idx + 1) as u32;
        let line = raw_line;
        let scope = syntax.scopes.get(&line_no).cloned().unwrap_or_default();

        scan_line(
            path,
            line,
            line_no,
            &scope.container,
            &scope.module_path,
            SyntaxLineFacts {
                lint_attributes: syntax
                    .lint_attributes
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                panic_macros: syntax
                    .panic_macros
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                panic_methods: syntax
                    .panic_methods
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                index_column: syntax.index_columns.get(&line_no).copied(),
                unsafe_constructs: syntax
                    .unsafe_constructs
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                unsafe_attribute: syntax.unsafe_attribute_lines.contains(&line_no),
                safety_comment_nearby: has_nearby_safety_comment(&safety_comments, line_no),
            },
            &mut findings,
        );
    }

    findings
}

fn scan_line(
    path: &Path,
    line: &str,
    line_no: u32,
    container: &Option<String>,
    module_stack: &[String],
    syntax: SyntaxLineFacts<'_>,
    findings: &mut Vec<Finding>,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return;
    }

    for attr_kind in syntax.lint_attributes {
        let Some(attr_text) = detect_attr(trimmed, attr_kind.name()) else {
            continue;
        };
        let lint = extract_first_lint(attr_text);
        let policy_id = lint_policy_reference(trimmed);
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: attribute_column(line),
                container,
                module_stack,
            },
            FindingKind::LintException,
            match attr_kind {
                LintAttributeKind::Allow => "allow_attribute",
                LintAttributeKind::Expect => "expect_attribute",
            },
            "attribute",
            |id| {
                id.lint = lint;
                id.symbol = Some(trimmed.to_string());
                id.target_fingerprint = policy_id.map(|policy_id| format!("policy:{policy_id}"));
            },
            findings,
        );
    }

    for unsafe_construct in syntax.unsafe_constructs {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: unsafe_construct.column,
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            unsafe_construct.kind.family(),
            unsafe_construct.kind.ast_kind(),
            |id| {
                if syntax.safety_comment_nearby {
                    id.target_fingerprint = Some("safety-comment:present".to_string());
                }
            },
            findings,
        );
    }
    if syntax.unsafe_attribute {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_attr",
            "unsafe_attr",
            |id| {
                if syntax.safety_comment_nearby {
                    id.target_fingerprint = Some("safety-comment:present".to_string());
                }
            },
            findings,
        );
    }

    for method_call in syntax.panic_methods {
        let receiver = receiver_before_method_column(line, method_call.column);
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: method_call.column,
                container,
                module_stack,
            },
            FindingKind::Panic,
            method_call.kind.family(),
            "method_call",
            |id| {
                id.callee = Some(method_call.kind.family().to_string());
                id.receiver_fingerprint = Some(receiver);
            },
            findings,
        );
    }

    for macro_invocation in syntax.panic_macros {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: macro_invocation.column,
                container,
                module_stack,
            },
            FindingKind::Panic,
            macro_invocation.kind.family(),
            "macro_call",
            |id| {
                id.macro_name = Some(macro_invocation.kind.macro_name().to_string());
            },
            findings,
        );
    }

    if let Some(index_column) = syntax.index_column {
        let family = if line.contains("&") && line.contains("[") {
            "string_slice"
        } else {
            "indexing"
        };
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: index_column,
                container,
                module_stack,
            },
            FindingKind::Panic,
            family,
            "index_expr",
            |id| {
                id.symbol = Some(index_symbol(line));
                id.target_fingerprint = line.split('[').next().map(|s| {
                    normalize_snippet(s)
                        .chars()
                        .rev()
                        .take(40)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect()
                });
            },
            findings,
        );
    }
}
