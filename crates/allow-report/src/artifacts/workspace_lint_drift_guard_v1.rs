//! Workspace lint drift guard (#3904 PR C).
//!
//! Grades the lint inventory (PR A) against the cutover state (PR B)
//! using the explicit exception ledger (PR C): every clippy-enforced
//! package must either inherit the workspace lints or carry an
//! explicit package-specific exception with a reason; any package-local
//! `[lints]` table must itself be an explicit exception and may not
//! weaken a workspace-denied lint; item-level weakening in non-test
//! source must be owned by an exception or be visible to cargo-allow's
//! ledger; dated migration exceptions expire on their date.
//!
//! Drift classes:
//! - `missing_inheritance`: clippy-enforced package without inheritance
//!   and without an explicit exception;
//! - `unowned_weakening`: item-level allows in non-test source with no
//!   covering exception;
//! - `weakening_local_lints`: a local `[lints]` table that weakens
//!   (`allow`/`warn`) a lint the workspace denies;
//! - `conflicting_policy`: the same lint declared twice for one
//!   package at different levels;
//! - `expired_exception`: a dated exception past its expiry;
//! - `unreasoned_exception`: an exception without a reason.
//!
//! Claim boundary: a read-only drift guard over the typed inventory
//! and the explicit exception set. It changes no lint level, mutates
//! nothing, and does not approve exceptions — approval stays with the
//! reviewed exception entries and the cargo-allow ledger.

use serde::{Deserialize, Serialize};

use super::workspace_lint_inventory_v1::LintPackageRowV1;

pub const WORKSPACE_LINT_DRIFT_GUARD_SCHEMA_ID: &str = "cargo-allow.workspace-lint-drift-guard.v1";
pub const WORKSPACE_LINT_DRIFT_GUARD_SCHEMA_VERSION: u32 = 1;

const CLAIM_BOUNDARY: &str = "A read-only drift guard grading the workspace lint inventory against the explicit exception set. It rejects unowned weakening, missing inheritance, conflicting policy, and expired exceptions; it changes no lint level, mutates nothing, and does not approve exceptions — approval stays with the reviewed exception entries and the cargo-allow ledger.";

/// One explicit package/fixture difference. Every departure from
/// uniform workspace inheritance must be represented as one of these,
/// with a reason; dated migration debt expires on its date.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLintExceptionV1 {
    /// The package the exception covers.
    pub package: String,
    /// The exception scope: `inheritance` (the package does not
    /// inherit), `local_lints` (the package declares local tables), or
    /// `weakening` (the package's item-level allows are owned).
    pub scope: WorkspaceLintExceptionScopeV1,
    /// The exact reason the difference is legitimate.
    pub reason: String,
    /// Optional expiry for migration debt; past this date the
    /// exception is expired drift.
    #[serde(default)]
    pub expires_on: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLintExceptionScopeV1 {
    Inheritance,
    LocalLints,
    Weakening,
}

impl WorkspaceLintExceptionScopeV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inheritance => "inheritance",
            Self::LocalLints => "local_lints",
            Self::Weakening => "weakening",
        }
    }
}

/// The drift classes the guard names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceLintDriftClassV1 {
    MissingInheritance,
    UnownedWeakening,
    WeakeningLocalLints,
    ConflictingPolicy,
    ExpiredException,
    UnreasonedException,
}

impl WorkspaceLintDriftClassV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MissingInheritance => "missing_inheritance",
            Self::UnownedWeakening => "unowned_weakening",
            Self::WeakeningLocalLints => "weakening_local_lints",
            Self::ConflictingPolicy => "conflicting_policy",
            Self::ExpiredException => "expired_exception",
            Self::UnreasonedException => "unreasoned_exception",
        }
    }
}

/// One drift finding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLintDriftFindingV1 {
    pub class: WorkspaceLintDriftClassV1,
    pub package: String,
    pub detail: String,
}

/// The graded drift report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLintDriftReportV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// True only when no drift was found: the live tree matches the
    /// cutover law plus explicit exceptions.
    pub clean: bool,
    #[serde(default)]
    pub findings: Vec<WorkspaceLintDriftFindingV1>,
    pub claim_boundary: String,
}

/// A local `[lints]` declaration row the classifier consumes: the
/// level `allow` or `warn` on a workspace-denied lint is a weakening.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLocalLintRowV1 {
    pub package: String,
    pub lint: String,
    pub level: String,
}

/// Weakening levels for a lint the workspace denies.
fn is_weakening_level(level: &str) -> bool {
    matches!(level, "allow" | "warn")
}

