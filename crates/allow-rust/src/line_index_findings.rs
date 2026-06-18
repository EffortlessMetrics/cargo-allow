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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use allow_core::FindingKind;

    use super::*;

    #[test]
    fn scan_index_expr_projects_index_and_slice_families_with_identity() {
        let container = Some("load".to_string());
        let modules = vec!["parser".to_string()];
        let context = line_context(&container, &modules);
        let expressions = [
            index_expression(9, "items[index]", Some("items"), Some("index"), false),
            index_expression(24, "text[1..3]", Some("text"), Some("1..3"), true),
        ];
        let mut findings = Vec::new();

        scan_index_expr(context, &expressions, &mut findings);

        assert_eq!(findings.len(), 2);
        let index = finding_with_family(&findings, "indexing");
        assert_eq!(index.kind, FindingKind::Panic);
        assert_eq!(index.path, Path::new("src/lib.rs"));
        assert_eq!(index.identity.ast_kind, "index_expr");
        assert_eq!(index.identity.symbol.as_deref(), Some("items[index]"));
        assert_eq!(
            index.identity.receiver_fingerprint.as_deref(),
            Some("items")
        );
        assert_eq!(index.identity.target_fingerprint.as_deref(), Some("index"));
        assert_eq!(index.identity.container.as_deref(), Some("parser::load"));
        assert_eq!(index.identity.module.as_deref(), Some("parser"));
        assert_eq!(index.identity.line_hint, Some(42));
        assert_eq!(index.identity.column_hint, Some(9));

        let slice = finding_with_family(&findings, "string_slice");
        assert_eq!(slice.kind, FindingKind::Panic);
        assert_eq!(slice.identity.ast_kind, "index_expr");
        assert_eq!(slice.identity.symbol.as_deref(), Some("text[1..3]"));
        assert_eq!(slice.identity.receiver_fingerprint.as_deref(), Some("text"));
        assert_eq!(slice.identity.target_fingerprint.as_deref(), Some("1..3"));
        assert_eq!(slice.identity.column_hint, Some(24));
    }

    #[test]
    fn scan_index_expr_appends_to_existing_findings() {
        let container = None;
        let modules = Vec::new();
        let context = line_context(&container, &modules);
        let expression = [index_expression(
            13,
            "values[pos]",
            Some("values"),
            None,
            false,
        )];
        let mut findings = Vec::new();

        scan_index_expr(context, &[], &mut findings);
        assert!(findings.is_empty());

        scan_index_expr(context, &expression, &mut findings);

        assert_eq!(findings.len(), 1);
        let finding = finding_with_family(&findings, "indexing");
        assert_eq!(finding.identity.symbol.as_deref(), Some("values[pos]"));
        assert_eq!(
            finding.identity.receiver_fingerprint.as_deref(),
            Some("values")
        );
        assert_eq!(finding.identity.target_fingerprint, None);
        assert_eq!(finding.identity.container, None);
        assert_eq!(finding.identity.module, None);
    }

    fn line_context<'a>(
        container: &'a Option<String>,
        module_stack: &'a [String],
    ) -> LineContext<'a> {
        LineContext {
            path: Path::new("src/lib.rs"),
            line: "    let value = items[index] + text[1..3].len();",
            line_no: 42,
            container,
            module_stack,
        }
    }

    fn index_expression(
        column: u32,
        symbol: &str,
        receiver_fingerprint: Option<&str>,
        target_fingerprint: Option<&str>,
        is_slice: bool,
    ) -> IndexExpression {
        IndexExpression {
            column,
            symbol: symbol.to_string(),
            receiver_fingerprint: receiver_fingerprint.map(str::to_string),
            target_fingerprint: target_fingerprint.map(str::to_string),
            is_slice,
        }
    }

    fn finding_with_family<'a>(findings: &'a [Finding], family: &str) -> &'a Finding {
        findings
            .iter()
            .find(|finding| finding.family.as_deref() == Some(family))
            .unwrap_or_else(|| std::panic::panic_any(format!("expected {family} finding")))
    }
}
