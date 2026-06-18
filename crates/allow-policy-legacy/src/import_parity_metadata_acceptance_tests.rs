//! Acceptance fixture matrix for import-parity owner/reason/evidence governance (#1717).
//!
//! Characterizes semantic-selector migration lanes in one harness: reviewed entries
//! preserve owner, reason, evidence, and legacy `covered_by`; weak or missing evidence
//! stays visible as debt rather than laundering into silent approval.

use super::*;
use crate::migration_lane_descriptors::{CompatKind, legacy_lane_descriptor};
use allow_core::{AllowConfig, AllowEntry, CargoAllowResult};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn import_parity_metadata_acceptance_matrix_characterizes_governance_round_trip() {
    for case in metadata_acceptance_cases() {
        let policy_path =
            stage_metadata_acceptance_fixture(case.fixture_file, case.legacy_filename);
        let cfg = case
            .loader
            .load(&policy_path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("{} migration: {err}", case.lane)));

        assert_eq!(cfg.policy, "cargo-allow", "{} canonical policy", case.lane);

        let entry = find_acceptance_entry(&cfg, case.entry_id, case.family).unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "{} should migrate entry {} with family {:?}; got ids {:?}",
                case.lane,
                case.entry_id,
                case.family,
                cfg.allow.iter().map(|entry| &entry.id).collect::<Vec<_>>()
            ))
        });

        assert_governance_metadata(case.lane, entry, &case);
        assert_semantic_selector_identity(case.lane, entry, &case);

        if case.expect_weak_evidence {
            assert_weak_evidence_visible(case.lane, entry, &case);
        } else if !case.expect_baseline_debt {
            assert!(
                allow_policy::weak_evidence_reference_count(Path::new("."), &cfg) == 0,
                "{} reviewed entry should not emit weak-evidence TODO markers",
                case.lane
            );
        }
    }
}

#[test]
fn import_parity_metadata_acceptance_batch_preserves_governance_across_semantic_lanes() {
    let dir = crate::test_support::fixture_dir();
    let cases = [
        (
            "no-panic-allowlist-semantic-selectors-covered-by.toml",
            "no-panic-allowlist.toml",
        ),
        (
            "lint-exception-semantic-selectors-covered-by.toml",
            "clippy-exceptions.toml",
        ),
        ("unsafe.toml", "unsafe-allowlist.toml"),
    ];

    for (fixture_file, legacy_filename) in cases {
        let source = migration_fixture_path(fixture_file);
        let text = fs::read_to_string(&source).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read batch fixture {fixture_file}: {err}"))
        });
        fs::write(dir.join(legacy_filename), text).unwrap_or_else(|err| {
            std::panic::panic_any(format!("write batch fixture {legacy_filename}: {err}"))
        });
    }

    let batch = import_legacy_policy_dir(&dir, None).unwrap_or_else(|err| {
        std::panic::panic_any(format!("semantic governance batch import: {err}"))
    });

    assert_eq!(
        batch.families.len(),
        3,
        "batch import should keep panic, lint, and unsafe semantic-selector families separate"
    );
    assert_eq!(batch.families[0].compat_kind, CompatKind::NoPanicAllowlist);
    assert_eq!(batch.families[1].compat_kind, CompatKind::LintException);
    assert_eq!(batch.families[2].compat_kind, CompatKind::Unsafe);

    for (entry_id, expected_owner, expected_evidence) in [
        (
            "fixture-semantic-covered",
            "parser",
            "test:semantic_selector_covered_by_round_trip",
        ),
        (
            "fixture-clippy-covered",
            "lint",
            "test:lint_semantic_selector_covered_by_round_trip",
        ),
        (
            "fixture-unsafe",
            "runtime",
            "unsafe-review:docs/evidence/unsafe/read.json",
        ),
    ] {
        let entry = batch
            .config
            .allow
            .iter()
            .find(|entry| entry.id == entry_id)
            .unwrap_or_else(|| std::panic::panic_any(format!("batch missing entry {entry_id}")));
        assert_eq!(entry.owner, expected_owner, "{entry_id} owner");
        assert!(
            entry.evidence.iter().any(|item| item == expected_evidence),
            "{entry_id} should preserve evidence `{expected_evidence}` in {:?}",
            entry.evidence
        );
        assert!(
            entry.selector.has_structural_identity(),
            "{entry_id} should keep semantic selector identity in batch import"
        );
    }
}

