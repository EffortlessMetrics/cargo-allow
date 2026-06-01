use allow_core::Finding;
use std::path::Path;

use crate::line_facts::SyntaxLineFacts;
use crate::line_findings::scan_line;
use crate::safety_comments::{has_nearby_safety_comment, safety_comment_lines};
use crate::syntax_kinds::RustSyntaxFacts;

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
                index_expressions: syntax
                    .index_expressions
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                unsafe_constructs: syntax
                    .unsafe_constructs
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                unsafe_attribute_columns: syntax
                    .unsafe_attribute_columns
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                safety_comment_nearby: has_nearby_safety_comment(&safety_comments, line_no),
            },
            &mut findings,
        );
    }

    findings
}
