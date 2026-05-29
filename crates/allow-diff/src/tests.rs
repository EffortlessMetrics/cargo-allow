use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, Selector, Span, StructuralIdentity,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn finding_posture_ignores_line_movement_for_same_identity() {
    let base = vec![finding("src/lib.rs", 10, "load")];
    let head = vec![finding("src/lib.rs", 99, "load")];

    let changes = finding_posture_changes(&base, &head);

    assert!(changes.is_empty());
}

#[test]
fn finding_posture_reports_new_and_removed_findings() {
    let base = vec![finding("src/old.rs", 10, "old")];
    let head = vec![finding("src/new.rs", 10, "new")];

    let changes = finding_posture_changes(&base, &head);

    assert!(
        changes.iter().any(|change| {
            change.kind == FindingPostureKind::New && change.path == "src/new.rs"
        })
    );
    assert!(changes.iter().any(|change| {
        change.kind == FindingPostureKind::Removed && change.path == "src/old.rs"
    }));
}

#[test]
fn finding_posture_reports_count_changes_for_same_identity() {
    let base = vec![finding("src/lib.rs", 10, "load")];
    let head = vec![
        finding("src/lib.rs", 10, "load"),
        finding("src/lib.rs", 20, "load"),
    ];

    let changes = finding_posture_changes(&base, &head);

    assert_eq!(changes.len(), 1);
    let change = changes
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one posture change"));
    assert_eq!(change.kind, FindingPostureKind::New);
    assert_eq!(change.path, "src/lib.rs");
}

#[test]
fn detects_scope_broadening_from_path_to_glob() {
    let base = config_with(entry("allow-1"));
    let mut widened = entry("allow-1");
    widened.path = None;
    widened.glob = Some("src/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeBroadened
            && change.severity == PolicyChangeSeverity::Fail
    }));
}

#[test]
fn detects_selector_glob_broadening_even_when_path_remains() {
    let mut base_entry = entry("allow-1");
    base_entry.selector.glob = Some("src/lib.rs".to_string());
    let base = config_with(base_entry);
    let mut widened = entry("allow-1");
    widened.selector.glob = Some("src/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&base, &head);

    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ScopeBroadened)
    );
}

#[test]
fn detects_scope_narrowing_from_glob_to_path() {
    let mut base_entry = entry("allow-1");
    base_entry.path = None;
    base_entry.glob = Some("src/**".to_string());
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeNarrowed
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_scope_narrowing_between_globs() {
    let mut base_entry = entry("allow-1");
    base_entry.path = None;
    base_entry.glob = Some("src/**".to_string());
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.path = None;
    head_entry.glob = Some("src/parser/**".to_string());
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeNarrowed
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn scope_broadening_respects_directory_segment_boundaries() {
    let mut widened = entry("allow-1");
    widened.path = None;
    widened.glob = Some("src/parse/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&config_with(entry("allow-1")), &head);

    assert!(
        !changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ScopeBroadened),
        "src/parse/** must not be treated as covering src/parser/lib.rs"
    );
}

#[test]
fn glob_scope_changes_respect_directory_segment_boundaries() {
    let mut base_entry = entry("allow-1");
    base_entry.path = None;
    base_entry.glob = Some("src/parser/**".to_string());
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.path = None;
    head_entry.glob = Some("src/parse/**".to_string());
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(
        !changes.iter().any(|change| {
            matches!(
                change.kind,
                PolicyChangeKind::ScopeBroadened | PolicyChangeKind::ScopeNarrowed
            )
        }),
        "sibling directory globs should not be classified as broadened or narrowed"
    );
}

#[test]
fn detects_selector_precision_decrease() {
    let base = config_with(entry("allow-1"));
    let mut weaker = entry("allow-1");
    weaker.selector.normalized_snippet_hash = None;
    weaker.selector.container = None;
    let head = config_with(weaker);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::SelectorPrecisionDecreased
            && change.message.contains("decreased")
    }));
}

