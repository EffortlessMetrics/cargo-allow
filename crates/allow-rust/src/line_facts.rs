use crate::syntax_kinds::{
    IndexExpression, LintAttribute, PanicMacroInvocation, PanicMethodCall, UnsafeAttribute,
    UnsafeSyntaxConstruct,
};

pub(crate) struct SyntaxLineFacts<'a> {
    pub(crate) lint_attributes: &'a [LintAttribute],
    pub(crate) panic_macros: &'a [PanicMacroInvocation],
    pub(crate) panic_methods: &'a [PanicMethodCall],
    pub(crate) index_expressions: &'a [IndexExpression],
    pub(crate) unsafe_constructs: &'a [UnsafeSyntaxConstruct],
    pub(crate) unsafe_attributes: &'a [UnsafeAttribute],
    pub(crate) safety_comment_nearby: bool,
}
