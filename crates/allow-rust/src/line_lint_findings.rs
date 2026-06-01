use allow_core::{Finding, FindingKind, normalize_snippet};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::{LintAttribute, LintAttributeKind};
use crate::text::{detect_attr, extract_lints, lint_policy_reference};

pub(crate) fn scan_lint_attributes(
    context: LineContext<'_>,
    lint_attributes: &[LintAttribute],
    findings: &mut Vec<Finding>,
) {
    for attr in lint_attributes {
        let trimmed = attr.text.trim();
        let Some(attr_text) = detect_attr(trimmed, attr.kind.name()) else {
            continue;
        };
        let policy_id = lint_policy_reference(trimmed);
        let lints = extract_lints(attr_text);
        for lint in lints {
            let policy_fingerprint = policy_id
                .as_ref()
                .map(|policy_id| format!("policy:{policy_id}"));
            push_finding(
                context.site(attr.column),
                FindingKind::LintException,
                match attr.kind {
                    LintAttributeKind::Allow => "allow_attribute",
                    LintAttributeKind::Expect => "expect_attribute",
                },
                "attribute",
                |id| {
                    id.lint = Some(lint);
                    id.symbol = Some(normalize_snippet(trimmed));
                    id.target_fingerprint = policy_fingerprint;
                },
                findings,
            );
        }
    }
}
