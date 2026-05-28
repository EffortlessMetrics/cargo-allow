use crate::syntax_kinds::{
    LintAttributeKind, PanicMacroInvocation, PanicMethodCall, UnsafeSyntaxConstruct,
};

pub(crate) struct SyntaxLineFacts<'a> {
    pub(crate) lint_attributes: &'a [LintAttributeKind],
    pub(crate) panic_macros: &'a [PanicMacroInvocation],
    pub(crate) panic_methods: &'a [PanicMethodCall],
    pub(crate) index_column: Option<u32>,
    pub(crate) unsafe_constructs: &'a [UnsafeSyntaxConstruct],
    pub(crate) unsafe_attribute: bool,
    pub(crate) safety_comment_nearby: bool,
}
