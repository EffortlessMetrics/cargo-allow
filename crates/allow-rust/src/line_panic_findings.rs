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
