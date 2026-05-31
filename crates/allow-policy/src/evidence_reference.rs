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
    LegacyPolicy,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
struct EvidenceKindSpec {
    kind: EvidenceKind,
    canonical_prefix: &'static str,
    prefixes: &'static [&'static str],
    local_file: bool,
}

const EVIDENCE_KIND_SPECS: &[EvidenceKindSpec] = &[
    EvidenceKindSpec {
        kind: EvidenceKind::Doc,
        canonical_prefix: "doc",
        prefixes: &["doc"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Spec,
        canonical_prefix: "spec",
        prefixes: &["spec"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Adr,
        canonical_prefix: "adr",
        prefixes: &["adr"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Ripr,
        canonical_prefix: "ripr",
        prefixes: &["ripr"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::UnsafeReview,
        canonical_prefix: "unsafe-review",
        prefixes: &["unsafe-review", "unsafe_review"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Coverage,
        canonical_prefix: "coverage",
        prefixes: &["coverage"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Test,
        canonical_prefix: "test",
        prefixes: &["test"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Cargo,
        canonical_prefix: "cargo",
        prefixes: &["cargo"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Issue,
        canonical_prefix: "issue",
        prefixes: &["issue"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Pr,
        canonical_prefix: "pr",
        prefixes: &["pr"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::LegacyPolicy,
        canonical_prefix: "legacy-policy",
        prefixes: &["legacy-policy", "legacy_policy"],
        local_file: false,
    },
];

/// Canonical evidence prefixes shown in user-facing guidance.
pub fn canonical_evidence_prefixes() -> impl Iterator<Item = &'static str> {
    EVIDENCE_KIND_SPECS.iter().map(|spec| spec.canonical_prefix)
}

/// All evidence prefixes recognized by the policy parser, including aliases.
pub fn recognized_evidence_prefixes() -> impl Iterator<Item = &'static str> {
    EVIDENCE_KIND_SPECS
        .iter()
        .flat_map(|spec| spec.prefixes.iter().copied())
}

/// Recognized evidence prefixes that must point to local source-tree files.
pub fn local_file_evidence_prefixes() -> impl Iterator<Item = &'static str> {
    EVIDENCE_KIND_SPECS
        .iter()
        .filter(|spec| spec.local_file)
        .flat_map(|spec| spec.prefixes.iter().copied())
}

/// Recognized evidence prefixes that cargo-allow treats as traceability only.
pub fn traceability_evidence_prefixes() -> impl Iterator<Item = &'static str> {
    EVIDENCE_KIND_SPECS
        .iter()
        .filter(|spec| !spec.local_file)
        .flat_map(|spec| spec.prefixes.iter().copied())
}

impl EvidenceKind {
    pub(crate) fn parse(prefix: &str) -> Self {
        EVIDENCE_KIND_SPECS
            .iter()
            .find(|spec| spec.prefixes.contains(&prefix))
            .map_or(Self::Unknown, |spec| spec.kind)
    }

    pub(crate) fn is_local_file(self) -> bool {
        EVIDENCE_KIND_SPECS
            .iter()
            .find(|spec| spec.kind == self)
            .is_some_and(|spec| spec.local_file)
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

#[cfg(test)]
#[path = "evidence_reference_tests.rs"]
mod tests;
