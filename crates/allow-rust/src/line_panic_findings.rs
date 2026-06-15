use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::{PanicMacroInvocation, PanicMethodCall};

pub(crate) fn scan_panic_calls(
    context: LineContext<'_>,
    panic_methods: &[PanicMethodCall],
    panic_macros: &[PanicMacroInvocation],
    findings: &mut Vec<Finding>,
) {
    for method_call in panic_methods {
        push_finding(
            context.site(method_call.column),
            FindingKind::Panic,
            method_call.kind.family(),
            "method_call",
            |id| {
                id.callee = Some(method_call.kind.family().to_string());
                id.receiver_fingerprint = method_call.receiver_fingerprint.clone();
            },
            findings,
        );
    }

    for macro_invocation in panic_macros {
        push_finding(
            context.site(macro_invocation.column),
            FindingKind::Panic,
            macro_invocation.kind.family(),
            "macro_call",
            |id| {
                id.macro_name = Some(macro_invocation.kind.macro_name().to_string());
                id.target_fingerprint = Some(macro_invocation.macro_path.clone());
            },
            findings,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::syntax_kinds::{PanicMacroKind, PanicMethodKind};

    #[test]
    fn scan_panic_calls_projects_method_families_with_identity() {
        let container = Some("load".to_string());
        let modules = vec!["parser".to_string()];
        let context = line_context(&container, &modules);
        let methods = [
            panic_method(13, PanicMethodKind::Unwrap, Some("builder.step()")),
            panic_method(31, PanicMethodKind::Expect, Some("fallback.source()")),
        ];
        let mut findings = Vec::new();

        scan_panic_calls(context, &methods, &[], &mut findings);

        assert_eq!(findings.len(), 2);
        let unwrap = finding_with_family(&findings, "unwrap");
        assert_eq!(unwrap.kind, FindingKind::Panic);
        assert_eq!(unwrap.path, Path::new("src/lib.rs"));
        assert_eq!(unwrap.identity.ast_kind, "method_call");
        assert_eq!(unwrap.identity.callee.as_deref(), Some("unwrap"));
        assert_eq!(
            unwrap.identity.receiver_fingerprint.as_deref(),
            Some("builder.step()")
        );
        assert_eq!(unwrap.identity.container.as_deref(), Some("load"));
        assert_eq!(unwrap.identity.module.as_deref(), Some("parser"));
        assert_eq!(unwrap.identity.line_hint, Some(42));
        assert_eq!(unwrap.identity.column_hint, Some(13));

        let expect = finding_with_family(&findings, "expect");
        assert_eq!(expect.kind, FindingKind::Panic);
        assert_eq!(expect.identity.ast_kind, "method_call");
        assert_eq!(expect.identity.callee.as_deref(), Some("expect"));
        assert_eq!(
            expect.identity.receiver_fingerprint.as_deref(),
            Some("fallback.source()")
        );
        assert_eq!(expect.identity.column_hint, Some(31));
    }

    #[test]
    fn scan_panic_calls_projects_macro_families_and_paths() {
        let container = Some("load".to_string());
        let modules = vec!["parser".to_string(), "fatal".to_string()];
        let context = line_context(&container, &modules);
        let macros = [
            panic_macro(7, PanicMacroKind::Panic, "std::panic"),
            panic_macro(19, PanicMacroKind::Todo, "todo"),
        ];
        let mut findings = Vec::new();

        scan_panic_calls(context, &[], &macros, &mut findings);

        assert_eq!(findings.len(), 2);
        let panic = finding_with_family(&findings, "panic_macro");
        assert_eq!(panic.kind, FindingKind::Panic);
        assert_eq!(panic.path, Path::new("src/lib.rs"));
        assert_eq!(panic.identity.ast_kind, "macro_call");
        assert_eq!(panic.identity.macro_name.as_deref(), Some("panic"));
        assert_eq!(
            panic.identity.target_fingerprint.as_deref(),
            Some("std::panic")
        );
        assert_eq!(panic.identity.container.as_deref(), Some("load"));
        assert_eq!(panic.identity.module.as_deref(), Some("parser::fatal"));
        assert_eq!(panic.identity.line_hint, Some(42));
        assert_eq!(panic.identity.column_hint, Some(7));

        let todo = finding_with_family(&findings, "todo");
        assert_eq!(todo.kind, FindingKind::Panic);
        assert_eq!(todo.identity.ast_kind, "macro_call");
        assert_eq!(todo.identity.macro_name.as_deref(), Some("todo"));
        assert_eq!(todo.identity.target_fingerprint.as_deref(), Some("todo"));
        assert_eq!(todo.identity.column_hint, Some(19));
    }

    #[test]
    fn scan_panic_calls_appends_to_existing_findings() {
        let container = None;
        let modules = Vec::new();
        let context = line_context(&container, &modules);
        let method = [panic_method(9, PanicMethodKind::Unwrap, None)];
        let mut findings = Vec::new();

        scan_panic_calls(context, &[], &[], &mut findings);
        assert!(findings.is_empty());

        scan_panic_calls(
            line_context(&container, &modules),
            &method,
            &[],
            &mut findings,
        );

        assert_eq!(findings.len(), 1);
        let finding = finding_with_family(&findings, "unwrap");
        assert_eq!(finding.identity.callee.as_deref(), Some("unwrap"));
        assert_eq!(finding.identity.receiver_fingerprint, None);
        assert_eq!(finding.identity.container, None);
        assert_eq!(finding.identity.module, None);
    }

    fn line_context<'a>(
        container: &'a Option<String>,
        module_stack: &'a [String],
    ) -> LineContext<'a> {
        LineContext {
            path: Path::new("src/lib.rs"),
            line: "    builder.step().unwrap(); std::panic!(\"bad\");",
            line_no: 42,
            container,
            module_stack,
        }
    }

    fn panic_method(
        column: u32,
        kind: PanicMethodKind,
        receiver_fingerprint: Option<&str>,
    ) -> PanicMethodCall {
        PanicMethodCall {
            kind,
            column,
            receiver_fingerprint: receiver_fingerprint.map(str::to_string),
        }
    }

    fn panic_macro(column: u32, kind: PanicMacroKind, macro_path: &str) -> PanicMacroInvocation {
        PanicMacroInvocation {
            kind,
            column,
            macro_path: macro_path.to_string(),
        }
    }

    fn finding_with_family<'a>(findings: &'a [Finding], family: &str) -> &'a Finding {
        findings
            .iter()
            .find(|finding| finding.family.as_deref() == Some(family))
            .unwrap_or_else(|| std::panic::panic_any(format!("expected {family} finding")))
    }
}
