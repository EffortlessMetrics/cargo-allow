//! Unit tests for the #4646 enforcing-completeness gate predicate. Sibling
//! test module for `check.rs` per the crate's command-test layout convention.

use super::check_selected_input_incomplete;
use crate::InventoryFacts;
use allow_inventory::{InventoryCompleteness, InventorySource};
use std::path::PathBuf;

fn complete_fallback_facts() -> InventoryFacts {
    // A complete filesystem fallback is a supported source-only route
    // (#4646): fallback provenance alone must not fail enforcement.
    InventoryFacts::scanned(InventorySource::FilesystemFallback, 3)
        .with_completeness(InventoryCompleteness::Fallback)
}

#[test]
fn complete_and_scoped_inventories_with_clean_scans_pass() {
    for completeness in [
        InventoryCompleteness::Complete,
        InventoryCompleteness::Scoped,
        InventoryCompleteness::Fallback,
    ] {
        let facts =
            InventoryFacts::scanned(InventorySource::GitTracked, 3).with_completeness(completeness);
        assert!(
            !check_selected_input_incomplete(&facts),
            "completeness {} with zero omissions must read as complete",
            completeness.as_str()
        );
    }
}

#[test]
fn partial_completeness_fails_the_predicate() {
    let facts = InventoryFacts::scanned(InventorySource::GitTracked, 3)
        .with_completeness(InventoryCompleteness::Partial);
    assert!(check_selected_input_incomplete(&facts));
}

#[test]
fn truncated_fallback_walk_fails_even_without_partial_marker() {
    // Defense in depth (#4646): if a producer carried the omission
    // count without the Partial completeness marker, the gate must
    // still fail closed.
    let mut facts = complete_fallback_facts();
    facts.inventory_skipped_paths = 1;
    assert!(check_selected_input_incomplete(&facts));
}

#[test]
fn scanner_skips_and_parse_errors_fail_the_predicate() {
    let skipped = complete_fallback_facts().with_rust_files_skipped(1);
    assert!(check_selected_input_incomplete(&skipped));
    let parse_errors = complete_fallback_facts().with_rust_files_with_parse_errors(1);
    assert!(check_selected_input_incomplete(&parse_errors));
}

#[test]
fn scanned_inventory_preserves_inventory_skip_counts() {
    // A truncated fallback inventory must keep its omission count in
    // the facts the gate reads (#4646); provenance stays in `source`.
    let inventory = allow_inventory::Inventory {
        files: vec![PathBuf::from("visible.txt")],
        source: InventorySource::FilesystemFallback,
        completeness: InventoryCompleteness::Partial,
        empty_git_tracked: false,
        deleted_tracked: Vec::new(),
        inaccessible_paths: Vec::new(),
        git_error: Some("not a repository".to_string()),
        skipped_paths: vec![PathBuf::from("deep/.cargo-allow-inventory-depth-limit-64")],
        submodule_paths: Vec::new(),
        reparse_skipped_paths: Vec::new(),
    };
    let facts = InventoryFacts::scanned_inventory(&inventory);
    assert_eq!(facts.inventory_skipped_paths, 1);
    assert_eq!(facts.source, InventorySource::FilesystemFallback);
    assert!(check_selected_input_incomplete(&facts));
}

#[test]
fn ignored_subtree_scope_keeps_the_predicate_clean() {
    // #4646 review: skips inside policy-ignored subtrees are filtered at the
    // inventory boundary, so the facts a fully-ignored-truncation walk
    // produces must read complete for the enforcing gate.
    let inventory = allow_inventory::Inventory {
        files: vec![PathBuf::from("visible.txt")],
        source: InventorySource::FilesystemFallback,
        completeness: InventoryCompleteness::Fallback,
        empty_git_tracked: false,
        deleted_tracked: Vec::new(),
        inaccessible_paths: Vec::new(),
        git_error: Some("not a repository".to_string()),
        skipped_paths: Vec::new(),
        submodule_paths: Vec::new(),
        reparse_skipped_paths: Vec::new(),
    };
    let facts = InventoryFacts::scanned_inventory(&inventory);
    assert_eq!(facts.inventory_skipped_paths, 0);
    assert!(!check_selected_input_incomplete(&facts));
}

#[test]
fn reparse_alias_skips_fail_the_predicate() {
    // #4677: a junctioned alias traversal is a recorded omission. Even if a
    // producer carried the disclosure count without the Partial completeness
    // marker, the gate must fail closed the same way it does for walk skips.
    let mut facts = complete_fallback_facts();
    facts.reparse_skipped_paths = 1;
    assert!(check_selected_input_incomplete(&facts));
}

#[test]
fn scanned_inventory_preserves_reparse_skip_counts() {
    // #4677: the junction alias disclosure must survive the Inventory ->
    // InventoryFacts projection so the gate and the coverage reason both read it.
    let inventory = allow_inventory::Inventory {
        files: vec![PathBuf::from("visible.txt")],
        source: InventorySource::GitTracked,
        completeness: InventoryCompleteness::Partial,
        empty_git_tracked: false,
        deleted_tracked: Vec::new(),
        inaccessible_paths: Vec::new(),
        git_error: None,
        skipped_paths: Vec::new(),
        submodule_paths: Vec::new(),
        reparse_skipped_paths: vec![PathBuf::from("linked_src/lib.rs")],
    };
    let facts = InventoryFacts::scanned_inventory(&inventory);
    assert_eq!(facts.reparse_skipped_paths, 1);
    assert!(check_selected_input_incomplete(&facts));
}
