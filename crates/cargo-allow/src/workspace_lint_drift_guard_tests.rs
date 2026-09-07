//! Drift-guard law tests for the #3904 PR C: every drift class fires,
//! explicit exceptions own legitimate differences, expired and
//! unreasoned exceptions are themselves drift, ordering is
//! deterministic, and the views derive from one report.

use allow_report::{
    LintPackageRowV1, WorkspaceLintDriftClassV1, WorkspaceLintDriftReportV1,
    WorkspaceLintExceptionScopeV1, WorkspaceLintExceptionV1, WorkspaceLocalLintRowV1,
    evaluate_workspace_lint_drift, render_workspace_lint_drift_human,
    render_workspace_lint_drift_json,
};

/// The evaluation window's current date for these tests.
const TODAY: &str = "2026-09-07";

fn package(name: &str) -> LintPackageRowV1 {
    LintPackageRowV1 {
        package: name.to_string(),
        inherits_workspace_lints: true,
        declared_lints: Vec::new(),
        crate_level_attributes: Vec::new(),
        local_allow_count: 0,
        test_only_weakenings: Vec::new(),
    }
}

fn exception(
    package: &str,
    scope: WorkspaceLintExceptionScopeV1,
    reason: &str,
    expires_on: Option<&str>,
) -> WorkspaceLintExceptionV1 {
    WorkspaceLintExceptionV1 {
        package: package.to_string(),
        scope,
        reason: reason.to_string(),
        expires_on: expires_on.map(str::to_string),
    }
}

fn grade(
    packages: &[LintPackageRowV1],
    local_lints: &[WorkspaceLocalLintRowV1],
    enforced: &[String],
    exceptions: &[WorkspaceLintExceptionV1],
) -> WorkspaceLintDriftReportV1 {
    evaluate_workspace_lint_drift(packages, local_lints, enforced, exceptions, TODAY)
}

#[test]
fn workspace_lint_drift_guard_clean_tree_reports_clean() {
    let report = grade(
        &[package("cargo-allow"), package("allow-core")],
        &[],
        &["cargo-allow".to_string(), "allow-core".to_string()],
        &[],
    );
    assert!(report.clean);
    assert!(report.findings.is_empty());
}

#[test]
fn workspace_lint_drift_guard_names_missing_inheritance() {
    let mut orphan = package("allow-core");
    orphan.inherits_workspace_lints = false;
    let report = grade(
        &[package("cargo-allow"), orphan],
        &[],
        &["cargo-allow".to_string(), "allow-core".to_string()],
        &[],
    );
    assert!(!report.clean);
    assert!(report.findings.iter().any(|finding| finding.class
        == WorkspaceLintDriftClassV1::MissingInheritance
        && finding.package == "allow-core"));
}

#[test]
fn workspace_lint_drift_guard_explicit_exception_owns_the_difference() {
    let mut orphan = package("allow-core");
    orphan.inherits_workspace_lints = false;
    let report = grade(
        &[package("cargo-allow"), orphan],
        &[],
        &["cargo-allow".to_string(), "allow-core".to_string()],
        &[exception(
            "allow-core",
            WorkspaceLintExceptionScopeV1::Inheritance,
            "fixture crate: the scan-characterization fixtures assert local lint posture",
            None,
        )],
    );
    assert!(report.clean);
}

#[test]
fn workspace_lint_drift_guard_names_unowned_weakening_and_owns_it() {
    let mut weakening = package("cargo-allow");
    weakening.local_allow_count = 5;
    let unowned = grade(
        std::slice::from_ref(&weakening),
        &[],
        &["cargo-allow".to_string()],
        &[],
    );
    assert!(unowned.findings.iter().any(|finding| finding.class
        == WorkspaceLintDriftClassV1::UnownedWeakening
        && finding.detail.contains("5 item-level allow attribute(s)")));
    let owned = grade(
        std::slice::from_ref(&weakening),
        &[],
        &["cargo-allow".to_string()],
        &[exception(
            "cargo-allow",
            WorkspaceLintExceptionScopeV1::Weakening,
            "the release-set lane's receipted fixture allows",
            None,
        )],
    );
    assert!(owned.clean);
}

