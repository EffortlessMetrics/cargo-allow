use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::{PanicMacroInvocation, PanicMethodCall};
use crate::text::receiver_before_method_column;

pub(crate) fn scan_panic_calls(
    context: LineContext<'_>,
    panic_methods: &[PanicMethodCall],
    panic_macros: &[PanicMacroInvocation],
    findings: &mut Vec<Finding>,
) {
    for method_call in panic_methods {
        let receiver = receiver_before_method_column(context.line, method_call.column);
        push_finding(
            context.site(method_call.column),
            FindingKind::Panic,
            method_call.kind.family(),
            "method_call",
            |id| {
                id.callee = Some(method_call.kind.family().to_string());
                if !receiver.is_empty() {
                    id.receiver_fingerprint = Some(receiver);
                }
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
            },
            findings,
        );
    }
}