#[test]
fn detects_allow_entry_retargeted_to_different_kind_or_family() {
    let base = config_with(entry("allow-1"));
    let mut retargeted = entry("allow-1");
    retargeted.kind = FindingKind::Unsafe;
    retargeted.family = Some("unsafe_block".to_string());
    let head = config_with(retargeted);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::KindChanged
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("panic -> unsafe")
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::FamilyChanged
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("unwrap -> unsafe_block")
    }));
}

#[test]
fn detects_selector_precision_increase_as_improvement() {
    let mut weaker = entry("allow-1");
    weaker.selector.normalized_snippet_hash = None;
    weaker.selector.container = None;
    let base = config_with(weaker);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::SelectorPrecisionIncreased
            && change.severity == PolicyChangeSeverity::Improvement
            && change.message.contains("increased")
    }));
}

#[test]
fn selector_precision_scores_structural_selectors_above_glob_only_scope() {
    let strong = entry("allow-1");
    let mut weak = entry("allow-1");
    weak.path = None;
    weak.glob = Some("src/**".to_string());
    weak.selector.ast_kind = None;
    weak.selector.container = None;
    weak.selector.callee = None;
    weak.selector.normalized_snippet_hash = None;

    assert!(selector_precision_score(&strong) > selector_precision_score(&weak));
}

#[test]
fn selector_precision_ignores_line_hints() {
    let mut with_hint = entry("allow-1");
    with_hint.selector.line_hint = Some(900);
    let mut without_hint = entry("allow-1");
    without_hint.selector.line_hint = None;

    assert_eq!(
        selector_precision_score(&with_hint),
        selector_precision_score(&without_hint)
    );
}

#[test]
fn detects_evidence_removed_and_lifecycle_extended() {
    let base = config_with(entry("allow-1"));
    let mut weaker = entry("allow-1");
    weaker.evidence.clear();
    weaker.lifecycle.expires = Some("2026-12-01".to_string());
    weaker.lifecycle.review_after = Some("2026-10-01".to_string());
    let head = config_with(weaker);

    let changes = policy_changes(&base, &head);

    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::EvidenceRemoved)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ExpiryExtended)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ReviewAfterExtended)
    );
}

#[test]
fn detects_lifecycle_shortened_as_improvement() {
    let base = config_with(entry("allow-1"));
    let mut tighter = entry("allow-1");
    tighter.lifecycle.expires = Some("2026-08-15".to_string());
    tighter.lifecycle.review_after = Some("2026-07-01".to_string());
    let head = config_with(tighter);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_added_lifecycle_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.lifecycle.expires = None;
    base_entry.lifecycle.review_after = None;
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_evidence_added_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::EvidenceAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_required_metadata_removed_and_limit_loosened() {
    let base = config_with(entry("allow-1"));
    let mut weaker = entry("allow-1");
    weaker.owner.clear();
    weaker.reason.clear();
    weaker.classification.clear();
    weaker.occurrence_limit = None;
    let head = config_with(weaker);

    let changes = policy_changes(&base, &head);

    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::OwnerRemoved)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ReasonRemoved)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ClassificationRemoved)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::OccurrenceLimitLoosened)
    );
}

#[test]
fn detects_required_metadata_added_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.owner.clear();
    base_entry.reason.clear();
    base_entry.classification.clear();
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OwnerAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReasonAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ClassificationAdded
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_occurrence_limit_tightened_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.occurrence_limit = Some(4);
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.occurrence_limit = Some(2);
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OccurrenceLimitTightened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_new_occurrence_limit_as_improvement() {
    let mut base_entry = entry("allow-1");
    base_entry.occurrence_limit = None;
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OccurrenceLimitTightened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_added_baseline_debt_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut added = entry("allow-2");
    added.classification = "baseline_debt".to_string();
    let mut head = base.clone();
    head.allow.push(added);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::BaselineDebtAdded
            && change.severity == PolicyChangeSeverity::Fail
    }));
}

