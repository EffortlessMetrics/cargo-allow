use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::IndexExpression;

pub(crate) fn scan_index_expr(
    context: LineContext<'_>,
    index_expressions: &[IndexExpression],
    findings: &mut Vec<Finding>,
) {
    for expression in index_expressions {
        let family = if expression.is_slice {
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
                id.symbol = Some(expression.symbol.clone());
                id.receiver_fingerprint = expression.receiver_fingerprint.clone();
                id.target_fingerprint = expression.target_fingerprint.clone();
            },
            findings,
        );
    }
}