#[derive(Clone, Copy)]
enum AcceptanceLoader {
    Canonical,
    NoPanicAllowlist,
    PanicBaseline,
    Clippy,
    Unsafe,
}

impl AcceptanceLoader {
    fn load(self, path: &Path) -> CargoAllowResult<AllowConfig> {
        match self {
            Self::Canonical => load_legacy_or_canonical(path),
            Self::NoPanicAllowlist => load_no_panic_allowlist_compat_config(path),
            Self::PanicBaseline => load_no_panic_baseline_compat_config(path),
            Self::Clippy => load_clippy_exceptions_compat_config(path),
            Self::Unsafe => load_unsafe_allowlist_compat_config(path),
        }
    }
}

struct MetadataAcceptanceCase {
    lane: &'static str,
    fixture_file: &'static str,
    legacy_filename: &'static str,
    entry_id: &'static str,
    family: Option<&'static str>,
    expected_owner: &'static str,
    expected_reason: &'static str,
    expected_classification: &'static str,
    expected_evidence: &'static [&'static str],
    expected_ast_kind: Option<&'static str>,
    expected_container: Option<&'static str>,
    expected_receiver: Option<&'static str>,
    expected_target: Option<&'static str>,
    expect_weak_evidence: bool,
    expect_baseline_debt: bool,
    loader: AcceptanceLoader,
}