#[test]
fn detects_baseline_debt_normalized_as_failure() {
    let mut base_entry = entry("allow-1");
    base_entry.classification = "baseline_debt".to_string();
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::BaselineDebtNormalized
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("baseline_debt")
    }));
}

#[test]
fn detects_removed_allow_as_improvement() {
    let mut base = config_with(entry("allow-1"));
    base.allow.push(entry("allow-2"));
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.allow_id == "allow-2"
            && change.kind == PolicyChangeKind::RemovedAllow
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn lifecycle_never_and_removed_dates_are_classified_by_risk_direction() {
    let mut never_base = entry("allow-1");
    never_base.lifecycle.expires = Some("never".to_string());
    never_base.lifecycle.review_after = Some("never".to_string());
    let base = config_with(never_base);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterShortened
            && change.severity == PolicyChangeSeverity::Improvement
    }));

    let base = config_with(entry("allow-1"));
    let mut removed = entry("allow-1");
    removed.lifecycle.expires = None;
    removed.lifecycle.review_after = None;
    let head = config_with(removed);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ExpiryExtended
            && change.severity == PolicyChangeSeverity::Review
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ReviewAfterExtended
            && change.severity == PolicyChangeSeverity::Review
    }));
}

#[test]
fn lifecycle_invalid_dates_do_not_create_directional_changes() {
    let mut base_entry = entry("allow-1");
    base_entry.lifecycle.expires = Some("not-a-date".to_string());
    base_entry.lifecycle.review_after = Some("2026-08-01".to_string());
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.lifecycle.expires = Some("2026-12-01".to_string());
    head_entry.lifecycle.review_after = Some("also-not-a-date".to_string());
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(!changes.iter().any(|change| matches!(
        change.kind,
        PolicyChangeKind::ExpiryExtended
            | PolicyChangeKind::ExpiryShortened
            | PolicyChangeKind::ReviewAfterExtended
            | PolicyChangeKind::ReviewAfterShortened
    )));
}

#[test]
fn detects_occurrence_limit_increase_as_loosened() {
    let mut base_entry = entry("allow-1");
    base_entry.occurrence_limit = Some(1);
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.occurrence_limit = Some(3);
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::OccurrenceLimitLoosened
            && change.severity == PolicyChangeSeverity::Fail
    }));
}

#[test]
fn detects_non_baseline_added_allow_for_review() {
    let base = AllowConfig::empty();
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, PolicyChangeKind::AddedAllow);
    assert_eq!(changes[0].severity, PolicyChangeSeverity::Review);
}

#[test]
fn policy_change_string_helpers_cover_all_public_variants() {
    let cases = [
        (PolicyChangeKind::AddedAllow, "added_allow"),
        (PolicyChangeKind::RemovedAllow, "removed_allow"),
        (PolicyChangeKind::BaselineDebtAdded, "baseline_debt_added"),
        (
            PolicyChangeKind::BaselineDebtNormalized,
            "baseline_debt_normalized",
        ),
        (PolicyChangeKind::KindChanged, "kind_changed"),
        (PolicyChangeKind::FamilyChanged, "family_changed"),
        (PolicyChangeKind::ScopeBroadened, "scope_broadened"),
        (PolicyChangeKind::ScopeNarrowed, "scope_narrowed"),
        (
            PolicyChangeKind::SelectorPrecisionDecreased,
            "selector_precision_decreased",
        ),
        (
            PolicyChangeKind::SelectorPrecisionIncreased,
            "selector_precision_increased",
        ),
        (PolicyChangeKind::ExpiryExtended, "expiry_extended"),
        (PolicyChangeKind::ExpiryShortened, "expiry_shortened"),
        (
            PolicyChangeKind::ReviewAfterExtended,
            "review_after_extended",
        ),
        (
            PolicyChangeKind::ReviewAfterShortened,
            "review_after_shortened",
        ),
        (PolicyChangeKind::EvidenceAdded, "evidence_added"),
        (PolicyChangeKind::EvidenceRemoved, "evidence_removed"),
        (PolicyChangeKind::OwnerAdded, "owner_added"),
        (PolicyChangeKind::OwnerRemoved, "owner_removed"),
        (PolicyChangeKind::ReasonAdded, "reason_added"),
        (PolicyChangeKind::ReasonRemoved, "reason_removed"),
        (
            PolicyChangeKind::ClassificationAdded,
            "classification_added",
        ),
        (
            PolicyChangeKind::ClassificationRemoved,
            "classification_removed",
        ),
        (
            PolicyChangeKind::OccurrenceLimitTightened,
            "occurrence_limit_tightened",
        ),
        (
            PolicyChangeKind::OccurrenceLimitLoosened,
            "occurrence_limit_loosened",
        ),
    ];

    for (kind, expected) in cases {
        assert_eq!(kind.as_str(), expected);
    }
    assert_eq!(PolicyChangeSeverity::Improvement.as_str(), "improvement");
    assert_eq!(PolicyChangeSeverity::Review.as_str(), "review");
    assert_eq!(PolicyChangeSeverity::Fail.as_str(), "fail");
    assert!(!PolicyChangeSeverity::Review.fails());
    assert!(PolicyChangeSeverity::Fail.fails());
}

