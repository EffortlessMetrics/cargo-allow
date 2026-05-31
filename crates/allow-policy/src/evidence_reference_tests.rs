use std::path::PathBuf;

use super::{
    EVIDENCE_KIND_SPECS, EvidenceKind, EvidenceReference, canonical_evidence_prefixes,
    local_file_evidence_prefixes, recognized_evidence_prefixes, traceability_evidence_prefixes,
};

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
