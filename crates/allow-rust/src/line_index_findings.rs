use allow_core::{Finding, FindingKind};
use std::path::Path;

use crate::finding_builder::{FindingSite, push_finding};
use crate::text::{index_symbol, index_target_fingerprint};

pub(crate) fn scan_index_expr(
    context: IndexLineContext<'_>,
    index_column: Option<u32>,
    findings: &mut Vec<Finding>,
) {
    if let Some(index_column) = index_column {
        let family = if context.line.contains("&") && context.line.contains("[") {
            "string_slice"
        } else {
            "indexing"
        };
        push_finding(
            FindingSite {
                path: context.path,
                line: context.line,
                line_no: context.line_no,
                column: index_column,
                container: context.container,
                module_stack: context.module_stack,
            },
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

pub(crate) struct IndexLineContext<'a> {
    pub(crate) path: &'a Path,
    pub(crate) line: &'a str,
    pub(crate) line_no: u32,
    pub(crate) container: &'a Option<String>,
    pub(crate) module_stack: &'a [String],
}
