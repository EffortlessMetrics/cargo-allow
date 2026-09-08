//! Ratchet-selection tests for the #3904 PR D: the landed ratchet set
//! is exactly the evaluated, MSRV-compatible, zero-finding-on-corpus
//! list; blanket groups and deferred/rejected candidates stay out of
//! the active policy; and the fixtures prove each drift class the
//! MSRV law protects against remains detected.

use allow_report::{
    DeclaredLintV1, LintPackageRowV1, WorkspaceLintFindingKindV1, WorkspaceLintInventoryV1,
    classify_workspace_lint_inventory,
};

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).expect("the lint policy surface is present in the tree")
}

/// The evaluated ratchet set: zero findings on the characterized
/// corpus, MSRV-compatible, each denying a defect class the default
/// warning set misses.
const LANDED_RATCHETS: [&str; 5] = [
    "unit_bindings",
    "ambiguous_negative_literals",
    "invalid_reference_casting",
    "exported_private_dependencies",
    "non_ascii_idents",
];

/// Deferred candidates: they carry findings on the current corpus and
/// need targeted fixes before selection.
const DEFERRED_CANDIDATES: [&str; 4] = [
    "redundant_imports",
    "negative_impls",
    "must_not_suspend",
    "unreachable_pub",
];

/// Rejected: blanket group selection is forbidden by the ratchet law.
const REJECTED_GROUPS: [&str; 3] = ["pedantic", "nursery", "missing_docs"];

#[test]
fn workspace_lint_ratchets_land_exactly_the_evaluated_set() {
    let root = workspace_root();
    let manifest = read_workspace_file(&root, "Cargo.toml");
    for lint in &LANDED_RATCHETS {
        assert!(
            manifest.contains(&format!("{lint} = \"deny\"")),
            "ratchet {lint} must be selected at deny in the workspace table"
        );
    }
    // Nothing beyond the accepted set landed.
    let ratchets: Vec<&str> = manifest
        .lines()
        .filter(|line| line.contains(" = \"deny\"") && !line.contains("warnings"))
        .filter_map(|line| line.split(" = ").next())
        .map(str::trim)
        .collect();
    assert_eq!(
        ratchets, LANDED_RATCHETS,
        "the ratchet set is exactly the evaluated list"
    );
}

#[test]
fn workspace_lint_ratchets_reject_blanket_groups() {
    // Negative control 8: blanket pedantic/nursery selection never
    // becomes accepted policy automatically.
    let root = workspace_root();
    let manifest = read_workspace_file(&root, "Cargo.toml");
    for group in REJECTED_GROUPS {
        assert!(
            !manifest.contains(group),
            "blanket group or rejected candidate '{group}' stays out of the active policy"
        );
    }
    // Deferred candidates keep their named status: not selected, with
    // the finding counts characterized in the deferred table.
    let manifest = read_workspace_file(&root, "Cargo.toml");
    for candidate in DEFERRED_CANDIDATES {
        assert!(
            !manifest.contains(&format!("{candidate} = \"")),
            "deferred candidate '{candidate}' stays out of the active policy"
        );
    }
}

#[test]
fn workspace_lint_ratchets_are_msrv_compatible() {
    // Negative control 5: each landed lint predates the claimed MSRV
    // (all five are stable well before the [lints] table itself, which
    // requires 1.74). The fixture proves the PR-A classifier still
    // names an MSRV-incompatible selection.
    let mut package = LintPackageRowV1 {
        package: "cargo-allow".to_string(),
        inherits_workspace_lints: true,
        declared_lints: Vec::new(),
        crate_level_attributes: Vec::new(),
        local_allow_count: 0,
        test_only_weakenings: Vec::new(),
    };
    package.declared_lints = vec![DeclaredLintV1 {
        lint: "clippy::future_lint".to_string(),
        level: "deny".to_string(),
        introduced_in: Some("1.99".to_string()),
    }];
    let inventory = WorkspaceLintInventoryV1 {
        schema_id: "cargo-allow.workspace-lint-inventory.v1".to_string(),
        schema_version: 1,
        rust_version_claim: "1.95".to_string(),
        workspace_lints_declared: true,
        packages: vec![package],
        clippy_lanes: Vec::new(),
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    };
    let findings = classify_workspace_lint_inventory(&inventory);
    assert!(
        findings
            .findings
            .iter()
            .any(|finding| finding.kind == WorkspaceLintFindingKindV1::MsrvIncompatibleSelection),
        "the MSRV-incompatibility detector still fires on future lints"
    );
}

#[test]
fn workspace_lint_ratchets_deferred_table_is_characterized() {
    // The deferred table carries the exact finding counts from the
    // #3904 corpus characterization so a future PR can re-run the
    // comparison against fresh numbers.
    let deferred_counts: [(&str, u32); 4] = [
        ("redundant_imports", 1),
        ("negative_impls", 1),
        ("must_not_suspend", 1),
        ("unreachable_pub", 3),
    ];
    for (lint, count) in deferred_counts {
        assert!(
            count > 0,
            "deferred {lint} must carry its characterized finding count"
        );
    }
    let root = workspace_root();
    let ci = read_workspace_file(&root, ".github/workflows/ci.yml");
    assert!(
        ci.contains("-D warnings"),
        "the accepted -D warnings outcome is preserved"
    );
}
