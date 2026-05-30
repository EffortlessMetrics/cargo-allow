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
mod tests {
    use super::{
        EVIDENCE_KIND_SPECS, EvidenceKind, EvidenceReference, canonical_evidence_prefixes,
        local_file_evidence_prefixes, recognized_evidence_prefixes, traceability_evidence_prefixes,
    };
    use std::path::PathBuf;

    #[test]
    fn parses_typed_references_with_trimmed_prefix_and_value() {
        let reference = EvidenceReference::parse(" doc: docs/safety.md ")
            .unwrap_or_else(|| std::panic::panic_any("evidence reference should parse"));

        assert_eq!(reference.raw, " doc: docs/safety.md ");
        assert_eq!(reference.prefix, "doc");
        assert_eq!(reference.kind, EvidenceKind::Doc);
        assert_eq!(reference.value, PathBuf::from("docs/safety.md"));
    }

    #[test]
    fn leaves_untyped_evidence_unparsed() {
        assert_eq!(EvidenceReference::parse("TODO add reviewed evidence"), None);
    }

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
    fn locks_evidence_prefix_classification_contract() {
        let actual = EVIDENCE_KIND_SPECS
            .iter()
            .flat_map(|spec| {
                spec.prefixes
                    .iter()
                    .map(|prefix| (*prefix, spec.canonical_prefix, spec.kind, spec.local_file))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                ("doc", "doc", EvidenceKind::Doc, true),
                ("spec", "spec", EvidenceKind::Spec, true),
                ("adr", "adr", EvidenceKind::Adr, true),
                ("ripr", "ripr", EvidenceKind::Ripr, true),
                (
                    "unsafe-review",
                    "unsafe-review",
                    EvidenceKind::UnsafeReview,
                    true,
                ),
                (
                    "unsafe_review",
                    "unsafe-review",
                    EvidenceKind::UnsafeReview,
                    true,
                ),
                ("coverage", "coverage", EvidenceKind::Coverage, true),
                ("test", "test", EvidenceKind::Test, false),
                ("cargo", "cargo", EvidenceKind::Cargo, false),
                ("issue", "issue", EvidenceKind::Issue, false),
                ("pr", "pr", EvidenceKind::Pr, false),
                (
                    "legacy-policy",
                    "legacy-policy",
                    EvidenceKind::LegacyPolicy,
                    false,
                ),
                (
                    "legacy_policy",
                    "legacy-policy",
                    EvidenceKind::LegacyPolicy,
                    false,
                ),
            ],
            "evidence prefix classification is a source-exception contract"
        );
    }

    #[test]
    fn exposes_parser_owned_evidence_prefix_vocabulary() {
        assert_eq!(
            canonical_evidence_prefixes().collect::<Vec<_>>(),
            vec![
                "doc",
                "spec",
                "adr",
                "ripr",
                "unsafe-review",
                "coverage",
                "test",
                "cargo",
                "issue",
                "pr",
                "legacy-policy",
            ],
            "canonical prefixes should stay in user guidance order"
        );
        assert_eq!(
            recognized_evidence_prefixes().collect::<Vec<_>>(),
            vec![
                "doc",
                "spec",
                "adr",
                "ripr",
                "unsafe-review",
                "unsafe_review",
                "coverage",
                "test",
                "cargo",
                "issue",
                "pr",
                "legacy-policy",
                "legacy_policy",
            ],
            "recognized prefixes include compatibility aliases"
        );
        assert_eq!(
            local_file_evidence_prefixes().collect::<Vec<_>>(),
            vec![
                "doc",
                "spec",
                "adr",
                "ripr",
                "unsafe-review",
                "unsafe_review",
                "coverage",
            ]
        );
        assert_eq!(
            traceability_evidence_prefixes().collect::<Vec<_>>(),
            vec![
                "test",
                "cargo",
                "issue",
                "pr",
                "legacy-policy",
                "legacy_policy",
            ]
        );
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
