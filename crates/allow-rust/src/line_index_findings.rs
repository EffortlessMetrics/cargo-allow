use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::text::{index_symbol, index_target_fingerprint};

pub(crate) fn scan_index_expr(
    context: LineContext<'_>,
    index_columns: &[u32],
    findings: &mut Vec<Finding>,
) {
    for index_column in index_columns {
        let family = if context.line.contains("&") && context.line.contains("[") {
            "string_slice"
        } else {
            "indexing"
        };
        push_finding(
            context.site(*index_column),
            FindingKind::Panic,
            family,
            "index_expr",
            |id| {
                id.symbol = Some(index_symbol(context.line));
                id.target_fingerprint = index_target_fingerprint(context.line);
            },
            findings,
        );
    }
}
