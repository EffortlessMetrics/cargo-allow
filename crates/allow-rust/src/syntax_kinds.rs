use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LintAttributeKind {
    Allow,
    Expect,
}

impl LintAttributeKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Expect => "expect",
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
}

impl UnsafeSyntaxKind {
    pub(crate) fn family(self) -> &'static str {
        match self {
            Self::Fn => "unsafe_fn",
            Self::Impl => "unsafe_impl",
            Self::Trait => "unsafe_trait",
            Self::ExternBlock => "unsafe_extern_block",
            Self::Block => "unsafe_block",
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnsafeSyntaxConstruct {
    pub(crate) kind: UnsafeSyntaxKind,
    pub(crate) column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanicMacroKind {
    Panic,
    Todo,
    Unimplemented,
    Unreachable,
}

impl PanicMacroKind {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "panic" => Some(Self::Panic),
            "todo" => Some(Self::Todo),
            "unimplemented" => Some(Self::Unimplemented),
            "unreachable" => Some(Self::Unreachable),
            _ => None,
        }
    }

    pub(crate) fn macro_name(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Unreachable => "unreachable",
        }
    }

    pub(crate) fn family(self) -> &'static str {
        match self {
            Self::Panic => "panic_macro",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Unreachable => "unreachable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PanicMacroInvocation {
    pub(crate) kind: PanicMacroKind,
    pub(crate) column: u32,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RustLineScope {
    pub(crate) container: Option<String>,
    pub(crate) module_path: Vec<String>,
    pub(crate) span_len: u32,
}

#[derive(Default)]
pub(crate) struct RustSyntaxFacts {
    pub(crate) index_columns: BTreeMap<u32, u32>,
    pub(crate) lint_attributes: BTreeMap<u32, Vec<LintAttribute>>,
    pub(crate) panic_macros: BTreeMap<u32, Vec<PanicMacroInvocation>>,
    pub(crate) panic_methods: BTreeMap<u32, Vec<PanicMethodCall>>,
    pub(crate) scopes: BTreeMap<u32, RustLineScope>,
    pub(crate) unsafe_constructs: BTreeMap<u32, Vec<UnsafeSyntaxConstruct>>,
    pub(crate) unsafe_attribute_columns: BTreeMap<u32, Vec<u32>>,
}
