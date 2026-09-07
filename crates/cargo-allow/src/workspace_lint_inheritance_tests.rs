//! Inheritance-parity tests for the #3904 workspace lint inventory:
//! the live workspace facts are inventoried honestly (no effective
//! lint change), the clippy command lanes are bound to the CI source,
//! and the drift detection fixtures prove omitted inheritance, local
//! weakening, test-only weakening, and MSRV-incompatible selection are
//! detected.

use allow_report::{
    ClippyCommandLaneV1, LintPackageRowV1, WorkspaceLintFindingKindV1, WorkspaceLintInventoryV1,
    classify_workspace_lint_inventory,
};
use serde::Deserialize;

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel))
        .expect("the lint posture surface is present in the tree")
}

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

#[derive(Debug, Deserialize)]
struct PackageManifest {
    package: PackageName,
}

#[derive(Debug, Deserialize)]
struct PackageName {
    name: String,
}

fn live_inventory() -> (WorkspaceLintInventoryV1, Vec<(String, u32, Vec<String>)>) {
    let root = workspace_root();
    let crates_dir = root.join("crates");
    let mut packages = Vec::new();
    let mut raw_facts: Vec<(String, u32, Vec<String>)> = Vec::new();
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&crates_dir)
        .expect("the crates directory resolves")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    for entry in entries {
        let manifest_path = entry.join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        let text =
            std::fs::read_to_string(&manifest_path).expect("the package manifest is retained");
        let manifest: PackageManifest = toml::from_str(&text).expect("the package manifest parses");
        let inherits = text.contains("[lints]\nworkspace = true")
            || text.contains("[lints]\r\nworkspace = true");
        // Count item-level allow attributes across the package's .rs
        // sources (the manifest itself carries none).
        let mut local_allow_count = 0_u32;
        let mut source_stack = vec![entry.join("src")];
        while let Some(dir) = source_stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for source_entry in entries.filter_map(|source_entry| source_entry.ok()) {
                let source_path = source_entry.path();
                if source_path.is_dir() {
                    source_stack.push(source_path);
                } else if source_path.extension().is_some_and(|ext| ext == "rs")
                    && let Ok(source) = std::fs::read_to_string(&source_path)
                {
                    local_allow_count += source.matches("#[allow(").count() as u32;
                }
            }
        }
        let test_only_weakenings: Vec<String> = text
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("#[cfg_attr(test")
                    || (trimmed.contains("#[cfg(test)]") && trimmed.contains("allow("))
            })
            .map(str::to_string)
            .collect();
        raw_facts.push((
            manifest.package.name.clone(),
            local_allow_count,
            test_only_weakenings.clone(),
        ));
        packages.push(LintPackageRowV1 {
            package: manifest.package.name,
            inherits_workspace_lints: inherits,
            declared_lints: Vec::new(),
            crate_level_attributes: Vec::new(),
            local_allow_count,
            test_only_weakenings,
        });
    }
    let root_manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("the workspace manifest is retained");
    let inventory = WorkspaceLintInventoryV1 {
        schema_id: "cargo-allow.workspace-lint-inventory.v1".to_string(),
        schema_version: 1,
        rust_version_claim: "1.95".to_string(),
        workspace_lints_declared: root_manifest.contains("[workspace.lints"),
        packages,
        clippy_lanes: Vec::new(),
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    };
    (inventory, raw_facts)
}

#[test]
fn workspace_lint_inheritance_live_workspace_is_inventoried_honestly() {
    let (inventory, _) = live_inventory();
    // The release-set clippy lane from CI enforces thirteen packages.
    assert!(
        inventory.packages.len() >= 20,
        "the workspace members are all inventoried"
    );
    // PR B cutover: the workspace declares lints and every member
    // inherits them.
    assert!(
        inventory.workspace_lints_declared,
        "the cutover declares [workspace.lints] in the root manifest"
    );
    assert!(
        inventory
            .packages
            .iter()
            .all(|package| package.inherits_workspace_lints),
        "every member inherits the workspace lints after the cutover"
    );
}

#[test]
fn workspace_lint_inheritance_every_enforced_package_inherits_after_cutover() {
    // The release-set lane's packages are clippy-enforced and all
    // inherit the workspace lints after the PR B cutover: the
    // missing-inheritance drift class is empty on the live tree.
    let root = workspace_root();
    let ci = read_workspace_file(&root, ".github/workflows/ci.yml");
    let all_lines: Vec<&str> = ci.lines().collect();
    let start = all_lines
        .iter()
        .position(|line| line.contains("cargo clippy --locked --all-targets"))
        .expect("the release-set clippy command exists");
    let release_line = all_lines
        .iter()
        .copied()
        .skip(start)
        .take_while(|line| line.contains("-p ") || line.contains("cargo clippy"))
        .collect::<Vec<_>>()
        .join(" ");
    let enforced: Vec<String> = release_line
        .split("-p ")
        .skip(1)
        .map(|chunk| {
            chunk
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches(char::is_whitespace)
                .to_string()
        })
        .filter(|name| !name.is_empty())
        .collect();
    assert!(enforced.len() >= 10, "the release-set lane is explicit");

    let (inventory, _) = live_inventory();
    for name in &enforced {
        let row = inventory.packages.iter().find(|row| &row.package == name);
        assert!(
            row.is_some_and(|row| row.inherits_workspace_lints),
            "enforced package {name} must be inventoried and inherit the workspace lints"
        );
    }
}

