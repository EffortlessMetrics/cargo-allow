use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use allow_core::{AllowConfig, FindingKind, MatchStatus};

use super::{list_rows, list_rows_with_source_tree_files};
use crate::list::test_support::{test_entry, test_outcome};

fn list_fixture_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "cargo-allow-list-rows-tests-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    path
}

#[test]
fn entry_reference_diagnostics_for_source_tree_call_presence_observer() {
    let root = list_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-diagnostics", FindingKind::Unsafe);
    entry.evidence = vec!["doc:docs/untracked.md".to_string()];
    cfg.allow.push(entry);
    let source_tree_files = BTreeSet::new();

    let rows = list_rows_with_source_tree_files(&root, &cfg, &[], &[], Some(&source_tree_files));

    match rows.as_slice() {
        [row] => {
            assert_eq!(row.id, "allow-diagnostics");
            assert_eq!(row.broken_evidence_references, 1);
        }
        other => assert_eq!(other.len(), 1),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_entry_status_call_presence_observer() {
    let mut cfg = AllowConfig::empty();
    let entry = test_entry("allow-new", FindingKind::Panic);
    cfg.allow.push(entry);
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        Some("allow-new"),
        None,
        "new finding",
    )];

    let rows = list_rows(Path::new("."), &cfg, &[], &outcomes);

    match rows.as_slice() {
        [row] => assert_eq!(row.status, MatchStatus::New),
        other => assert_eq!(other.len(), 1),
    }
}