#[test]
fn workspace_lint_drift_guard_names_weakening_local_lints() {
    let local = WorkspaceLocalLintRowV1 {
        package: "cargo-allow".to_string(),
        lint: "clippy::unwrap_used".to_string(),
        level: "allow".to_string(),
    };
    let weakening = grade(
        &[package("cargo-allow")],
        std::slice::from_ref(&local),
        &["cargo-allow".to_string()],
        &[],
    );
    assert!(
        weakening
            .findings
            .iter()
            .any(|finding| finding.class == WorkspaceLintDriftClassV1::WeakeningLocalLints)
    );
    // A strengthening (deny) local declaration is not a weakening.
    let strengthening = WorkspaceLocalLintRowV1 {
        package: "cargo-allow".to_string(),
        lint: "clippy::unwrap_used".to_string(),
        level: "deny".to_string(),
    };
    let strengthening_report = grade(
        &[package("cargo-allow")],
        std::slice::from_ref(&strengthening),
        &["cargo-allow".to_string()],
        &[],
    );
    assert!(strengthening_report.findings.is_empty());
}

#[test]
fn workspace_lint_drift_guard_names_conflicting_policy() {
    let mut conflicting = package("cargo-allow");
    conflicting.declared_lints = vec![
        allow_report::DeclaredLintV1 {
            lint: "clippy::unwrap_used".to_string(),
            level: "warn".to_string(),
            introduced_in: None,
        },
        allow_report::DeclaredLintV1 {
            lint: "clippy::unwrap_used".to_string(),
            level: "deny".to_string(),
            introduced_in: None,
        },
    ];
    let report = grade(
        std::slice::from_ref(&conflicting),
        &[],
        &["cargo-allow".to_string()],
        &[],
    );
    assert!(report.findings.iter().any(|finding| finding.class
        == WorkspaceLintDriftClassV1::ConflictingPolicy
        && finding.detail.contains("conflicting levels")));
}

#[test]
fn workspace_lint_drift_guard_expires_migration_debt() {
    let mut orphan = package("allow-core");
    orphan.inherits_workspace_lints = false;
    let expired_exception = exception(
        "allow-core",
        WorkspaceLintExceptionScopeV1::Inheritance,
        "migration debt: fixture cutover tracked in the drift-guard follow-up",
        Some("2026-06-01"),
    );
    let dated = grade(
        &[package("cargo-allow"), orphan.clone()],
        &[],
        &["cargo-allow".to_string(), "allow-core".to_string()],
        &[expired_exception],
    );
    assert!(dated.findings.iter().any(|finding| finding.class
        == WorkspaceLintDriftClassV1::ExpiredException
        && finding.package == "allow-core"));
    let valid_exception = exception(
        "allow-core",
        WorkspaceLintExceptionScopeV1::Inheritance,
        "migration debt",
        Some("2099-12-31"),
    );
    let valid = grade(
        &[package("cargo-allow"), orphan],
        &[],
        &["cargo-allow".to_string(), "allow-core".to_string()],
        &[valid_exception],
    );
    assert!(
        !valid
            .findings
            .iter()
            .any(|finding| finding.class == WorkspaceLintDriftClassV1::ExpiredException)
    );
}

#[test]
fn workspace_lint_drift_guard_names_unreasoned_exceptions() {
    let whitespace_reason = exception(
        "cargo-allow",
        WorkspaceLintExceptionScopeV1::Weakening,
        "   ",
        None,
    );
    let unreasoned = grade(
        &[package("cargo-allow")],
        &[],
        &["cargo-allow".to_string()],
        &[whitespace_reason],
    );
    assert!(
        unreasoned
            .findings
            .iter()
            .any(|finding| finding.class == WorkspaceLintDriftClassV1::UnreasonedException)
    );
}

#[test]
fn workspace_lint_drift_guard_order_is_deterministic() {
    let mut orphan = package("allow-core");
    orphan.inherits_workspace_lints = false;
    let mut weakening = package("cargo-allow");
    weakening.local_allow_count = 2;
    let first = grade(
        &[orphan.clone(), weakening.clone()],
        &[],
        &["cargo-allow".to_string(), "allow-core".to_string()],
        &[],
    );
    let second = grade(
        &[weakening.clone(), orphan.clone()],
        &[],
        &["cargo-allow".to_string(), "allow-core".to_string()],
        &[],
    );
    assert_eq!(first, second);
    let json = render_workspace_lint_drift_json(&first).expect("serialization succeeds");
    let roundtrip: WorkspaceLintDriftReportV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, first);
    let human = render_workspace_lint_drift_human(&first);
    assert!(human.contains("workspace-lint-drift-guard: drift"));
    assert!(human.contains("claim boundary:"));
}

