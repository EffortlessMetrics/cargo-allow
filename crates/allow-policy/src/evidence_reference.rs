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
    prefixes: &'static [&'static str],
    local_file: bool,
}

const EVIDENCE_KIND_SPECS: &[EvidenceKindSpec] = &[
    EvidenceKindSpec {
        kind: EvidenceKind::Test,
        prefixes: &["test"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Cargo,
        prefixes: &["cargo"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Ripr,
        prefixes: &["ripr"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::UnsafeReview,
        prefixes: &["unsafe-review", "unsafe_review"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Coverage,
        prefixes: &["coverage"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Doc,
        prefixes: &["doc"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Spec,
        prefixes: &["spec"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Adr,
        prefixes: &["adr"],
        local_file: true,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Issue,
        prefixes: &["issue"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::Pr,
        prefixes: &["pr"],
        local_file: false,
    },
    EvidenceKindSpec {
        kind: EvidenceKind::LegacyPolicy,
        prefixes: &["legacy-policy", "legacy_policy"],
        local_file: false,
    },
];

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
mod tests {
    use super::EvidenceKind;

    #[test]
    fn parses_evidence_kind_aliases_from_specs() {
        assert_eq!(
            EvidenceKind::parse("unsafe-review"),
            EvidenceKind::UnsafeReview
        );
        assert_eq!(
            EvidenceKind::parse("unsafe_review"),
            EvidenceKind::UnsafeReview
        );
        assert_eq!(
            EvidenceKind::parse("legacy-policy"),
            EvidenceKind::LegacyPolicy
        );
        assert_eq!(
            EvidenceKind::parse("legacy_policy"),
            EvidenceKind::LegacyPolicy
        );
        assert_eq!(EvidenceKind::parse("unknown"), EvidenceKind::Unknown);
    }

    #[test]
    fn classifies_local_file_evidence_from_specs() {
        assert!(EvidenceKind::Doc.is_local_file());
        assert!(EvidenceKind::Spec.is_local_file());
        assert!(EvidenceKind::Adr.is_local_file());
        assert!(EvidenceKind::Ripr.is_local_file());
        assert!(EvidenceKind::UnsafeReview.is_local_file());
        assert!(EvidenceKind::Coverage.is_local_file());
        assert!(!EvidenceKind::Test.is_local_file());
        assert!(!EvidenceKind::Cargo.is_local_file());
        assert!(!EvidenceKind::Issue.is_local_file());
        assert!(!EvidenceKind::Pr.is_local_file());
        assert!(!EvidenceKind::LegacyPolicy.is_local_file());
        assert!(!EvidenceKind::Unknown.is_local_file());
    }
}
