use super::*;
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

        if case.compat_loader.is_some() {
            let compat = load_compat_config(case.compat_loader, &policy_path);
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
    let lanes = [
        ("non-rust", "non-rust.toml", "non-rust-allowlist.toml"),
        ("generated", "generated.toml", "generated-allowlist.toml"),
        ("executable", "executable.toml", "executable-allowlist.toml"),
        ("workflow", "workflow.toml", "workflow-allowlist.toml"),
        (
            "dependency-surface",
            "dependency-surface.toml",
            "dependency-surface-allowlist.toml",
        ),
        ("process", "process.toml", "process-allowlist.toml"),
        ("network", "network.toml", "network-allowlist.toml"),
        (
            "no-panic allowlist",
            "no-panic-allowlist.toml",
            "no-panic-allowlist.toml",
        ),
        (
            "panic baseline",
            "panic-baseline.toml",
            "no-panic-baseline.toml",
        ),
        (
            "lint-exception",
            "lint-exception.toml",
            "clippy-exceptions.toml",
        ),
        ("unsafe", "unsafe.toml", "unsafe-allowlist.toml"),
    ];

    for (lane, fixture_file, legacy_filename) in lanes {
        let policy_path = stage_migration_fixture(fixture_file, legacy_filename);
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
fn migration_fixture_matrix_policy_dir_batch_imports_primary_lanes() {
    let dir = crate::test_support::fixture_dir();
    let mappings = [
        ("non-rust.toml", "non-rust-allowlist.toml"),
        ("generated.toml", "generated-allowlist.toml"),
        ("executable.toml", "executable-allowlist.toml"),
        ("workflow.toml", "workflow-allowlist.toml"),
        (
            "dependency-surface.toml",
            "dependency-surface-allowlist.toml",
        ),
        ("process.toml", "process-allowlist.toml"),
        ("network.toml", "network-allowlist.toml"),
        ("no-panic-allowlist.toml", "no-panic-allowlist.toml"),
        ("panic-baseline.toml", "no-panic-baseline.toml"),
        ("lint-exception.toml", "clippy-exceptions.toml"),
        ("unsafe.toml", "unsafe-allowlist.toml"),
    ];

    for (fixture_file, legacy_filename) in mappings {
        let source = migration_fixture_path(fixture_file);
        let text = fs::read_to_string(&source).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read migration fixture {fixture_file}: {err}"))
        });
        fs::write(dir.join(legacy_filename), text).unwrap_or_else(|err| {
            std::panic::panic_any(format!("write policy dir fixture {legacy_filename}: {err}"))
        });
    }

    let cfg = load_legacy_policy_dir(&dir).unwrap_or_else(|err| {
        std::panic::panic_any(format!("policy directory batch migration: {err}"))
    });

    assert_eq!(cfg.policy, "cargo-allow");
    assert!(
        cfg.allow.len() >= 11,
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
    vec![
        MigrationLaneCase {
            lane: "non-rust",
            fixture_file: "non-rust.toml",
            legacy_filename: "non-rust-allowlist.toml",
            entry_id: "fixture-non-rust",
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
            compat_loader: None,
        },
        MigrationLaneCase {
            lane: "generated",
            fixture_file: "generated.toml",
            legacy_filename: "generated-allowlist.toml",
            entry_id: "fixture-generated",
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
            compat_loader: Some(CompatLoader::Generated),
        },
        MigrationLaneCase {
            lane: "executable",
            fixture_file: "executable.toml",
            legacy_filename: "executable-allowlist.toml",
            entry_id: "fixture-executable",
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
            compat_loader: Some(CompatLoader::Executable),
        },
        MigrationLaneCase {
            lane: "workflow",
            fixture_file: "workflow.toml",
            legacy_filename: "workflow-allowlist.toml",
            entry_id: "workflow-action-github-workflows-release-yml--actions-checkout-v4",
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
            compat_loader: Some(CompatLoader::Workflow),
        },
        MigrationLaneCase {
            lane: "dependency-surface",
            fixture_file: "dependency-surface.toml",
            legacy_filename: "dependency-surface-allowlist.toml",
            entry_id: "fixture-dependency",
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
            compat_loader: Some(CompatLoader::DependencySurface),
        },
        MigrationLaneCase {
            lane: "process",
            fixture_file: "process.toml",
            legacy_filename: "process-allowlist.toml",
            entry_id: "fixture-process",
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
            compat_loader: Some(CompatLoader::Process),
        },
        MigrationLaneCase {
            lane: "network",
            fixture_file: "network.toml",
            legacy_filename: "network-allowlist.toml",
            entry_id: "fixture-network",
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
            compat_loader: Some(CompatLoader::Network),
        },
        MigrationLaneCase {
            lane: "no-panic allowlist",
            fixture_file: "no-panic-allowlist.toml",
            legacy_filename: "no-panic-allowlist.toml",
            entry_id: "fixture-no-panic-unwrap",
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
            compat_loader: Some(CompatLoader::NoPanicAllowlist),
        },
        MigrationLaneCase {
            lane: "panic baseline",
            fixture_file: "panic-baseline.toml",
            legacy_filename: "no-panic-baseline.toml",
            entry_id: "panic-baseline-0001",
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
            compat_loader: Some(CompatLoader::NoPanicBaseline),
        },
        MigrationLaneCase {
            lane: "panic baseline missing evidence",
            fixture_file: "panic-baseline-no-evidence.toml",
            legacy_filename: "no-panic-baseline.toml",
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
            lane: "lint-exception",
            fixture_file: "lint-exception.toml",
            legacy_filename: "clippy-exceptions.toml",
            entry_id: "fixture-clippy",
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
            compat_loader: Some(CompatLoader::Clippy),
        },
        MigrationLaneCase {
            lane: "lint-exception minimal",
            fixture_file: "lint-exception-minimal.toml",
            legacy_filename: "clippy-exceptions.toml",
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
            lane: "unsafe",
            fixture_file: "unsafe.toml",
            legacy_filename: "unsafe-allowlist.toml",
            entry_id: "fixture-unsafe",
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
            compat_loader: Some(CompatLoader::Unsafe),
        },
        MigrationLaneCase {
            lane: "unsafe missing evidence",
            fixture_file: "unsafe-no-evidence.toml",
            legacy_filename: "unsafe-allowlist.toml",
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

fn load_compat_config(loader: Option<CompatLoader>, path: &Path) -> CargoAllowResult<AllowConfig> {
    match loader {
        Some(CompatLoader::Generated) => load_generated_compat_config(path),
        Some(CompatLoader::Executable) => load_executable_compat_config(path),
        Some(CompatLoader::Workflow) => load_workflow_compat_config(path),
        Some(CompatLoader::DependencySurface) => load_dependency_surface_compat_config(path),
        Some(CompatLoader::Process) => load_process_compat_config(path),
        Some(CompatLoader::Network) => load_network_compat_config(path),
        Some(CompatLoader::NoPanicAllowlist) => load_no_panic_allowlist_compat_config(path),
        Some(CompatLoader::NoPanicBaseline) => load_no_panic_baseline_compat_config(path),
        Some(CompatLoader::Clippy) => load_clippy_exceptions_compat_config(path),
        Some(CompatLoader::Unsafe) => load_unsafe_allowlist_compat_config(path),
        None => Err(allow_core::CargoAllowError::new(
            "compat loader not configured for lane",
        )),
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
