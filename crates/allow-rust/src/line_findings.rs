use allow_core::Finding;
use std::path::Path;

use crate::line_context::LineContext;
use crate::line_facts::SyntaxLineFacts;
use crate::line_index_findings::scan_index_expr;
use crate::line_lint_findings::scan_lint_attributes;
use crate::line_panic_findings::scan_panic_calls;
use crate::line_unsafe_findings::{UnsafeLineContext, scan_unsafe_constructs};

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

    let context = LineContext {
        path,
        line,
        line_no,
        container,
        module_stack,
    };

    scan_lint_attributes(context, syntax.lint_attributes, findings);

    scan_unsafe_constructs(
        UnsafeLineContext {
            line: context,
            safety_comment_nearby: syntax.safety_comment_nearby,
        },
        syntax.unsafe_constructs,
        syntax.unsafe_attribute_columns,
        findings,
    );

    scan_panic_calls(context, syntax.panic_methods, syntax.panic_macros, findings);

    scan_index_expr(context, syntax.index_columns, findings);
}
