use super::*;
use crate::migration_lane_descriptors::{
    CompatKind, LegacyLaneDescriptor, all_legacy_lane_descriptors,
};
use allow_core::{AllowConfig, AllowEntry, CargoAllowResult};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn migration_fixture_matrix_characterizes_supported_legacy_lanes() {
    for case in migration_lane_cases() {
        let policy_path = stage_migration_fixture(case.fixture_file, case.legacy_filename);
        let migrated = load_legacy_or_canonical(&policy_path);
        assert!(
            migrated.is_ok(),
            "{} ({}) should parse: {:?}",
            case.lane,
            case.fixture_file,
            migrated.as_ref().err()
        );
        let cfg = match migrated {
            Ok(cfg) => cfg,
            Err(_) => continue,
        };

        assert_eq!(cfg.policy, "cargo-allow", "{} canonical policy", case.lane);

        let entry = find_matrix_entry(&cfg, case.entry_id, case.family);
        assert!(
            entry.is_some(),
            "{} should migrate entry {} with family {:?}; got ids {:?}",
            case.lane,
            case.entry_id,
            case.family,
            cfg.allow.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
        let Some(entry) = entry else {
            continue;
        };

        assert_eq!(entry.owner, case.expected_owner, "{} owner", case.lane);
        assert!(
            entry.reason.contains(case.expected_reason),
            "{} reason should contain `{}` in `{}`",
            case.lane,
            case.expected_reason,
            entry.reason
        );

        if let Some(classification) = case.expected_classification {
            assert_eq!(
                entry.classification, classification,
                "{} classification",
                case.lane
            );
        }

        for value in case.expected_evidence {
            assert!(
                entry.evidence.iter().any(|item| item == value),
                "{} should preserve evidence `{value}` in {:?}",
                case.lane,
                entry.evidence
            );
        }

        for value in case.expected_links {
            assert!(
                entry.links.iter().any(|item| item == value),
                "{} should preserve link `{value}` in {:?}",
                case.lane,
                entry.links
            );
        }

        if let Some(created) = case.expected_created {
            assert_eq!(
                entry.lifecycle.created.as_deref(),
                Some(created),
                "{} created",
                case.lane
            );
        }
        if let Some(review_after) = case.expected_review_after {
            assert_eq!(
                entry.lifecycle.review_after.as_deref(),
                Some(review_after),
                "{} review_after",
                case.lane
            );
        }
        if let Some(expires) = case.expected_expires {
            assert_eq!(
                entry.lifecycle.expires.as_deref(),
                Some(expires),
                "{} expires",
                case.lane
            );
        }

        match case.occurrence_limit {
            OccurrenceLimitExpect::None => {
                assert_eq!(
                    entry.occurrence_limit, None,
                    "{} occurrence_limit",
                    case.lane
                );
            }
            OccurrenceLimitExpect::Some(limit) => {
                assert_eq!(
                    entry.occurrence_limit,
                    Some(limit),
                    "{} occurrence_limit",
                    case.lane
                );
            }
        }

        if case.expect_baseline_debt_markers {
            assert_baseline_debt_visible(case.lane, entry);
        }

        if case.lane == "unsafe missing evidence" {
            assert!(
                entry
                    .evidence
                    .iter()
                    .any(|item| item.contains("TODO: add unsafe-review")),
                "{} should surface missing unsafe evidence as visible TODO debt",
                case.lane
            );
        }

        if let Some(loader) = case.compat_loader {
            let compat = load_compat_config(loader, &policy_path);
            assert!(
                compat.is_ok(),
                "{} compat loader should succeed: {:?}",
                case.lane,
                compat.as_ref().err()
            );
        }
    }
}

#[test]
fn migration_fixture_matrix_rerun_is_deterministic_for_primary_lanes() {
    for descriptor in all_legacy_lane_descriptors() {
        let lane = descriptor.compat_kind_id();
        let policy_path =
            stage_migration_fixture(descriptor.primary_fixture_file, descriptor.legacy_filename);
        let first = load_legacy_or_canonical(&policy_path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("{lane} first migration: {err}")));
        let second = load_legacy_or_canonical(&policy_path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("{lane} second migration: {err}")));
        assert_eq!(
            migration_fingerprint(&first),
            migration_fingerprint(&second),
            "{lane} migration should be deterministic across reruns"
        );
    }
}

