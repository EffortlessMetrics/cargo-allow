use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, Selector, Span, StructuralIdentity,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod finding_posture;
mod policy_lifecycle;
mod policy_metadata;
mod policy_scope;
mod policy_selector;

#[test]
fn detects_requirement_loosened_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.requirements.owner_required = false;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementLoosened
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "requirements.owner_required"
            && change.message.contains("true -> false")
    }));
}

#[test]
fn detects_requirement_tightened_as_improvement() {
    let mut base = config_with(entry("allow-1"));
    base.requirements.evidence_required = false;
    let mut head = base.clone();
    head.requirements.evidence_required = true;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementTightened
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "requirements.evidence_required"
            && change.message.contains("false -> true")
    }));
}

#[test]
fn detects_allow_bare_allow_attributes_polarity() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.requirements.allow_bare_allow_attributes = true;

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementLoosened
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "requirements.allow_bare_allow_attributes"
            && change.message.contains("false -> true")
    }));

    let changes = policy_changes(&head, &base);
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::RequirementTightened
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "requirements.allow_bare_allow_attributes"
            && change.message.contains("true -> false")
    }));
}

#[test]
fn detects_added_workspace_ignored_scope_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.workspace.ignored.push("src/**".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceIgnoredAdded
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "workspace.ignored"
            && change.message.contains("src/**")
    }));
}

#[test]
fn detects_removed_workspace_ignored_scope_as_improvement() {
    let mut base = config_with(entry("allow-1"));
    base.workspace.ignored.push("ignored/**".to_string());
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceIgnoredRemoved
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "workspace.ignored"
            && change.message.contains("ignored/**")
    }));
}

#[test]
fn detects_workspace_generated_scope_changes() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.workspace.generated.push("schemas/**".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceGeneratedAdded
            && change.severity == PolicyChangeSeverity::Review
            && change.allow_id == "workspace.generated"
            && change.message.contains("schemas/**")
    }));

    let changes = policy_changes(&head, &base);
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceGeneratedRemoved
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "workspace.generated"
            && change.message.contains("schemas/**")
    }));
}