#[test]
fn findings_at_revision_preserves_source_package_context() {
    let root = temp_root("revision-package-context");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);

    let findings = findings_at_revision(&root, "HEAD", &AllowConfig::empty())
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    let unwrap = findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name.as_deref(), Some("demo"));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_applies_workspace_ignored_globs() {
    let root = temp_root("revision-ignored");
    fs::create_dir_all(root.join("ignored"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("ignored dir: {err}")));
    fs::write(
        root.join("ignored").join("panic.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("ignored rust write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.workspace.ignored.push("ignored/**".to_string());

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(
        findings
            .iter()
            .all(|finding| finding.path.as_path() != Path::new("ignored/panic.rs"))
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.family.as_deref() == Some("unwrap"))
    );
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_dependency_surface_companions() {
    let root = temp_root("revision-dependency-surface");
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(dependency_surface_entry("Cargo.toml"));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("dependency_surface")
            && finding.path.as_path() == Path::new("Cargo.toml")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn git_tree_revision_parser_skips_symlinks_and_preserves_newlines() {
    let output = b"100644 blob abc123\tsrc/lib.rs\0\
120000 blob def456\tsrc/link.rs\0\
160000 commit 123456\tvendor/submodule\0\
100644 blob fedcba\tfixtures/line\nbreak.rs\0";

    let files = revision_git::parse_git_ls_tree_z(output);

    assert_eq!(
        files,
        vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("fixtures/line\nbreak.rs")
        ]
    );
}

fn config_with(entry: AllowEntry) -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);
    cfg
}

fn entry(id: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Range is validated before use.".to_string(),
        evidence: vec!["test:range_is_validated".to_string()],
        links: Vec::new(),
        occurrence_limit: Some(1),
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: Some("2026-09-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("unwrap".to_string()),
            normalized_snippet_hash: Some("fnv1a64:1234".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn dependency_surface_entry(path: &str) -> AllowEntry {
    AllowEntry {
        id: "dep-cargo-toml".to_string(),
        kind: FindingKind::PolicyException,
        family: Some("dependency_surface".to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "release".to_string(),
        classification: "dependency_surface".to_string(),
        reason: "Dependency surface is governed by policy.".to_string(),
        evidence: vec!["legacy-policy:dependency-surface".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("dependency_surface".to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: Some("workspace_manifest".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn finding(path: &str, line: u32, container: &str) -> Finding {
    let mut identity = StructuralIdentity::new("rust", "unsafe_fn");
    identity.container = Some(container.to_string());
    identity.normalized_snippet_hash = Some(format!("fnv1a64:{container}"));
    Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_fn".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 1 }),
        identity,
        message: "test finding".to_string(),
    }
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-diff-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("temp root: {err}")));
    root
}

fn git(root: &PathBuf, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}