#[test]
fn migration_fixture_matrix_multi_family_batch_preserves_lane_metadata() {
    use crate::migration_lane_descriptors::CompatKind;
    use allow_core::FindingKind;

    let dir = crate::test_support::fixture_dir();
    let panic_allowlist = migration_fixture_path("no-panic-allowlist.toml");
    let panic_baseline = migration_fixture_path("panic-baseline.toml");
    let lint_exception = migration_fixture_path("lint-exception.toml");

    for (source, legacy_filename) in [
        (panic_allowlist, "no-panic-allowlist.toml"),
        (panic_baseline, "no-panic-baseline.toml"),
        (lint_exception, "clippy-exceptions.toml"),
    ] {
        let text = fs::read_to_string(&source).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "read migration fixture {}: {err}",
                source.display()
            ))
        });
        fs::write(dir.join(legacy_filename), text).unwrap_or_else(|err| {
            std::panic::panic_any(format!("write policy dir fixture {legacy_filename}: {err}"))
        });
    }

    let batch = import_legacy_policy_dir(&dir, None).unwrap_or_else(|err| {
        std::panic::panic_any(format!("multi-family policy directory import: {err}"))
    });

    assert_eq!(batch.families.len(), 3);
    assert_eq!(batch.families[0].compat_kind, CompatKind::NoPanicAllowlist);
    assert_eq!(batch.families[1].compat_kind, CompatKind::PanicBaseline);
    assert_eq!(batch.families[2].compat_kind, CompatKind::LintException);
    assert_eq!(batch.families[0].finding_kind, FindingKind::Panic);
    assert_eq!(batch.families[2].finding_kind, FindingKind::LintException);
    assert!(
        batch.families[0].entry_families.contains(&"unwrap".to_string())
            && batch.families[1].entry_families.contains(&"unwrap".to_string())
            && batch.families[2]
                .entry_families
                .contains(&"expect_attribute".to_string()),
        "batch import should preserve distinct per-lane entry families without collapsing panic and lint lanes"
    );

    let cfg = &batch.config;
    assert!(
        cfg.allow.iter().any(|entry| entry.id == "fixture-no-panic-unwrap"
            && entry.kind == FindingKind::Panic),
        "batch import should retain reviewed panic allowlist entries"
    );
    assert!(
        cfg.allow.iter().any(|entry| entry.id == "panic-baseline-0001"
            && entry.kind == FindingKind::Panic
            && entry.occurrence_limit == Some(2)),
        "batch import should retain panic baseline occurrence limits"
    );
    assert!(
        cfg.allow.iter().any(|entry| entry.id == "fixture-clippy"
            && entry.kind == FindingKind::LintException),
        "batch import should retain lint exception entries"
    );
}

#[test]
fn migration_fixture_matrix_policy_dir_batch_imports_primary_lanes() {
    let dir = crate::test_support::fixture_dir();

    for descriptor in all_legacy_lane_descriptors() {
        let source = migration_fixture_path(descriptor.primary_fixture_file);
        let text = fs::read_to_string(&source).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "read migration fixture {}: {err}",
                descriptor.primary_fixture_file
            ))
        });
        fs::write(dir.join(descriptor.legacy_filename), text).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "write policy dir fixture {}: {err}",
                descriptor.legacy_filename
            ))
        });
    }

    let cfg = load_legacy_policy_dir(&dir).unwrap_or_else(|err| {
        std::panic::panic_any(format!("policy directory batch migration: {err}"))
    });

    assert_eq!(cfg.policy, "cargo-allow");
    assert!(
        cfg.allow.len() >= all_legacy_lane_descriptors().len(),
        "batch import should merge all primary lane entries; got {}",
        cfg.allow.len()
    );
    assert!(
        cfg.allow.iter().any(|entry| entry.id == "fixture-non-rust"),
        "batch import should include non-rust lane"
    );
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.id == "panic-baseline-0001"),
        "batch import should include panic baseline lane"
    );
}