#[test]
fn workspace_scope_changes_normalize_windows_separators() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.workspace.ignored.push(r"src\generated\**".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceIgnoredAdded
            && change.message.contains("src/generated/**")
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
fn detects_baseline_debt_introduced_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head_entry = entry("allow-1");
    head_entry.classification = "baseline_debt".to_string();
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::BaselineDebtIntroduced
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("baseline_debt")
    }));
    assert!(
        !changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ClassificationChanged),
        "baseline debt introduction should not be downgraded to a generic classification change"
    );
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
            PolicyChangeKind::BaselineDebtIntroduced,
            "baseline_debt_introduced",
        ),
        (
            PolicyChangeKind::BaselineDebtNormalized,
            "baseline_debt_normalized",
        ),
        (PolicyChangeKind::KindChanged, "kind_changed"),
        (PolicyChangeKind::FamilyChanged, "family_changed"),
        (PolicyChangeKind::ScopeBroadened, "scope_broadened"),
        (PolicyChangeKind::ScopeChanged, "scope_changed"),
        (PolicyChangeKind::ScopeNarrowed, "scope_narrowed"),
        (PolicyChangeKind::SelectorChanged, "selector_changed"),
        (
            PolicyChangeKind::SelectorPrecisionDecreased,
            "selector_precision_decreased",
        ),
        (
            PolicyChangeKind::SelectorPrecisionIncreased,
            "selector_precision_increased",
        ),
        (PolicyChangeKind::CreatedAdded, "created_added"),
        (PolicyChangeKind::CreatedChanged, "created_changed"),
        (PolicyChangeKind::CreatedRemoved, "created_removed"),
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
        (PolicyChangeKind::LinkAdded, "link_added"),
        (PolicyChangeKind::LinkRemoved, "link_removed"),
        (PolicyChangeKind::OwnerAdded, "owner_added"),
        (PolicyChangeKind::OwnerChanged, "owner_changed"),
        (PolicyChangeKind::OwnerRemoved, "owner_removed"),
        (PolicyChangeKind::OwnerUnassigned, "owner_unassigned"),
        (PolicyChangeKind::PolicyOwnerAdded, "policy_owner_added"),
        (PolicyChangeKind::PolicyOwnerChanged, "policy_owner_changed"),
        (PolicyChangeKind::PolicyOwnerRemoved, "policy_owner_removed"),
        (
            PolicyChangeKind::PolicyOwnerUnassigned,
            "policy_owner_unassigned",
        ),
        (
            PolicyChangeKind::PolicyStatusChanged,
            "policy_status_changed",
        ),
        (
            PolicyChangeKind::PolicyStatusWeakened,
            "policy_status_weakened",
        ),
        (
            PolicyChangeKind::PolicyStatusTightened,
            "policy_status_tightened",
        ),
        (PolicyChangeKind::ReasonAdded, "reason_added"),
        (PolicyChangeKind::ReasonChanged, "reason_changed"),
        (PolicyChangeKind::ReasonRemoved, "reason_removed"),
        (
            PolicyChangeKind::RequirementLoosened,
            "requirement_loosened",
        ),
        (
            PolicyChangeKind::RequirementTightened,
            "requirement_tightened",
        ),
        (
            PolicyChangeKind::WorkspaceIgnoredAdded,
            "workspace_ignored_added",
        ),
        (
            PolicyChangeKind::WorkspaceIgnoredRemoved,
            "workspace_ignored_removed",
        ),
        (
            PolicyChangeKind::WorkspaceGeneratedAdded,
            "workspace_generated_added",
        ),
        (
            PolicyChangeKind::WorkspaceGeneratedRemoved,
            "workspace_generated_removed",
        ),
        (
            PolicyChangeKind::ClassificationAdded,
            "classification_added",
        ),
        (
            PolicyChangeKind::ClassificationChanged,
            "classification_changed",
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
fn findings_at_revision_includes_generated_gitattributes_companions() {
    let root = temp_root("revision-generated-gitattributes");
    fs::create_dir_all(root.join("generated"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated dir: {err}")));
    fs::write(
        root.join(".gitattributes"),
        "generated/schema.json linguist-generated=true\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("gitattributes write: {err}")));
    fs::write(root.join("generated").join("schema.json"), "{}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated file write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(generated_code_entry("generated/schema.json"));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::GeneratedCode
            && finding.family.as_deref() == Some("generated_code")
            && finding.path.as_path() == Path::new("generated/schema.json")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_workflow_companions() {
    let root = temp_root("revision-workflow");
    let workflow_dir = root.join(".github").join("workflows");
    fs::create_dir_all(&workflow_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
    fs::write(
        workflow_dir.join("ci.yml"),
        "steps:\n  - uses: actions/checkout@v4\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("workflow write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(workflow_entry(
        "workflow-ci",
        "github_workflow",
        "github_workflow",
        ".github/workflows/ci.yml",
        None,
    ));
    cfg.allow.push(workflow_entry(
        "workflow-action-checkout",
        "workflow_external_action",
        "github_action_uses",
        ".github/workflows/ci.yml",
        Some("action:actions/checkout@v4"),
    ));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("github_workflow")
            && finding.path.as_path() == Path::new(".github/workflows/ci.yml")
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("workflow_external_action")
            && finding.identity.target_fingerprint.as_deref() == Some("action:actions/checkout@v4")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_config_companions() {
    let root = temp_root("revision-config-companions");
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["commit", "--allow-empty", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(config_policy_entry(
        "proc-cargo-test",
        "process_spawn",
        ".github/workflows/ci.yml",
        "cargo test",
        "process:cargo test",
    ));
    cfg.allow.push(config_policy_entry(
        "net-crates-io",
        "network_destination",
        "policy/network-allowlist.toml",
        "crates.io lane build",
        "network:crates.io:auth:false:lane:build",
    ));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("process_spawn")
            && finding.identity.target_fingerprint.as_deref() == Some("process:cargo test")
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("network_destination")
            && finding.identity.target_fingerprint.as_deref()
                == Some("network:crates.io:auth:false:lane:build")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_executable_tree_mode_companions() {
    let root = temp_root("revision-executable");
    let script_dir = root.join("scripts");
    fs::create_dir_all(&script_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("script dir: {err}")));
    fs::write(script_dir.join("package-proof.sh"), "#!/usr/bin/env bash\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("script write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["update-index", "--chmod=+x", "scripts/package-proof.sh"],
    );
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(executable_entry("scripts/package-proof.sh"));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("executable_file")
            && finding.path.as_path() == Path::new("scripts/package-proof.sh")
            && finding.identity.target_fingerprint.as_deref() == Some("git-mode:100755")
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

#[test]
fn git_tree_revision_parser_preserves_executable_modes() {
    let output = b"100644 blob abc123\tREADME.md\0\
100755 blob def456\tscripts/package-proof.sh\0\
120000 blob fedcba\tscripts/link.sh\0";

    let files = revision_git::parse_git_ls_tree_file_entries_z(output);

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].mode, "100644");
    assert_eq!(files[0].path, PathBuf::from("README.md"));
    assert_eq!(files[1].mode, "100755");
    assert_eq!(files[1].path, PathBuf::from("scripts/package-proof.sh"));
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

fn generated_code_entry(path: &str) -> AllowEntry {
    AllowEntry {
        id: "generated-schema".to_string(),
        kind: FindingKind::GeneratedCode,
        family: Some("generated_code".to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "codegen".to_string(),
        classification: "generated_code".to_string(),
        reason: "Generated schema is tracked for review.".to_string(),
        evidence: vec!["legacy-policy:generated".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: Some("json".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn workflow_entry(
    id: &str,
    family: &str,
    ast_kind: &str,
    path: &str,
    target_fingerprint: Option<&str>,
) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::PolicyException,
        family: Some(family.to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "ci".to_string(),
        classification: family.to_string(),
        reason: "Workflow surface is governed by policy.".to_string(),
        evidence: vec!["legacy-policy:workflow".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some(ast_kind.to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: target_fingerprint.map(str::to_string),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn executable_entry(path: &str) -> AllowEntry {
    AllowEntry {
        id: "exec-package-proof".to_string(),
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "release".to_string(),
        classification: "executable_file".to_string(),
        reason: "Release helper intentionally retains an executable bit.".to_string(),
        evidence: vec!["legacy-policy:executable".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("git_executable_file".to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: Some("git-mode:100755".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn config_policy_entry(
    id: &str,
    family: &str,
    path: &str,
    symbol: &str,
    target_fingerprint: &str,
) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::PolicyException,
        family: Some(family.to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "infra".to_string(),
        classification: family.to_string(),
        reason: "Policy surface is retained for review.".to_string(),
        evidence: vec!["legacy-policy:policy".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some(family.to_string()),
            symbol: Some(symbol.to_string()),
            target_fingerprint: Some(target_fingerprint.to_string()),
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