#[test]
fn workspace_lint_drift_guard_live_tree_is_clean_with_owned_weakening() {
    // Live enforcement: grade all 22 member packages against the
    // cutover state. The item-level allows each package carries are
    // owned by explicit weakening exceptions — they are receipted in
    // the cargo-allow ledger by the no-new guard, which is the
    // ownership evidence the exception records.
    let root = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
    )
    .join("../..")
    .canonicalize()
    .expect("workspace root resolves");
    let crates_dir = root.join("crates");
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&crates_dir)
        .expect("the crates directory resolves")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    let mut packages = Vec::new();
    let mut owned_weakening: Vec<WorkspaceLintExceptionV1> = Vec::new();
    for entry in &entries {
        let manifest_path = entry.join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest_text =
            std::fs::read_to_string(&manifest_path).expect("the manifest is retained");
        let package_name = manifest_text
            .lines()
            .find_map(|line| line.strip_prefix("name = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("every member manifest declares a name")
            .to_string();
        let mut local_allow_count = 0_u32;
        let mut source_stack = vec![entry.join("src")];
        while let Some(dir) = source_stack.pop() {
            let Ok(dir_entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for source_entry in dir_entries.filter_map(|source_entry| source_entry.ok()) {
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
        packages.push(allow_report::LintPackageRowV1 {
            package: package_name.clone(),
            inherits_workspace_lints: manifest_text.contains("[lints]")
                && manifest_text.contains("workspace = true"),
            declared_lints: Vec::new(),
            crate_level_attributes: Vec::new(),
            local_allow_count,
            test_only_weakenings: Vec::new(),
        });
        if local_allow_count > 0 {
            owned_weakening.push(WorkspaceLintExceptionV1 {
                package: package_name,
                scope: WorkspaceLintExceptionScopeV1::Weakening,
                reason: "the package's item-level allow attributes are receipted in the \
                         cargo-allow ledger by the no-new guard"
                    .to_string(),
                expires_on: None,
            });
        }
    }
    assert!(packages.len() >= 22, "all 22 members are inventoried");
    let enforced: Vec<String> = packages.iter().map(|row| row.package.clone()).collect();
    // The checked-in, explicitly reviewed weakening exception set: these
    // packages' item-level allows are receipted in the cargo-allow ledger
    // by the no-new guard. A package gaining allows without joining this
    // reviewed set stays unowned drift and fails the guard.
    let owned: Vec<String> = vec![
        "allow-policy".to_string(),
        "allow-report".to_string(),
        "allow-rust".to_string(),
        "cargo-allow".to_string(),
    ];
    let exceptions: Vec<WorkspaceLintExceptionV1> = owned
        .iter()
        .map(|package| WorkspaceLintExceptionV1 {
            package: package.clone(),
            scope: WorkspaceLintExceptionScopeV1::Weakening,
            reason: "the package's item-level allow attributes are receipted in the \
                     cargo-allow ledger by the no-new guard"
                .to_string(),
            expires_on: None,
        })
        .collect();
    let report = evaluate_workspace_lint_drift(&packages, &[], &enforced, &exceptions, TODAY);
    assert!(
        report.clean,
        "the live tree must satisfy the cutover law with the reviewed \
         weakening set: {:?}",
        report.findings
    );

    // Negative live-style fixture: an existing member package gaining
    // item-level allows without joining the reviewed set stays unowned
    // drift.
    let mut newcomer = package("allow-core");
    newcomer.local_allow_count = 4;
    let mut all = packages.clone();
    all.push(newcomer);
    let unowned = evaluate_workspace_lint_drift(&all, &[], &enforced, &exceptions, TODAY);
    assert!(
        unowned.findings.iter().any(|finding| finding.class
            == WorkspaceLintDriftClassV1::UnownedWeakening
            && finding.package == "allow-core"),
        "a newly introduced unowned allow must fail the guard"
    );
}