#[derive(Clone, Copy)]
enum OccurrenceLimitExpect {
    None,
    Some(u32),
}

#[derive(Clone, Copy)]
enum CompatLoader {
    Generated,
    Executable,
    Workflow,
    DependencySurface,
    Process,
    Network,
    NoPanicAllowlist,
    NoPanicBaseline,
    Clippy,
    Unsafe,
}

impl CompatLoader {
    const fn from_lane(lane: CompatKind) -> Option<Self> {
        Some(match lane {
            CompatKind::NonRust => return None,
            CompatKind::Generated => Self::Generated,
            CompatKind::Executable => Self::Executable,
            CompatKind::Workflow => Self::Workflow,
            CompatKind::DependencySurface => Self::DependencySurface,
            CompatKind::Process => Self::Process,
            CompatKind::Network => Self::Network,
            CompatKind::NoPanicAllowlist => Self::NoPanicAllowlist,
            CompatKind::PanicBaseline => Self::NoPanicBaseline,
            CompatKind::LintException => Self::Clippy,
            CompatKind::Unsafe => Self::Unsafe,
        })
    }
}

struct PrimaryCaseExpectations {
    family: Option<&'static str>,
    expected_owner: &'static str,
    expected_reason: &'static str,
    expected_classification: Option<&'static str>,
    expected_evidence: &'static [&'static str],
    expected_links: &'static [&'static str],
    occurrence_limit: OccurrenceLimitExpect,
    expected_created: Option<&'static str>,
    expected_review_after: Option<&'static str>,
    expected_expires: Option<&'static str>,
    expect_baseline_debt_markers: bool,
}

struct MigrationLaneCase {
    lane: &'static str,
    fixture_file: &'static str,
    legacy_filename: &'static str,
    entry_id: &'static str,
    family: Option<&'static str>,
    expected_owner: &'static str,
    expected_reason: &'static str,
    expected_classification: Option<&'static str>,
    expected_evidence: &'static [&'static str],
    expected_links: &'static [&'static str],
    occurrence_limit: OccurrenceLimitExpect,
    expected_created: Option<&'static str>,
    expected_review_after: Option<&'static str>,
    expected_expires: Option<&'static str>,
    expect_baseline_debt_markers: bool,
    compat_loader: Option<CompatLoader>,
}

fn migration_lane_cases() -> Vec<MigrationLaneCase> {
    let mut cases = primary_migration_lane_cases();
    cases.extend(variant_migration_lane_cases());
    cases
}

#[test]
fn primary_migration_lane_cases_cover_all_compat_kinds() {
    assert_eq!(
        primary_migration_lane_cases().len(),
        CompatKind::ALL.len(),
        "primary migration fixture matrix should cover every compat kind"
    );
}

