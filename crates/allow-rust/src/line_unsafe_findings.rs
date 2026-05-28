use allow_core::{Finding, FindingKind};
use std::path::Path;

use crate::finding_builder::{FindingSite, push_finding};
use crate::syntax_kinds::UnsafeSyntaxConstruct;
use crate::text::column;

pub(crate) fn scan_unsafe_constructs(
    context: UnsafeLineContext<'_>,
    unsafe_constructs: &[UnsafeSyntaxConstruct],
    unsafe_attribute: bool,
    findings: &mut Vec<Finding>,
) {
    for unsafe_construct in unsafe_constructs {
        push_finding(
            FindingSite {
                path: context.path,
                line: context.line,
                line_no: context.line_no,
                column: unsafe_construct.column,
                container: context.container,
                module_stack: context.module_stack,
            },
            FindingKind::Unsafe,
            unsafe_construct.kind.family(),
            unsafe_construct.kind.ast_kind(),
            |id| {
                if context.safety_comment_nearby {
                    id.target_fingerprint = Some("safety-comment:present".to_string());
                }
            },
            findings,
        );
    }
    if unsafe_attribute {
        push_finding(
            FindingSite {
                path: context.path,
                line: context.line,
                line_no: context.line_no,
                column: column(context.line, "unsafe"),
                container: context.container,
                module_stack: context.module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_attr",
            "unsafe_attr",
            |id| {
                if context.safety_comment_nearby {
                    id.target_fingerprint = Some("safety-comment:present".to_string());
                }
            },
            findings,
        );
    }
}

pub(crate) struct UnsafeLineContext<'a> {
    pub(crate) path: &'a Path,
    pub(crate) line: &'a str,
    pub(crate) line_no: u32,
    pub(crate) container: &'a Option<String>,
    pub(crate) module_stack: &'a [String],
    pub(crate) safety_comment_nearby: bool,
}