/// Grade the inventory against the cutover law plus the explicit
/// exception set. Pure, deterministic, ordered by (class, package,
/// detail).
#[must_use]
pub fn evaluate_workspace_lint_drift(
    packages: &[LintPackageRowV1],
    local_lint_rows: &[WorkspaceLocalLintRowV1],
    clippy_enforced: &[String],
    exceptions: &[WorkspaceLintExceptionV1],
    current_date: &str,
) -> WorkspaceLintDriftReportV1 {
    let mut findings: Vec<WorkspaceLintDriftFindingV1> = Vec::new();

    let exception_for = |package: &str,
                         scope: WorkspaceLintExceptionScopeV1|
     -> Option<&WorkspaceLintExceptionV1> {
        exceptions
            .iter()
            .find(|exception| exception.package == package && exception.scope == scope)
    };

    // Exception-internal law first: unreasoned and expired exceptions
    // are drift in their own right.
    for exception in exceptions {
        if exception.reason.trim().is_empty() {
            findings.push(WorkspaceLintDriftFindingV1 {
                class: WorkspaceLintDriftClassV1::UnreasonedException,
                package: exception.package.clone(),
                detail: format!("{} exception carries no reason", exception.scope.label()),
            });
        }
        if let Some(expires) = &exception.expires_on {
            // ISO dates compare lexicographically; an expiry earlier
            // than the current date is expired migration debt.
            if !expires.is_empty() && expires.as_str() < current_date {
                findings.push(WorkspaceLintDriftFindingV1 {
                    class: WorkspaceLintDriftClassV1::ExpiredException,
                    package: exception.package.clone(),
                    detail: format!("{} exception expired on {expires}", exception.scope.label()),
                });
            }
            if expires.trim().is_empty() {
                findings.push(WorkspaceLintDriftFindingV1 {
                    class: WorkspaceLintDriftClassV1::UnreasonedException,
                    package: exception.package.clone(),
                    detail: format!(
                        "{} exception carries an empty expiry date",
                        exception.scope.label()
                    ),
                });
            }
        }
    }

    for package in packages {
        let enforced = clippy_enforced.contains(&package.package);

        // Missing inheritance: enforced, not inheriting, and no
        // explicit inheritance exception.
        if enforced
            && !package.inherits_workspace_lints
            && exception_for(&package.package, WorkspaceLintExceptionScopeV1::Inheritance).is_none()
        {
            findings.push(WorkspaceLintDriftFindingV1 {
                class: WorkspaceLintDriftClassV1::MissingInheritance,
                package: package.package.clone(),
                detail: "clippy-enforced without inheritance and without an explicit exception"
                    .to_string(),
            });
        }

        // Unowned weakening: item-level allows in non-test source with
        // no weakening exception. Owned weakening is explicit (the
        // exception names the package); cargo-allow's ledger stays the
        // receipt authority for the underlying attributes.
        if package.local_allow_count > 0
            && enforced
            && exception_for(&package.package, WorkspaceLintExceptionScopeV1::Weakening).is_none()
        {
            findings.push(WorkspaceLintDriftFindingV1 {
                class: WorkspaceLintDriftClassV1::UnownedWeakening,
                package: package.package.clone(),
                detail: format!(
                    "{} item-level allow attribute(s) in non-test source with no weakening exception",
                    package.local_allow_count
                ),
            });
        }

        // Conflicting policy: the same lint declared twice for this
        // package at different levels.
        let mut seen: Vec<(&str, &str)> = Vec::new();
        for declared in &package.declared_lints {
            let duplicate_conflict = seen.iter().any(|(lint, level)| {
                lint == &declared.lint.as_str() && level != &declared.level.as_str()
            });
            if duplicate_conflict {
                findings.push(WorkspaceLintDriftFindingV1 {
                    class: WorkspaceLintDriftClassV1::ConflictingPolicy,
                    package: package.package.clone(),
                    detail: format!("{} is declared at conflicting levels", declared.lint),
                });
            }
            seen.push((&declared.lint, declared.level.as_str()));
        }
    }

    // Weakening local lints: a local declaration that downgrades a
    // lint must be covered by a local-lints exception for its package.
    for row in local_lint_rows {
        if is_weakening_level(&row.level)
            && exception_for(&row.package, WorkspaceLintExceptionScopeV1::LocalLints).is_none()
        {
            findings.push(WorkspaceLintDriftFindingV1 {
                class: WorkspaceLintDriftClassV1::WeakeningLocalLints,
                package: row.package.clone(),
                detail: format!(
                    "local {} = {} weakens the posture without a local-lints exception",
                    row.lint, row.level
                ),
            });
        }
    }

    findings.sort();

    WorkspaceLintDriftReportV1 {
        schema_id: WORKSPACE_LINT_DRIFT_GUARD_SCHEMA_ID.to_string(),
        schema_version: WORKSPACE_LINT_DRIFT_GUARD_SCHEMA_VERSION,
        clean: findings.is_empty(),
        findings,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of the drift report. Deterministic.
#[must_use]
pub fn render_workspace_lint_drift_human(report: &WorkspaceLintDriftReportV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "workspace-lint-drift-guard: {}",
        if report.clean { "clean" } else { "drift" }
    ));
    for finding in &report.findings {
        lines.push(format!(
            "  {:?} {}: {}",
            finding.class, finding.package, finding.detail
        ));
    }
    lines.push(format!("  claim boundary: {}", report.claim_boundary));
    lines.join("\n")
}

/// JSON view of the drift report.
pub fn render_workspace_lint_drift_json(
    report: &WorkspaceLintDriftReportV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}
