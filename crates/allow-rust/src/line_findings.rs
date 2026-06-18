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
        syntax.unsafe_attributes,
        findings,
    );

    scan_panic_calls(context, syntax.panic_methods, syntax.panic_macros, findings);

    scan_index_expr(context, syntax.index_expressions, findings);
}

#[cfg(test)]
mod tests {
    use super::scan_line;
    use crate::line_facts::SyntaxLineFacts;
    use crate::syntax_kinds::{
        IndexExpression, LintAttribute, LintAttributeKind, PanicMacroInvocation, PanicMacroKind,
        PanicMethodCall, PanicMethodKind, UnsafeSyntaxConstruct, UnsafeSyntaxKind,
    };
    use allow_core::{Finding, FindingKind};
    use std::path::Path;

    fn empty_syntax<'a>() -> SyntaxLineFacts<'a> {
        SyntaxLineFacts {
            lint_attributes: &[],
            panic_macros: &[],
            panic_methods: &[],
            index_expressions: &[],
            unsafe_constructs: &[],
            unsafe_attributes: &[],
            safety_comment_nearby: false,
        }
    }

    #[test]
    fn scan_line_skips_blank_and_comment_lines_before_routing_facts() {
        let lint_attributes = [LintAttribute {
            kind: LintAttributeKind::Allow,
            text: "#[allow(clippy::unwrap_used)]".to_string(),
            column: 2,
        }];
        let syntax = SyntaxLineFacts {
            lint_attributes: &lint_attributes,
            ..empty_syntax()
        };
        let container = Some("parse".to_string());
        let modules = vec!["parser".to_string()];
        let mut findings = Vec::new();

        scan_line(
            Path::new("src/lib.rs"),
            "   ",
            7,
            &container,
            &modules,
            syntax,
            &mut findings,
        );
        scan_line(
            Path::new("src/lib.rs"),
            " // #[allow(clippy::unwrap_used)]",
            8,
            &container,
            &modules,
            SyntaxLineFacts {
                lint_attributes: &lint_attributes,
                ..empty_syntax()
            },
            &mut findings,
        );

        assert!(findings.is_empty());
    }

    #[test]
    fn scan_line_routes_each_syntax_fact_group_with_context() {
        let line =
            r#"#[allow(clippy::unwrap_used)] unsafe { values[index].unwrap(); panic!("bad"); }"#;
        let lint_attributes = [LintAttribute {
            kind: LintAttributeKind::Allow,
            text: "#[allow(clippy::unwrap_used)]".to_string(),
            column: 2,
        }];
        let unsafe_constructs = [UnsafeSyntaxConstruct {
            kind: UnsafeSyntaxKind::Block,
            column: 32,
            symbol: Some("unsafe".to_string()),
        }];
        let panic_methods = [PanicMethodCall {
            kind: PanicMethodKind::Unwrap,
            column: 55,
            receiver_fingerprint: Some("recv:values-index".to_string()),
        }];
        let panic_macros = [PanicMacroInvocation {
            kind: PanicMacroKind::Panic,
            column: 65,
            macro_path: "panic".to_string(),
        }];
        let index_expressions = [IndexExpression {
            column: 39,
            symbol: "values[index]".to_string(),
            receiver_fingerprint: Some("recv:values".to_string()),
            target_fingerprint: Some("target:index".to_string()),
            is_slice: false,
        }];
        let container = Some("parse".to_string());
        let modules = vec!["parser".to_string(), "lexer".to_string()];
        let mut findings = Vec::new();

        scan_line(
            Path::new("src/lib.rs"),
            line,
            7,
            &container,
            &modules,
            SyntaxLineFacts {
                lint_attributes: &lint_attributes,
                panic_macros: &panic_macros,
                panic_methods: &panic_methods,
                index_expressions: &index_expressions,
                unsafe_constructs: &unsafe_constructs,
                unsafe_attributes: &[],
                safety_comment_nearby: true,
            },
            &mut findings,
        );

        assert_eq!(findings.len(), 5);
        assert!(has_finding(
            &findings,
            FindingKind::LintException,
            "allow_attribute"
        ));
        assert!(has_finding(&findings, FindingKind::Unsafe, "unsafe_block"));
        assert!(has_finding(&findings, FindingKind::Panic, "unwrap"));
        assert!(has_finding(&findings, FindingKind::Panic, "panic_macro"));
        assert!(has_finding(&findings, FindingKind::Panic, "indexing"));
        assert!(
            findings
                .iter()
                .all(|finding| finding.path == Path::new("src/lib.rs"))
        );
        assert!(
            findings.iter().all(
                |finding| finding.identity.container.as_deref() == Some("parser::lexer::parse")
            )
        );
        assert!(
            findings
                .iter()
                .all(|finding| finding.identity.module.as_deref() == Some("parser::lexer"))
        );
        assert!(
            findings
                .iter()
                .all(|finding| finding.identity.line_hint == Some(7))
        );
        assert!(findings.iter().any(|finding| {
            finding.kind == FindingKind::Unsafe
                && finding.identity.target_fingerprint.as_deref() == Some("safety-comment:present")
        }));
    }

    fn has_finding(findings: &[Finding], kind: FindingKind, family: &str) -> bool {
        findings
            .iter()
            .any(|finding| finding.kind == kind && finding.family.as_deref() == Some(family))
    }
}
