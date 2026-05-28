use allow_core::{Finding, FindingKind};
use std::path::Path;

use crate::finding_builder::{FindingSite, push_finding};
use crate::syntax_kinds::{PanicMacroInvocation, PanicMethodCall};
use crate::text::receiver_before_method_column;

pub(crate) fn scan_panic_calls(
    context: PanicLineContext<'_>,
    panic_methods: &[PanicMethodCall],
    panic_macros: &[PanicMacroInvocation],
    findings: &mut Vec<Finding>,
) {
    for method_call in panic_methods {
        let receiver = receiver_before_method_column(context.line, method_call.column);
        push_finding(
            FindingSite {
                path: context.path,
                line: context.line,
                line_no: context.line_no,
                column: method_call.column,
                container: context.container,
                module_stack: context.module_stack,
            },
            FindingKind::Panic,
            method_call.kind.family(),
            "method_call",
            |id| {
                id.callee = Some(method_call.kind.family().to_string());
                id.receiver_fingerprint = Some(receiver);
            },
            findings,
        );
    }

    for macro_invocation in panic_macros {
        push_finding(
            FindingSite {
                path: context.path,
                line: context.line,
                line_no: context.line_no,
                column: macro_invocation.column,
                container: context.container,
                module_stack: context.module_stack,
            },
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

pub(crate) struct PanicLineContext<'a> {
    pub(crate) path: &'a Path,
    pub(crate) line: &'a str,
    pub(crate) line_no: u32,
    pub(crate) container: &'a Option<String>,
    pub(crate) module_stack: &'a [String],
}