fn metadata_acceptance_cases() -> Vec<MetadataAcceptanceCase> {
    let no_panic = legacy_lane_descriptor(CompatKind::NoPanicAllowlist)
        .unwrap_or_else(|| std::panic::panic_any("no-panic descriptor missing"));
    let panic_baseline = legacy_lane_descriptor(CompatKind::PanicBaseline)
        .unwrap_or_else(|| std::panic::panic_any("panic baseline descriptor missing"));
    let lint = legacy_lane_descriptor(CompatKind::LintException)
        .unwrap_or_else(|| std::panic::panic_any("lint descriptor missing"));
    let unsafe_lane = legacy_lane_descriptor(CompatKind::Unsafe)
        .unwrap_or_else(|| std::panic::panic_any("unsafe descriptor missing"));

    vec![
        MetadataAcceptanceCase {
            lane: "no-panic semantic evidence",
            fixture_file: "no-panic-allowlist-semantic-selectors.toml",
            legacy_filename: no_panic.legacy_filename,
            entry_id: "fixture-semantic-unwrap",
            family: Some("unwrap"),
            expected_owner: "parser",
            expected_reason: "Semantic selector pins unwrap on optional after validation.",
            expected_classification: "reviewed_panic_exception",
            expected_evidence: &["test:semantic_selector_round_trip"],
            expected_ast_kind: Some("method_call"),
            expected_container: Some("load"),
            expected_receiver: Some("optional_value"),
            expected_target: None,
            expect_weak_evidence: false,
            expect_baseline_debt: false,
            loader: AcceptanceLoader::Canonical,
        },
        MetadataAcceptanceCase {
            lane: "no-panic semantic covered_by",
            fixture_file: "no-panic-allowlist-semantic-selectors-covered-by.toml",
            legacy_filename: no_panic.legacy_filename,
            entry_id: "fixture-semantic-covered",
            family: Some("unwrap"),
            expected_owner: "parser",
            expected_reason: "Semantic selector with legacy covered_by governance metadata.",
            expected_classification: "reviewed_panic_exception",
            expected_evidence: &["test:semantic_selector_covered_by_round_trip"],
            expected_ast_kind: Some("method_call"),
            expected_container: Some("load"),
            expected_receiver: Some("optional_value"),
            expected_target: None,
            expect_weak_evidence: false,
            expect_baseline_debt: false,
            loader: AcceptanceLoader::NoPanicAllowlist,
        },
        MetadataAcceptanceCase {
            lane: "lint semantic evidence",
            fixture_file: "lint-exception.toml",
            legacy_filename: lint.legacy_filename,
            entry_id: "fixture-clippy",
            family: Some("expect_attribute"),
            expected_owner: "lint",
            expected_reason: "Parser validates optional value before unwrap.",
            expected_classification: "reviewed_lint_exception",
            expected_evidence: &["test:parser_validates_optional_value"],
            expected_ast_kind: Some("attribute"),
            expected_container: None,
            expected_receiver: None,
            expected_target: Some("policy:fixture-clippy"),
            expect_weak_evidence: false,
            expect_baseline_debt: false,
            loader: AcceptanceLoader::Clippy,
        },
        MetadataAcceptanceCase {
            lane: "lint semantic covered_by",
            fixture_file: "lint-exception-semantic-selectors-covered-by.toml",
            legacy_filename: lint.legacy_filename,
            entry_id: "fixture-clippy-covered",
            family: Some("expect_attribute"),
            expected_owner: "lint",
            expected_reason: "Lint semantic selector with legacy covered_by governance metadata.",
            expected_classification: "reviewed_lint_exception",
            expected_evidence: &["test:lint_semantic_selector_covered_by_round_trip"],
            expected_ast_kind: Some("attribute"),
            expected_container: None,
            expected_receiver: None,
            expected_target: Some("policy:fixture-clippy-covered"),
            expect_weak_evidence: false,
            expect_baseline_debt: false,
            loader: AcceptanceLoader::Clippy,
        },
        MetadataAcceptanceCase {
            lane: "unsafe semantic evidence",
            fixture_file: "unsafe.toml",
            legacy_filename: unsafe_lane.legacy_filename,
            entry_id: "fixture-unsafe",
            family: Some("unsafe_block"),
            expected_owner: "runtime",
            expected_reason: "Caller validates pointer before read.",
            expected_classification: "reviewed_unsafe_boundary",
            expected_evidence: &["unsafe-review:docs/evidence/unsafe/read.json"],
            expected_ast_kind: Some("unsafe_block"),
            expected_container: Some("read"),
            expected_receiver: None,
            expected_target: None,
            expect_weak_evidence: false,
            expect_baseline_debt: false,
            loader: AcceptanceLoader::Unsafe,
        },
        MetadataAcceptanceCase {
            lane: "unsafe semantic missing evidence",
            fixture_file: "unsafe-no-evidence.toml",
            legacy_filename: unsafe_lane.legacy_filename,
            entry_id: "fixture-unsafe-no-evidence",
            family: Some("unsafe_fn"),
            expected_owner: "runtime",
            expected_reason: "Caller validates FFI boundary before call.",
            expected_classification: "reviewed_unsafe_boundary",
            expected_evidence: &[],
            expected_ast_kind: Some("unsafe_fn"),
            expected_container: Some("ffi_call"),
            expected_receiver: None,
            expected_target: None,
            expect_weak_evidence: true,
            expect_baseline_debt: false,
            loader: AcceptanceLoader::Unsafe,
        },
        MetadataAcceptanceCase {
            lane: "panic baseline missing evidence",
            fixture_file: "panic-baseline-no-evidence.toml",
            legacy_filename: panic_baseline.legacy_filename,
            entry_id: "panic-baseline-0001",
            family: Some("unwrap"),
            expected_owner: "parser",
            expected_reason: "Counted unwrap baseline without evidence.",
            expected_classification: "baseline_debt",
            expected_evidence: &["baseline_count:3"],
            expected_ast_kind: None,
            expected_container: None,
            expected_receiver: None,
            expected_target: None,
            expect_weak_evidence: false,
            expect_baseline_debt: true,
            loader: AcceptanceLoader::PanicBaseline,
        },
    ]
}

