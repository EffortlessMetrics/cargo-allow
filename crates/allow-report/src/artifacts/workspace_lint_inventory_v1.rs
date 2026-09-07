//! Workspace lint inventory (#3904 PR A).
//!
//! One deterministic inventory of the workspace's Rust and Clippy lint
//! posture: which packages inherit workspace lints, what they declare
//! locally, which crate-level and item-level weakening attributes
//! exist, and which CI clippy command lanes enforce which packages.
//! The inventory is observational — it changes no effective lint
//! level. Its findings name the drift classes the later cutover and
//! drift-guard PRs close:
//!
//! - missing workspace inheritance (a clippy-enforced package without
//!   `lints.workspace = true`);
//! - local weakening (an `#[allow(...)]` in non-test source, which
//!   weakens the command-line `-D warnings` posture without a
//!   cargo-allow receipt binding);
//! - test-only weakening (a test-gated blanket allow);
//! - MSRV-incompatible selection (a declared lint requiring a newer
//!   compiler than the workspace's `rust-version` claim);
//! - command-line drift (a package no clippy lane enforces).
//!
//! Claim boundary: observational inventory and classification only.
//! It owns no lint levels, replaces no CI command, and does not
//! approve exceptions — cargo-allow stays the exception-ledger
//! authority.

use serde::{Deserialize, Serialize};

pub const WORKSPACE_LINT_INVENTORY_SCHEMA_ID: &str = "cargo-allow.workspace-lint-inventory.v1";
pub const WORKSPACE_LINT_INVENTORY_SCHEMA_VERSION: u32 = 1;

const CLAIM_BOUNDARY: &str = "A deterministic observational inventory of workspace Rust and Clippy lint posture. It names drift classes for the later cutover and drift-guard PRs; it changes no effective lint level, enables no lint, weakens no lint, approves no exception, and does not replace the CI clippy commands or the cargo-allow exception ledger.";

/// One declared lint on a package (from a local `[lints]` table or a
/// crate-level attribute), with the toolchain that introduced it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredLintV1 {
    /// Lint path, e.g. `clippy::unwrap_used`.
    pub lint: String,
    /// Level: `deny`, `warn`, `allow`, `expect`, or `forbid`.
    pub level: String,
    /// The toolchain release that introduced the lint, when known
    /// (e.g. `1.75`). `None` means the requirement is uncharacterized.
    pub introduced_in: Option<String>,
}

/// Inventory facts for one package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintPackageRowV1 {
    pub package: String,
    /// The package declares `[lints] workspace = true`.
    pub inherits_workspace_lints: bool,
    /// Local `[lints]` declarations (package-specific policy).
    #[serde(default)]
    pub declared_lints: Vec<DeclaredLintV1>,
    /// Crate-level (`#![...]`) lint attributes verbatim.
    #[serde(default)]
    pub crate_level_attributes: Vec<String>,
    /// Item-level `#[allow(...)]` occurrences in non-test source.
    pub local_allow_count: u32,
    /// Test-gated blanket allows (`#[cfg_attr(test, allow(...))]` or a
    /// `#[cfg(test)]`-gated blanket module allow).
    #[serde(default)]
    pub test_only_weakenings: Vec<String>,
}

/// One CI clippy command lane.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClippyCommandLaneV1 {
    /// The product family the CI job names.
    pub lane: String,
    pub workflow: String,
    #[serde(default)]
    pub packages: Vec<String>,
    /// The flags after `--` (e.g. `["-D", "warnings"]`).
    #[serde(default)]
    pub deny_flags: Vec<String>,
}

/// The assembled inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLintInventoryV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// The workspace's claimed `rust-version`.
    pub rust_version_claim: String,
    /// True when `[workspace.lints]` exists in the root manifest.
    pub workspace_lints_declared: bool,
    #[serde(default)]
    pub packages: Vec<LintPackageRowV1>,
    #[serde(default)]
    pub clippy_lanes: Vec<ClippyCommandLaneV1>,
    #[serde(default)]
    pub limits: Vec<String>,
    pub claim_boundary: String,
}

/// The drift classes the classifier names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceLintFindingKindV1 {
    MissingWorkspaceInheritance,
    LocalWeakening,
    TestOnlyWeakening,
    MsrvIncompatibleSelection,
    CommandLineDrift,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLintFindingV1 {
    pub kind: WorkspaceLintFindingKindV1,
    pub package: String,
    pub detail: String,
}

/// The classified findings, in deterministic order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLintFindingsV1 {
    pub schema_id: String,
    pub schema_version: u32,
    #[serde(default)]
    pub findings: Vec<WorkspaceLintFindingV1>,
    pub claim_boundary: String,
}