fn primary_migration_lane_cases() -> Vec<MigrationLaneCase> {
    [
        primary_case_for_lane(
            CompatKind::NonRust,
            "non-rust",
            "fixture-non-rust",
            PrimaryCaseExpectations {
                family: None,
                expected_owner: "docs",
                expected_reason: "Repository policy prose.",
                expected_classification: Some("documentation"),
                expected_evidence: &["doc:docs/source-exception-ledger.md", "issue:#123"],
                expected_links: &[],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-05-09"),
                expected_expires: Some("never"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::Generated,
            "generated",
            "fixture-generated",
            PrimaryCaseExpectations {
                family: Some("generated_code"),
                expected_owner: "policy",
                expected_reason: "Generated schema fixture.",
                expected_classification: Some("generated_code"),
                expected_evidence: &["doc:docs/schemas/README.md"],
                expected_links: &["legacy-policy:fixture-generated"],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-10"),
                expected_review_after: Some("2026-05-10"),
                expected_expires: Some("never"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::Executable,
            "executable",
            "fixture-executable",
            PrimaryCaseExpectations {
                family: Some("executable_file"),
                expected_owner: "release",
                expected_reason: "Release helper fixture.",
                expected_classification: Some("executable_file"),
                expected_evidence: &["doc:docs/release/README.md"],
                expected_links: &["legacy-policy:fixture-executable"],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-08-09"),
                expected_expires: Some("never"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::Workflow,
            "workflow",
            "workflow-action-github-workflows-release-yml--actions-checkout-v4",
            PrimaryCaseExpectations {
                family: Some("workflow_external_action"),
                expected_owner: "release/ci",
                expected_reason: "Release workflow fixture.",
                expected_classification: Some("workflow_external_action"),
                expected_evidence: &["doc:docs/ci.md"],
                expected_links: &[],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-09-09"),
                expected_expires: Some("never"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::DependencySurface,
            "dependency-surface",
            "fixture-dependency",
            PrimaryCaseExpectations {
                family: Some("dependency_surface"),
                expected_owner: "release",
                expected_reason: "Workspace dependency block fixture.",
                expected_classification: Some("workspace_manifest"),
                expected_evidence: &["doc:docs/dependencies.md", "dep_count_at_baseline:22"],
                expected_links: &[],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-08-09"),
                expected_expires: Some("never"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::Process,
            "process",
            "fixture-process",
            PrimaryCaseExpectations {
                family: Some("process_spawn"),
                expected_owner: "release/ci",
                expected_reason: "Release helper fixture.",
                expected_classification: Some("network_process"),
                expected_evidence: &["doc:docs/ci.md"],
                expected_links: &[],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-08-09"),
                expected_expires: Some("never"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::Network,
            "network",
            "fixture-network",
            PrimaryCaseExpectations {
                family: Some("network_destination"),
                expected_owner: "release",
                expected_reason: "Release API fixture.",
                expected_classification: Some("public_network"),
                expected_evidence: &["doc:docs/release/README.md"],
                expected_links: &[],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-08-09"),
                expected_expires: Some("never"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::NoPanicAllowlist,
            "no-panic allowlist",
            "fixture-no-panic-unwrap",
            PrimaryCaseExpectations {
                family: Some("unwrap"),
                expected_owner: "parser",
                expected_reason: "Parser validates the optional value.",
                expected_classification: Some("reviewed_panic_exception"),
                expected_evidence: &["test:parser_validates_optional_value", "issue:#123"],
                expected_links: &[],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: None,
                expected_review_after: Some("2026-09-09"),
                expected_expires: None,
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::PanicBaseline,
            "panic baseline",
            "panic-baseline-0001",
            PrimaryCaseExpectations {
                family: Some("unwrap"),
                expected_owner: "parser",
                expected_reason: "Counted unwrap baseline after parser hardening.",
                expected_classification: Some("baseline_debt"),
                expected_evidence: &["test:parser_baseline", "issue:#456"],
                expected_links: &["legacy-policy:no-panic-baseline"],
                occurrence_limit: OccurrenceLimitExpect::Some(2),
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-06-09"),
                expected_expires: Some("2026-06-09"),
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::LintException,
            "lint-exception",
            "fixture-clippy",
            PrimaryCaseExpectations {
                family: Some("expect_attribute"),
                expected_owner: "lint",
                expected_reason: "Parser validates optional value before unwrap.",
                expected_classification: Some("reviewed_lint_exception"),
                expected_evidence: &["test:parser_validates_optional_value"],
                expected_links: &[],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-09-09"),
                expected_expires: None,
                expect_baseline_debt_markers: false,
            },
        ),
        primary_case_for_lane(
            CompatKind::Unsafe,
            "unsafe",
            "fixture-unsafe",
            PrimaryCaseExpectations {
                family: Some("unsafe_block"),
                expected_owner: "runtime",
                expected_reason: "Caller validates pointer before read.",
                expected_classification: Some("reviewed_unsafe_boundary"),
                expected_evidence: &["unsafe-review:docs/evidence/unsafe/read.json"],
                expected_links: &["legacy-policy:fixture-unsafe"],
                occurrence_limit: OccurrenceLimitExpect::None,
                expected_created: Some("2026-05-09"),
                expected_review_after: Some("2026-09-09"),
                expected_expires: None,
                expect_baseline_debt_markers: false,
            },
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn primary_case(
    descriptor: LegacyLaneDescriptor,
    lane_label: &'static str,
    entry_id: &'static str,
    expect: PrimaryCaseExpectations,
) -> MigrationLaneCase {
    MigrationLaneCase {
        lane: lane_label,
        fixture_file: descriptor.primary_fixture_file,
        legacy_filename: descriptor.legacy_filename,
        entry_id,
        family: expect.family,
        expected_owner: expect.expected_owner,
        expected_reason: expect.expected_reason,
        expected_classification: expect.expected_classification,
        expected_evidence: expect.expected_evidence,
        expected_links: expect.expected_links,
        occurrence_limit: expect.occurrence_limit,
        expected_created: expect.expected_created,
        expected_review_after: expect.expected_review_after,
        expected_expires: expect.expected_expires,
        expect_baseline_debt_markers: expect.expect_baseline_debt_markers,
        compat_loader: CompatLoader::from_lane(descriptor.lane),
    }
}

fn primary_case_for_lane(
    lane: CompatKind,
    lane_label: &'static str,
    entry_id: &'static str,
    expect: PrimaryCaseExpectations,
) -> Option<MigrationLaneCase> {
    let descriptor = all_legacy_lane_descriptors()
        .iter()
        .find(|descriptor| descriptor.lane == lane)
        .copied()?;
    Some(primary_case(descriptor, lane_label, entry_id, expect))
}

fn variant_lane_descriptor(lane: CompatKind) -> Option<LegacyLaneDescriptor> {
    all_legacy_lane_descriptors()
        .iter()
        .find(|descriptor| descriptor.lane == lane)
        .copied()
}

fn variant_migration_lane_cases() -> Vec<MigrationLaneCase> {
    let Some(panic_baseline) = variant_lane_descriptor(CompatKind::PanicBaseline) else {
        return vec![];
    };
    let Some(lint_exception) = variant_lane_descriptor(CompatKind::LintException) else {
        return vec![];
    };
    let Some(unsafe_lane) = variant_lane_descriptor(CompatKind::Unsafe) else {
        return vec![];
    };

    vec![
        MigrationLaneCase {
            lane: "panic baseline missing evidence",
            fixture_file: "panic-baseline-no-evidence.toml",
            legacy_filename: panic_baseline.legacy_filename,
            entry_id: "panic-baseline-0001",
            family: Some("unwrap"),
            expected_owner: "parser",
            expected_reason: "Counted unwrap baseline without evidence.",
            expected_classification: Some("baseline_debt"),
            expected_evidence: &["baseline_count:3"],
            expected_links: &["legacy-policy:no-panic-baseline"],
            occurrence_limit: OccurrenceLimitExpect::Some(3),
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-06-09"),
            expected_expires: Some("2026-06-09"),
            expect_baseline_debt_markers: true,
            compat_loader: None,
        },
        MigrationLaneCase {
            lane: "lint-exception minimal",
            fixture_file: "lint-exception-minimal.toml",
            legacy_filename: lint_exception.legacy_filename,
            entry_id: "legacy-clippy-0000",
            family: Some("expect_attribute"),
            expected_owner: "unowned",
            expected_reason: "requires human review",
            expected_classification: Some("baseline_debt"),
            expected_evidence: &["legacy-policy:legacy-clippy-0000"],
            expected_links: &[],
            occurrence_limit: OccurrenceLimitExpect::None,
            expected_created: None,
            expected_review_after: None,
            expected_expires: None,
            expect_baseline_debt_markers: true,
            compat_loader: Some(CompatLoader::Clippy),
        },
        MigrationLaneCase {
            lane: "unsafe missing evidence",
            fixture_file: "unsafe-no-evidence.toml",
            legacy_filename: unsafe_lane.legacy_filename,
            entry_id: "fixture-unsafe-no-evidence",
            family: Some("unsafe_fn"),
            expected_owner: "runtime",
            expected_reason: "Caller validates FFI boundary before call.",
            expected_classification: Some("reviewed_unsafe_boundary"),
            expected_evidence: &[],
            expected_links: &["legacy-policy:fixture-unsafe-no-evidence"],
            occurrence_limit: OccurrenceLimitExpect::None,
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-09-09"),
            expected_expires: None,
            expect_baseline_debt_markers: false,
            compat_loader: Some(CompatLoader::Unsafe),
        },
    ]
}

fn migration_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migration")
}

fn migration_fixture_path(fixture_file: &str) -> PathBuf {
    migration_fixture_root().join(fixture_file)
}

fn stage_migration_fixture(fixture_file: &str, legacy_filename: &str) -> PathBuf {
    let dir = crate::test_support::fixture_dir();
    let source = migration_fixture_path(fixture_file);
    let text = fs::read_to_string(&source).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "read migration fixture {}: {err}",
            source.display()
        ))
    });
    let path = dir.join(legacy_filename);
    fs::write(&path, text).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "write staged migration fixture {}: {err}",
            path.display()
        ))
    });
    path
}

fn find_matrix_entry<'a>(
    cfg: &'a AllowConfig,
    entry_id: &str,
    family: Option<&str>,
) -> Option<&'a AllowEntry> {
    cfg.allow.iter().find(|entry| {
        entry.id == entry_id && family.is_none_or(|family| entry.family.as_deref() == Some(family))
    })
}

fn assert_baseline_debt_visible(lane: &str, entry: &AllowEntry) {
    assert_eq!(
        entry.classification, "baseline_debt",
        "{lane} should classify missing-evidence debt as baseline_debt"
    );
    let has_traceability = entry.evidence.iter().any(|item| {
        item.starts_with("legacy_policy:")
            || item.starts_with("legacy-policy:")
            || item.starts_with("baseline_count:")
            || item.contains("TODO: add unsafe-review")
    });
    assert!(
        has_traceability
            || entry
                .links
                .iter()
                .any(|item| item.starts_with("legacy-policy:")),
        "{lane} should keep visible baseline_debt traceability in evidence {:?} or links {:?}",
        entry.evidence,
        entry.links
    );
}

fn load_compat_config(loader: CompatLoader, path: &Path) -> CargoAllowResult<AllowConfig> {
    match loader {
        CompatLoader::Generated => load_generated_compat_config(path),
        CompatLoader::Executable => load_executable_compat_config(path),
        CompatLoader::Workflow => load_workflow_compat_config(path),
        CompatLoader::DependencySurface => load_dependency_surface_compat_config(path),
        CompatLoader::Process => load_process_compat_config(path),
        CompatLoader::Network => load_network_compat_config(path),
        CompatLoader::NoPanicAllowlist => load_no_panic_allowlist_compat_config(path),
        CompatLoader::NoPanicBaseline => load_no_panic_baseline_compat_config(path),
        CompatLoader::Clippy => load_clippy_exceptions_compat_config(path),
        CompatLoader::Unsafe => load_unsafe_allowlist_compat_config(path),
    }
}

fn migration_fingerprint(cfg: &AllowConfig) -> Vec<String> {
    let mut rows = cfg
        .allow
        .iter()
        .map(|entry| {
            format!(
                "{}|{:?}|{}|{:?}|{:?}|{:?}|{:?}",
                entry.id,
                entry.family,
                entry.classification,
                entry.owner,
                entry.occurrence_limit,
                entry.evidence,
                entry.lifecycle.expires
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}
