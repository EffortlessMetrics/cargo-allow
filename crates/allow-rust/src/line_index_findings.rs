use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::IndexExpression;
use crate::text::{index_symbol, index_target_fingerprint};

pub(crate) fn scan_index_expr(
    context: LineContext<'_>,
    index_expressions: &[IndexExpression],
    findings: &mut Vec<Finding>,
) {
    for expression in index_expressions {
        let family = if context.line.contains("&") && context.line.contains("[") {
            "string_slice"
        } else {
            "indexing"
        };
        push_finding(
            context.site(expression.column),
            FindingKind::Panic,
            family,
            "index_expr",
            |id| {
                id.symbol = Some(index_symbol(context.line));
                id.receiver_fingerprint = expression.receiver_fingerprint.clone();
                id.target_fingerprint = index_target_fingerprint(context.line);
            },
            findings,
        );
    }
}
