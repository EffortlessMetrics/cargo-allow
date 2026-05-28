use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::LintAttributeKind;
use crate::text::{attribute_column, detect_attr, extract_first_lint, lint_policy_reference};

pub(crate) fn scan_lint_attributes(
    context: LineContext<'_>,
    lint_attributes: &[LintAttributeKind],
    findings: &mut Vec<Finding>,
) {
    let trimmed = context.line.trim();
    for attr_kind in lint_attributes {
        let Some(attr_text) = detect_attr(trimmed, attr_kind.name()) else {
            continue;
        };
        let lint = extract_first_lint(attr_text);
        let policy_id = lint_policy_reference(trimmed);
        push_finding(
            context.site(attribute_column(context.line)),
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
