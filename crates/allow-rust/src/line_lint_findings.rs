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

#[cfg(test)]
mod tests {
    use super::scan_lint_attributes;
    use crate::line_context::LineContext;
    use crate::syntax_kinds::{LintAttribute, LintAttributeKind};
    use allow_core::{FindingKind, normalize_snippet, stable_hash_hex};
    use std::path::{Path, PathBuf};

    fn context<'a>(
        path: &'a Path,
        line: &'a str,
        container: &'a Option<String>,
        module_stack: &'a [String],
    ) -> LineContext<'a> {
        LineContext {
            path,
            line,
            line_no: 12,
            container,
            module_stack,
        }
    }

    #[test]
    fn scan_lint_attributes_emits_each_lint_with_policy_and_context() {
        let line = r#"#[allow(dead_code, unused_variables)] #[expect(clippy::unwrap_used, reason = "policy:allow-lint")]"#;
        let lint_attributes = [
            LintAttribute {
                kind: LintAttributeKind::Allow,
                text: "   #[allow(dead_code, unused_variables)]   ".to_string(),
                column: 1,
            },
            LintAttribute {
                kind: LintAttributeKind::Expect,
                text: r#"   #[expect(clippy::unwrap_used, reason = "policy:allow-lint")]   "#
                    .to_string(),
                column: 40,
            },
        ];
        let container = Some("linted".to_string());
        let module_stack = vec!["parser".to_string(), "rules".to_string()];
        let mut findings = Vec::new();

        scan_lint_attributes(
            context(Path::new("src/lib.rs"), line, &container, &module_stack),
            &lint_attributes,
            &mut findings,
        );

        assert_eq!(findings.len(), 3);
        let observed = findings
            .iter()
            .map(|finding| {
                (
                    finding.family.as_deref(),
                    finding.identity.lint.as_deref(),
                    finding.identity.target_fingerprint.as_deref(),
                    finding.span.as_ref().map(|span| span.column),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed,
            vec![
                (Some("allow_attribute"), Some("dead_code"), None, Some(1)),
                (
                    Some("allow_attribute"),
                    Some("unused_variables"),
                    None,
                    Some(1)
                ),
                (
                    Some("expect_attribute"),
                    Some("clippy::unwrap_used"),
                    Some("policy:allow-lint"),
                    Some(40)
                ),
            ]
        );

        match findings.as_slice() {
            [allow, _, expect] => {
                assert_eq!(allow.kind, FindingKind::LintException);
                assert_eq!(allow.path, PathBuf::from("src/lib.rs"));
                assert_eq!(allow.span.as_ref().map(|span| span.line), Some(12));
                assert_eq!(allow.identity.ast_kind, "attribute");
                assert_eq!(allow.identity.container.as_deref(), Some("linted"));
                assert_eq!(allow.identity.module.as_deref(), Some("parser::rules"));
                assert_eq!(allow.identity.line_hint, Some(12));
                assert_eq!(
                    allow.identity.normalized_snippet_hash.as_deref(),
                    Some(stable_hash_hex(&normalize_snippet(line)).as_str())
                );
                assert_eq!(
                    allow.identity.symbol.as_deref(),
                    Some("#[allow(dead_code, unused_variables)]")
                );
                assert_eq!(allow.message, "lint_exception allow_attribute syntax found");

                assert_eq!(
                    expect.identity.symbol.as_deref(),
                    Some(r#"#[expect(clippy::unwrap_used, reason = "policy:allow-lint")]"#)
                );
                assert_eq!(
                    expect.message,
                    "lint_exception expect_attribute syntax found"
                );
            }
            other => assert_eq!(other.len(), 3),
        }
    }

    #[test]
    fn scan_lint_attributes_skips_mismatched_and_metadata_only_attributes() {
        let line = r#"#[expect(dead_code)] #[allow(reason = "policy:allow-lint")]"#;
        let lint_attributes = [
            LintAttribute {
                kind: LintAttributeKind::Allow,
                text: "#[expect(dead_code)]".to_string(),
                column: 1,
            },
            LintAttribute {
                kind: LintAttributeKind::Allow,
                text: r#"#[allow(reason = "policy:allow-lint")]"#.to_string(),
                column: 22,
            },
        ];
        let container = None;
        let module_stack = Vec::new();
        let mut findings = Vec::new();

        scan_lint_attributes(
            context(Path::new("src/lib.rs"), line, &container, &module_stack),
            &lint_attributes,
            &mut findings,
        );

        assert!(findings.is_empty());
    }
}
