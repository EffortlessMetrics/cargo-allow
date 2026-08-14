use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LintAttributeKind {
    Allow,
    Expect,
    Deny,
    Forbid,
    Warn,
}

impl LintAttributeKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Expect => "expect",
            Self::Deny => "deny",
            Self::Forbid => "forbid",
            Self::Warn => "warn",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LintAttribute {
    pub(crate) kind: LintAttributeKind,
    pub(crate) text: String,
    pub(crate) column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnsafeSyntaxKind {
    Fn,
    Impl,
    Trait,
    ExternBlock,
    Block,
    Const,
    Static,
}

impl UnsafeSyntaxKind {
    pub(crate) fn family(self) -> &'static str {
        match self {
            Self::Fn => "unsafe_fn",
            Self::Impl => "unsafe_impl",
            Self::Trait => "unsafe_trait",
            Self::ExternBlock => "unsafe_extern_block",
            Self::Block => "unsafe_block",
            Self::Const => "unsafe_const",
            Self::Static => "unsafe_static",
        }
    }

    pub(crate) fn ast_kind(self) -> &'static str {
        self.family()
    }

    pub(crate) fn priority(self) -> u8 {
        match self {
            Self::Fn => 0,
            Self::Impl => 1,
            Self::Trait => 2,
            Self::ExternBlock => 3,
            Self::Block => 4,
            Self::Const => 5,
            Self::Static => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsafeSyntaxConstruct {
    pub(crate) kind: UnsafeSyntaxKind,
    pub(crate) column: u32,
    pub(crate) symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsafeAttribute {
    pub(crate) column: u32,
    pub(crate) symbol: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanicMacroKind {
    Panic,
    Todo,
    Unimplemented,
    Unreachable,
    Assert,
    AssertEq,
    AssertNe,
}

impl PanicMacroKind {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "panic" => Some(Self::Panic),
            "todo" => Some(Self::Todo),
            "unimplemented" => Some(Self::Unimplemented),
            "unreachable" => Some(Self::Unreachable),
            "assert" => Some(Self::Assert),
            "assert_eq" => Some(Self::AssertEq),
            "assert_ne" => Some(Self::AssertNe),
            _ => None,
        }
    }

    pub(crate) fn macro_name(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Unreachable => "unreachable",
            Self::Assert => "assert",
            Self::AssertEq => "assert_eq",
            Self::AssertNe => "assert_ne",
        }
    }

    pub(crate) fn family(self) -> &'static str {
        match self {
            Self::Panic => "panic_macro",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Unreachable => "unreachable",
            Self::Assert => "assert",
            Self::AssertEq => "assert_eq",
            Self::AssertNe => "assert_ne",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PanicMacroInvocation {
    pub(crate) kind: PanicMacroKind,
    pub(crate) column: u32,
    pub(crate) macro_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanicMethodKind {
    Unwrap,
    Expect,
}

impl PanicMethodKind {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "unwrap" => Some(Self::Unwrap),
            "expect" => Some(Self::Expect),
            _ => None,
        }
    }

    pub(crate) fn family(self) -> &'static str {
        match self {
            Self::Unwrap => "unwrap",
            Self::Expect => "expect",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PanicMethodCall {
    pub(crate) kind: PanicMethodKind,
    pub(crate) column: u32,
    pub(crate) receiver_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexExpression {
    pub(crate) column: u32,
    pub(crate) symbol: String,
    pub(crate) receiver_fingerprint: Option<String>,
    pub(crate) target_fingerprint: Option<String>,
    pub(crate) is_slice: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RustLineScope {
    pub(crate) container: Option<String>,
    pub(crate) module_path: Vec<String>,
    pub(crate) span_len: u32,
}

#[derive(Default)]
pub(crate) struct RustSyntaxFacts {
    pub(crate) index_expressions: BTreeMap<u32, Vec<IndexExpression>>,
    pub(crate) lint_attributes: BTreeMap<u32, Vec<LintAttribute>>,
    pub(crate) panic_macros: BTreeMap<u32, Vec<PanicMacroInvocation>>,
    pub(crate) panic_methods: BTreeMap<u32, Vec<PanicMethodCall>>,
    pub(crate) scopes: BTreeMap<u32, RustLineScope>,
    pub(crate) safety_comment_lines: BTreeSet<u32>,
    pub(crate) unsafe_constructs: BTreeMap<u32, Vec<UnsafeSyntaxConstruct>>,
    pub(crate) unsafe_attributes: BTreeMap<u32, Vec<UnsafeAttribute>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_syntax_kind_priority_match_arm_discriminator() {
        let priorities = [
            (UnsafeSyntaxKind::Fn, 0),
            (UnsafeSyntaxKind::Impl, 1),
            (UnsafeSyntaxKind::Trait, 2),
            (UnsafeSyntaxKind::ExternBlock, 3),
            (UnsafeSyntaxKind::Block, 4),
        ];

        for (kind, expected) in priorities {
            assert_eq!(kind.priority(), expected);
        }
    }

    #[test]
    fn unsafe_syntax_kind_family_and_ast_kind_discriminators() {
        let families = [
            (UnsafeSyntaxKind::Fn, "unsafe_fn"),
            (UnsafeSyntaxKind::Impl, "unsafe_impl"),
            (UnsafeSyntaxKind::Trait, "unsafe_trait"),
            (UnsafeSyntaxKind::ExternBlock, "unsafe_extern_block"),
            (UnsafeSyntaxKind::Block, "unsafe_block"),
        ];

        for (kind, expected) in families {
            assert_eq!(kind.family(), expected);
            assert_eq!(kind.ast_kind(), expected);
        }
    }

    #[test]
    fn panic_macro_kind_from_name_match_arm_discriminator() {
        let accepted = [
            ("panic", PanicMacroKind::Panic),
            ("todo", PanicMacroKind::Todo),
            ("unimplemented", PanicMacroKind::Unimplemented),
            ("unreachable", PanicMacroKind::Unreachable),
            ("assert", PanicMacroKind::Assert),
            ("assert_eq", PanicMacroKind::AssertEq),
            ("assert_ne", PanicMacroKind::AssertNe),
        ];

        for (name, expected) in accepted {
            assert_eq!(PanicMacroKind::from_name(name), Some(expected));
        }
        assert_eq!(PanicMacroKind::from_name("debug_assert"), None);
        assert_eq!(PanicMacroKind::from_name("std::panic"), None);
    }

    #[test]
    fn panic_macro_kind_macro_name_match_arm_discriminator() {
        let names = [
            (PanicMacroKind::Panic, "panic"),
            (PanicMacroKind::Todo, "todo"),
            (PanicMacroKind::Unimplemented, "unimplemented"),
            (PanicMacroKind::Unreachable, "unreachable"),
        ];

        for (kind, expected) in names {
            assert_eq!(kind.macro_name(), expected);
        }
    }

    #[test]
    fn panic_macro_kind_family_match_arm_discriminator() {
        let families = [
            (PanicMacroKind::Panic, "panic_macro"),
            (PanicMacroKind::Todo, "todo"),
            (PanicMacroKind::Unimplemented, "unimplemented"),
            (PanicMacroKind::Unreachable, "unreachable"),
        ];

        for (kind, expected) in families {
            assert_eq!(kind.family(), expected);
        }
    }

    #[test]
    fn panic_method_kind_from_name_match_arm_discriminator() {
        let accepted = [
            ("unwrap", PanicMethodKind::Unwrap),
            ("expect", PanicMethodKind::Expect),
        ];

        for (name, expected) in accepted {
            assert_eq!(PanicMethodKind::from_name(name), Some(expected));
        }
        assert_eq!(PanicMethodKind::from_name("unwrap_or"), None);
        assert_eq!(PanicMethodKind::from_name("expect_err"), None);
    }

    #[test]
    fn panic_method_kind_family_match_arm_discriminator() {
        let families = [
            (PanicMethodKind::Unwrap, "unwrap"),
            (PanicMethodKind::Expect, "expect"),
        ];

        for (kind, expected) in families {
            assert_eq!(kind.family(), expected);
        }
    }
}
