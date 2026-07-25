use super::*;
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use allow_policy::{EvidenceReferenceCategory, EvidenceReferenceStatus};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "cargo-allow-evidence-inventory-{}-{stamp}",
        std::process::id()
    ))
}

fn test_entry(id: &str, evidence: Vec<&str>, links: Vec<&str>) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::PolicyException,
        family: Some("network_destination".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "security".to_string(),
        classification: "reviewed".to_string(),
        reason: "Network exception is reviewed.".to_string(),
        evidence: evidence.into_iter().map(str::to_string).collect(),
        links: links.into_iter().map(str::to_string).collect(),
        occurrence_limit: Some(1),
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("function".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

#[test]
fn evidence_reference_diagnostics_for_source_tree_downgrades_present_file_outside_inventory() {
    let root = fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let entry = test_entry("allow-network", vec!["doc:docs/untracked.md"], vec![]);
    let source_tree_files = BTreeSet::new();

    let diagnostics =
        evidence_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(diagnostic.raw, "doc:docs/untracked.md");
    assert_eq!(diagnostic.status, EvidenceReferenceStatus::LocalFileMissing);
    assert_eq!(diagnostic.category, EvidenceReferenceCategory::Missing);
    assert_eq!(
        diagnostic.message,
        DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE
    );
    assert_eq!(
        diagnostic.target.as_deref(),
        Some(Path::new("docs/untracked.md"))
    );
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn evidence_reference_diagnostics_for_source_tree_preserves_inventory_members() {
    let root = fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/tracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let entry = test_entry("allow-network", vec!["doc:docs/tracked.md"], vec![]);
    let mut source_tree_files = BTreeSet::new();
    source_tree_files.insert("docs/tracked.md".to_string());

    let diagnostics =
        evidence_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(diagnostic.status, EvidenceReferenceStatus::LocalFilePresent);
    assert_eq!(diagnostic.category, EvidenceReferenceCategory::Present);
    assert_eq!(diagnostic.message, "local evidence file exists");
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn evidence_reference_diagnostics_for_source_tree_skips_inventory_when_unavailable() {
    let root = fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/local.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let entry = test_entry("allow-network", vec!["doc:docs/local.md"], vec![]);

    let diagnostics = evidence_reference_diagnostics_for_source_tree(&root, &entry, None);

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(diagnostic.status, EvidenceReferenceStatus::LocalFilePresent);
    assert_eq!(diagnostic.category, EvidenceReferenceCategory::Present);
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn evidence_reference_rejects_directory_target_as_invalid_local_path() {
    // #1949: a directory path used as evidence (e.g. `doc:docs/`) must be
    // flagged as InvalidLocalPath, not silently treated as valid. The base
    // evidence_reference_diagnostic function catches this at the metadata
    // level (Ok(_) => InvalidLocalPath "exists but is not a file").
    let root = fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));

    let entry = test_entry("allow-dir-evidence", vec!["doc:docs"], vec![]);
    let source_tree_files = BTreeSet::new();

    let diagnostics =
        evidence_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(
        diagnostic.status,
        EvidenceReferenceStatus::InvalidLocalPath,
        "directory evidence target should be InvalidLocalPath: {:?}",
        diagnostic
    );
    assert_eq!(
        diagnostic.message,
        "local evidence path exists but is not a file"
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn policy_reference_diagnostics_for_source_tree_applies_inventory_to_links() {
    let root = fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/trace.md"), "traceability")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture link file: {err}")));
    let entry = test_entry("allow-network", vec![], vec!["doc:docs/trace.md"]);
    let source_tree_files = BTreeSet::new();

    let references =
        policy_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(references.len(), 1);
    let reference = references
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one policy reference"));
    assert_eq!(reference.source, ReferenceSource::Link);
    assert_eq!(
        reference.diagnostic.status,
        EvidenceReferenceStatus::LocalFileMissing
    );
    assert_eq!(
        reference.diagnostic.message,
        DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE
    );
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn validate_evidence_references_for_source_tree_returns_ok_when_inventory_clean() {
    let root = fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/tracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(test_entry(
        "allow-network",
        vec!["doc:docs/tracked.md"],
        vec![],
    ));
    let mut source_tree_files = BTreeSet::new();
    source_tree_files.insert("docs/tracked.md".to_string());

    let result =
        validate_evidence_references_for_source_tree(&root, &cfg, Some(&source_tree_files));

    assert!(result.is_ok());
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn validate_evidence_references_for_source_tree_returns_err_for_missing_local_files() {
    let root = fixture_dir();
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture root dir: {err}")));
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(test_entry(
        "allow-network",
        vec!["doc:docs/missing.md"],
        vec![],
    ));
    let source_tree_files = BTreeSet::new();

    let err = validate_evidence_references_for_source_tree(&root, &cfg, Some(&source_tree_files))
        .expect_err("missing local evidence should fail validation");

    let message = err.to_string();
    assert!(message.contains("allow-network"));
    assert!(message.contains("doc:docs/missing.md"));
    assert!(message.contains("local evidence file is missing"));
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn validate_evidence_references_for_source_tree_returns_err_for_outside_inventory_evidence() {
    let root = fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(test_entry(
        "allow-network",
        vec!["doc:docs/untracked.md"],
        vec![],
    ));
    let source_tree_files = BTreeSet::new();

    let err = validate_evidence_references_for_source_tree(&root, &cfg, Some(&source_tree_files))
        .expect_err("untracked local evidence should fail default inventory validation");

    let message = err.to_string();
    assert!(message.contains("allow-network"));
    assert!(message.contains("doc:docs/untracked.md"));
    assert!(message.contains(DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE));
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}