fn assert_governance_metadata(case_lane: &str, entry: &AllowEntry, case: &MetadataAcceptanceCase) {
    assert_eq!(entry.owner, case.expected_owner, "{case_lane} owner");
    assert!(
        entry.reason.contains(case.expected_reason),
        "{case_lane} reason should contain `{}` in `{}`",
        case.expected_reason,
        entry.reason
    );
    assert_eq!(
        entry.classification, case.expected_classification,
        "{case_lane} classification"
    );

    for value in case.expected_evidence {
        assert!(
            entry.evidence.iter().any(|item| item == value),
            "{case_lane} should preserve evidence `{value}` in {:?}",
            entry.evidence
        );
    }

    if case.expect_baseline_debt {
        assert_eq!(
            entry.classification, "baseline_debt",
            "{case_lane} should keep baseline_debt classification instead of laundering into reviewed approval"
        );
        let has_traceability = entry.evidence.iter().any(|item| {
            item.starts_with("legacy_policy:")
                || item.starts_with("legacy-policy:")
                || item.starts_with("baseline_count:")
        }) || entry
            .links
            .iter()
            .any(|item| item.starts_with("legacy-policy:"));
        assert!(
            has_traceability,
            "{case_lane} should keep visible baseline_debt traceability in evidence {:?} or links {:?}",
            entry.evidence, entry.links
        );
    }
}

fn assert_semantic_selector_identity(
    case_lane: &str,
    entry: &AllowEntry,
    case: &MetadataAcceptanceCase,
) {
    if case.expected_ast_kind.is_none()
        && case.expected_container.is_none()
        && case.expected_receiver.is_none()
        && case.expected_target.is_none()
    {
        return;
    }

    assert!(
        entry.selector.has_structural_identity(),
        "{case_lane} should preserve semantic selector identity"
    );

    if let Some(ast_kind) = case.expected_ast_kind {
        assert_eq!(
            entry.selector.ast_kind.as_deref(),
            Some(ast_kind),
            "{case_lane} ast_kind"
        );
    }
    if let Some(container) = case.expected_container {
        assert_eq!(
            entry.selector.container.as_deref(),
            Some(container),
            "{case_lane} container"
        );
    }
    if let Some(receiver) = case.expected_receiver {
        assert_eq!(
            entry.selector.receiver_fingerprint.as_deref(),
            Some(receiver),
            "{case_lane} receiver_fingerprint"
        );
    }
    if let Some(target) = case.expected_target {
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some(target),
            "{case_lane} target_fingerprint"
        );
    }
}

fn assert_weak_evidence_visible(
    case_lane: &str,
    entry: &AllowEntry,
    case: &MetadataAcceptanceCase,
) {
    assert!(
        entry
            .evidence
            .iter()
            .any(|item| item.contains("TODO: add unsafe-review")),
        "{case_lane} should surface missing unsafe evidence as visible TODO debt in {:?}",
        entry.evidence
    );
    assert!(
        entry
            .evidence
            .iter()
            .any(|item| item.starts_with("legacy-policy:")),
        "{case_lane} should keep legacy-policy traceability alongside TODO debt"
    );
    assert_eq!(
        entry.owner, case.expected_owner,
        "{case_lane} should still preserve owner while evidence is weak"
    );
    assert!(
        entry.reason.contains(case.expected_reason),
        "{case_lane} should still preserve reason while evidence is weak"
    );
}

fn migration_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migration")
}

fn migration_fixture_path(fixture_file: &str) -> PathBuf {
    migration_fixture_root().join(fixture_file)
}

fn stage_metadata_acceptance_fixture(fixture_file: &str, legacy_filename: &str) -> PathBuf {
    let dir = crate::test_support::fixture_dir();
    let source = migration_fixture_path(fixture_file);
    let text = fs::read_to_string(&source).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "read metadata acceptance fixture {}: {err}",
            source.display()
        ))
    });
    let path = dir.join(legacy_filename);
    fs::write(&path, text).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "write metadata acceptance fixture {}: {err}",
            path.display()
        ))
    });
    path
}

fn find_acceptance_entry<'a>(
    cfg: &'a AllowConfig,
    entry_id: &str,
    family: Option<&str>,
) -> Option<&'a AllowEntry> {
    cfg.allow.iter().find(|entry| {
        entry.id == entry_id && family.is_none_or(|family| entry.family.as_deref() == Some(family))
    })
}
