use crate::syntax_kinds::{
    IndexExpression, LintAttribute, PanicMacroInvocation, PanicMethodCall, UnsafeSyntaxConstruct,
};

pub(crate) struct SyntaxLineFacts<'a> {
    pub(crate) lint_attributes: &'a [LintAttribute],
    pub(crate) panic_macros: &'a [PanicMacroInvocation],
    pub(crate) panic_methods: &'a [PanicMethodCall],
    pub(crate) index_expressions: &'a [IndexExpression],
    pub(crate) unsafe_constructs: &'a [UnsafeSyntaxConstruct],
    pub(crate) unsafe_attribute_columns: &'a [u32],
    pub(crate) safety_comment_nearby: bool,
}
