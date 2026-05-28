use allow_core::{Finding, FindingKind};
use std::path::Path;

use crate::finding_builder::{FindingSite, push_finding};
use crate::syntax_kinds::LintAttributeKind;
use crate::text::{attribute_column, detect_attr, extract_first_lint, lint_policy_reference};

pub(crate) fn scan_lint_attributes(
    path: &Path,
    line: &str,
    line_no: u32,
    container: &Option<String>,
    module_stack: &[String],
    lint_attributes: &[LintAttributeKind],
    findings: &mut Vec<Finding>,
) {
    let trimmed = line.trim();
    for attr_kind in lint_attributes {
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
}