/// Parse a `major.minor` (or `major.minor.patch`) toolchain string.
fn parse_toolchain(value: &str) -> Option<(u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor))
}

/// Classify the inventory: pure, deterministic, ordered by (kind,
/// package, detail).
#[must_use]
pub fn classify_workspace_lint_inventory(
    inventory: &WorkspaceLintInventoryV1,
) -> WorkspaceLintFindingsV1 {
    let mut findings: Vec<WorkspaceLintFindingV1> = Vec::new();
    let claimed = parse_toolchain(&inventory.rust_version_claim);

    let enforced: Vec<&str> = inventory
        .clippy_lanes
        .iter()
        .flat_map(|lane| lane.packages.iter().map(String::as_str))
        .collect();

    let mut sorted_packages: Vec<&LintPackageRowV1> = inventory.packages.iter().collect();
    sorted_packages.sort_by(|a, b| a.package.cmp(&b.package));

    for package in sorted_packages {
        let enforced_here = enforced.contains(&package.package.as_str());

        // Missing inheritance: a clippy-enforced package whose manifest
        // does not inherit the workspace lint table.
        if enforced_here && !package.inherits_workspace_lints {
            findings.push(WorkspaceLintFindingV1 {
                kind: WorkspaceLintFindingKindV1::MissingWorkspaceInheritance,
                package: package.package.clone(),
                detail: "the package is clippy-enforced but does not declare [lints] \
                         workspace = true"
                    .to_string(),
            });
        }

        // Command-line drift: a package that no clippy lane enforces.
        if !enforced_here {
            findings.push(WorkspaceLintFindingV1 {
                kind: WorkspaceLintFindingKindV1::CommandLineDrift,
                package: package.package.clone(),
                detail: "no CI clippy command names this package".to_string(),
            });
        }

        // Local weakening: every item-level allow in non-test source
        // weakens the command-line -D warnings posture. Receipt binding
        // is cargo-allow's authority, so the finding only names the
        // attribute count and leaves approval to the ledger.
        if package.local_allow_count > 0 && enforced_here {
            findings.push(WorkspaceLintFindingV1 {
                kind: WorkspaceLintFindingKindV1::LocalWeakening,
                package: package.package.clone(),
                detail: format!(
                    "{} item-level allow attribute(s) in non-test source",
                    package.local_allow_count
                ),
            });
        }

        // Test-only weakening: test-gated blanket allows must be named
        // so a blanket lower standard cannot arrive silently.
        for attribute in &package.test_only_weakenings {
            findings.push(WorkspaceLintFindingV1 {
                kind: WorkspaceLintFindingKindV1::TestOnlyWeakening,
                package: package.package.clone(),
                detail: format!("test-gated blanket allow: {attribute}"),
            });
        }

        // MSRV-incompatible selection: a declared lint introduced after
        // the claimed rust-version.
        for declared in &package.declared_lints {
            let Some(introduced) = &declared.introduced_in else {
                continue;
            };
            let (Some((lint_major, lint_minor)), Some((claim_major, claim_minor))) =
                (parse_toolchain(introduced), claimed)
            else {
                continue;
            };
            if (lint_major, lint_minor) > (claim_major, claim_minor) {
                findings.push(WorkspaceLintFindingV1 {
                    kind: WorkspaceLintFindingKindV1::MsrvIncompatibleSelection,
                    package: package.package.clone(),
                    detail: format!(
                        "{} requires {} but the workspace claims rust-version {}.{}",
                        declared.lint, introduced, claim_major, claim_minor
                    ),
                });
            }
        }
    }

    findings.sort();

    WorkspaceLintFindingsV1 {
        schema_id: WORKSPACE_LINT_INVENTORY_SCHEMA_ID.to_string(),
        schema_version: WORKSPACE_LINT_INVENTORY_SCHEMA_VERSION,
        findings,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of the classified findings. Deterministic.
#[must_use]
pub fn render_workspace_lint_findings_human(findings: &WorkspaceLintFindingsV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "workspace-lint-inventory: {} finding(s)",
        findings.findings.len()
    ));
    for finding in &findings.findings {
        lines.push(format!(
            "  {:?} {}: {}",
            finding.kind, finding.package, finding.detail
        ));
    }
    lines.push(format!("  claim boundary: {}", findings.claim_boundary));
    lines.join("\n")
}

/// JSON view of the classified findings.
pub fn render_workspace_lint_findings_json(
    findings: &WorkspaceLintFindingsV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(findings)
}
