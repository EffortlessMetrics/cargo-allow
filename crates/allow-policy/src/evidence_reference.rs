use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceKind {
    Test,
    Cargo,
    Ripr,
    UnsafeReview,
    Coverage,
    Doc,
    Spec,
    Adr,
    Issue,
    Pr,
    Unknown,
}

impl EvidenceKind {
    pub(crate) fn parse(prefix: &str) -> Self {
        match prefix {
            "test" => Self::Test,
            "cargo" => Self::Cargo,
            "ripr" => Self::Ripr,
            "unsafe-review" | "unsafe_review" => Self::UnsafeReview,
            "coverage" => Self::Coverage,
            "doc" => Self::Doc,
            "spec" => Self::Spec,
            "adr" => Self::Adr,
            "issue" => Self::Issue,
            "pr" => Self::Pr,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn is_local_file(self) -> bool {
        matches!(
            self,
            Self::Ripr | Self::UnsafeReview | Self::Coverage | Self::Doc | Self::Spec | Self::Adr
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceReference<'a> {
    pub(crate) raw: &'a str,
    pub(crate) prefix: &'a str,
    pub(crate) kind: EvidenceKind,
    pub(crate) value: PathBuf,
}

impl<'a> EvidenceReference<'a> {
    pub(crate) fn parse(raw: &'a str) -> Option<Self> {
        let (prefix, value) = raw.split_once(':')?;
        let value = value.trim();
        if value.is_empty() {
            return Some(Self {
                raw,
                prefix: prefix.trim(),
                kind: EvidenceKind::parse(prefix.trim()),
                value: PathBuf::new(),
            });
        }
        Some(Self {
            raw,
            prefix: prefix.trim(),
            kind: EvidenceKind::parse(prefix.trim()),
            value: PathBuf::from(value),
        })
    }
}
