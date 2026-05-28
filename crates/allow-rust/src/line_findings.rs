use allow_core::{Finding, FindingKind};
use std::path::Path;

use crate::finding_builder::{FindingSite, push_finding};
use crate::line_facts::SyntaxLineFacts;
use crate::line_lint_findings::scan_lint_attributes;
use crate::line_unsafe_findings::{UnsafeLineContext, scan_unsafe_constructs};
use crate::text::{index_symbol, index_target_fingerprint, receiver_before_method_column};

pub(crate) fn scan_line(
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

    scan_lint_attributes(
        path,
        line,
        line_no,
        container,
        module_stack,
        syntax.lint_attributes,
        findings,
    );

    scan_unsafe_constructs(
        UnsafeLineContext {
            path,
            line,
            line_no,
            container,
            module_stack,
            safety_comment_nearby: syntax.safety_comment_nearby,
        },
        syntax.unsafe_constructs,
        syntax.unsafe_attribute,
        findings,
    );

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
                id.target_fingerprint = index_target_fingerprint(line);
            },
            findings,
        );
    }
}