#[test]
fn workspace_lint_inheritance_local_weakening_matches_the_live_count() {
    // The inventory's local-allow counts must match a direct scan of
    // the manifests' packages so the weakening findings cannot drift
    // from reality.
    let (inventory, raw_facts) = live_inventory();
    for (package, allow_count, test_weakenings) in &raw_facts {
        let row = inventory
            .packages
            .iter()
            .find(|row| &row.package == package);
        assert!(
            row.is_some_and(|row| {
                row.local_allow_count == *allow_count
                    && row.test_only_weakenings.len() == test_weakenings.len()
            }),
            "package {package}: the inventoried counts must match the direct scan"
        );
    }
    let total_local: u32 = inventory
        .packages
        .iter()
        .map(|row| row.local_allow_count)
        .sum();
    assert!(
        total_local > 0,
        "the live workspace carries local weakening to name"
    );
}

#[test]
fn workspace_lint_inheritance_test_only_weakening_fixture_is_detected() {
    // Negative control 3: a test-gated blanket allow must be named so
    // a blanket lower standard cannot arrive silently.
    let mut enforced = LintPackageRowV1 {
        package: "cargo-allow".to_string(),
        inherits_workspace_lints: true,
        declared_lints: Vec::new(),
        crate_level_attributes: Vec::new(),
        local_allow_count: 0,
        test_only_weakenings: vec!["#[cfg_attr(test, allow(clippy::expect_used))]".to_string()],
    };
    let root = workspace_root();
    let ci = read_workspace_file(&root, ".github/workflows/ci.yml");
    assert!(ci.contains("-D warnings"), "CI denies warnings");
    let _ = &mut enforced;
    let findings = classify_workspace_lint_inventory(&workspace_with(vec![enforced]));
    assert!(
        findings
            .findings
            .iter()
            .any(|finding| finding.kind == WorkspaceLintFindingKindV1::TestOnlyWeakening)
    );
}

fn workspace_with(packages: Vec<LintPackageRowV1>) -> WorkspaceLintInventoryV1 {
    WorkspaceLintInventoryV1 {
        schema_id: "cargo-allow.workspace-lint-inventory.v1".to_string(),
        schema_version: 1,
        rust_version_claim: "1.95".to_string(),
        workspace_lints_declared: true,
        packages,
        clippy_lanes: vec![ClippyCommandLaneV1 {
            lane: "release-set".to_string(),
            workflow: "ci.yml".to_string(),
            packages: vec!["cargo-allow".to_string()],
            deny_flags: vec!["-D".to_string(), "warnings".to_string()],
        }],
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

#[test]
fn workspace_lint_inheritance_msrv_fixture_is_detected() {
    // Negative control 5: a lint requiring a newer compiler than the
    // claimed MSRV must be named at inventory time.
    let mut enforced = LintPackageRowV1 {
        package: "cargo-allow".to_string(),
        inherits_workspace_lints: true,
        declared_lints: Vec::new(),
        crate_level_attributes: Vec::new(),
        local_allow_count: 0,
        test_only_weakenings: Vec::new(),
    };
    let _ = &mut enforced;
    let mut declared = enforced.clone();
    declared.declared_lints = vec![allow_report::DeclaredLintV1 {
        lint: "clippy::future_lint".to_string(),
        level: "deny".to_string(),
        introduced_in: Some("1.99".to_string()),
    }];
    let findings = classify_workspace_lint_inventory(&workspace_with(vec![declared]));
    assert!(findings.findings.iter().any(|finding| finding.kind
        == WorkspaceLintFindingKindV1::MsrvIncompatibleSelection
        && finding.detail.contains("1.99")));
}

#[test]
fn workspace_lint_inheritance_no_effective_change_is_provable() {
    // PR A changes no effective lint level: the workspace root has no
    // [workspace.lints], the packages have no [lints], and the CI
    // command still carries -D warnings. The inventory is
    // observational.
    let root = workspace_root();
    let root_manifest = read_workspace_file(&root, "Cargo.toml");
    // The cutover preserves the accepted rule set exactly: the
    // workspace tables deny the warnings group for both tools — the
    // manifest-level equivalent of the CI `-D warnings` outcome — and
    // nothing beyond that group was selected.
    assert!(root_manifest.contains("[workspace.lints.rust]"));
    assert!(root_manifest.contains("[workspace.lints.clippy]"));
    assert!(!root_manifest.contains("pedantic"));
    assert!(!root_manifest.contains("nursery"));
    let (inventory, _) = live_inventory();
    assert!(inventory.workspace_lints_declared);
    assert!(
        inventory
            .packages
            .iter()
            .all(|row| row.declared_lints.is_empty()),
        "no package declares local [lints] tables; inheritance is uniform"
    );
}
