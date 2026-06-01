use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::UnsafeSyntaxConstruct;

pub(crate) fn scan_unsafe_constructs(
    context: UnsafeLineContext<'_>,
    unsafe_constructs: &[UnsafeSyntaxConstruct],
    unsafe_attribute_columns: &[u32],
    findings: &mut Vec<Finding>,
) {
    for unsafe_construct in unsafe_constructs {
        push_finding(
            context.line.site(unsafe_construct.column),
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
    for column in unsafe_attribute_columns {
        push_finding(
            context.line.site(*column),
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
    pub(crate) line: LineContext<'a>,
    pub(crate) safety_comment_nearby: bool,
}
