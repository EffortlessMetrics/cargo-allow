use std::collections::BTreeMap;

use crate::artifacts::federation::FederationReportContext;

pub const REPORT_SCHEMA_VERSION: u32 = 1;
pub const REPORT_SCHEMA_ID: &str = "cargo-allow.report.v1";
pub const RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const RECEIPT_SCHEMA_ID: &str = "cargo-allow.receipt.v1";
pub const WORKLIST_SCHEMA_VERSION: u32 = 1;
pub const WORKLIST_SCHEMA_ID: &str = "cargo-allow.worklist.v1";
pub const LIST_SCHEMA_VERSION: u32 = 1;
pub const LIST_SCHEMA_ID: &str = "cargo-allow.list.v1";
pub const EXPLAIN_SCHEMA_VERSION: u32 = 1;
pub const EXPLAIN_SCHEMA_ID: &str = "cargo-allow.explain.v1";
pub const WHY_SCHEMA_VERSION: u32 = 1;
pub const WHY_SCHEMA_ID: &str = "cargo-allow.why.v1";
pub const ADD_FINDING_PLAN_SCHEMA_VERSION: u32 = 1;
pub const ADD_FINDING_PLAN_SCHEMA_ID: &str = "cargo-allow.add-finding-plan.v1";
pub const ADD_PLAN_APPLICATION_SCHEMA_VERSION: u32 = 1;
pub const ADD_PLAN_APPLICATION_SCHEMA_ID: &str = "cargo-allow.add-plan-application.v1";
pub const PRUNE_SCHEMA_VERSION: u32 = 1;
pub const PRUNE_SCHEMA_ID: &str = "cargo-allow.prune.v1";
pub const DOCTOR_SCHEMA_VERSION: u32 = 1;
pub const DOCTOR_SCHEMA_ID: &str = "cargo-allow.doctor.v1";
pub const PROPOSE_SCHEMA_VERSION: u32 = 1;
pub const PROPOSE_SCHEMA_ID: &str = "cargo-allow.propose.v1";
pub const ADD_SCHEMA_VERSION: u32 = 1;
pub const ADD_SCHEMA_ID: &str = "cargo-allow.add.v1";
pub const REFRESH_SCHEMA_VERSION: u32 = 1;
pub const REFRESH_SCHEMA_ID: &str = "cargo-allow.refresh.v1";
pub const MIGRATE_SCHEMA_VERSION: u32 = 1;
pub const MIGRATE_SCHEMA_ID: &str = "cargo-allow.migrate.v1";
pub const SPEC_SYSTEM_SCHEMA_VERSION: u32 = 1;
pub const SPEC_SYSTEM_SCHEMA_ID: &str = "cargo-allow.spec-system.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactContract {
    pub name: &'static str,
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub inventory_scanner: &'static str,
    pub fixed_command: Option<&'static str>,
}

pub const ARTIFACT_STATUS_PASSED: &str = "passed";
pub const ARTIFACT_STATUS_FAILED: &str = "failed";
pub const ARTIFACT_STATUS_ERROR: &str = "error";
pub const ARTIFACT_STATUSES: &[&str] = &[ARTIFACT_STATUS_PASSED, ARTIFACT_STATUS_FAILED];
pub const RECEIPT_STATUSES: &[&str] = &[
    ARTIFACT_STATUS_PASSED,
    ARTIFACT_STATUS_FAILED,
    ARTIFACT_STATUS_ERROR,
];
pub const RECEIPT_ENFORCEMENT_ADVISORY: &str = "advisory";
pub const RECEIPT_ENFORCEMENT_ENFORCING: &str = "enforcing";

pub const REPORT_COMMAND_AUDIT: &str = "audit";
pub const REPORT_COMMAND_CHECK: &str = "check";
pub const REPORT_COMMAND_DIFF: &str = "diff";
pub const REPORT_COMMANDS: &[&str] = &[
    REPORT_COMMAND_AUDIT,
    REPORT_COMMAND_CHECK,
    REPORT_COMMAND_DIFF,
];
pub const RECEIPT_COMMAND_CHECK: &str = "check";

pub const INVENTORY_SCOPE_SOURCE_TREE: &str = "source_tree";
pub const INVENTORY_SCANNER_SOURCE_SYNTAX: &str = "source_syntax";
pub const INVENTORY_SCANNER_POLICY_MIGRATION: &str = "policy_migration";
pub const INVENTORY_SCANNER_SOURCE_TREE_GRAPH: &str = "source_tree_graph";
pub const INVENTORY_SOURCE_UNKNOWN: &str = "unknown";

pub(crate) const ADD_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "add",
    schema_id: ADD_SCHEMA_ID,
    schema_version: ADD_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("add"),
};

pub(crate) const DOCTOR_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "doctor",
    schema_id: DOCTOR_SCHEMA_ID,
    schema_version: DOCTOR_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("doctor"),
};

pub(crate) const EXPLAIN_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "explain",
    schema_id: EXPLAIN_SCHEMA_ID,
    schema_version: EXPLAIN_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("explain"),
};

pub(crate) const WHY_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "why",
    schema_id: WHY_SCHEMA_ID,
    schema_version: WHY_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("why"),
};

pub(crate) const ADD_FINDING_PLAN_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "add-finding-plan",
    schema_id: ADD_FINDING_PLAN_SCHEMA_ID,
    schema_version: ADD_FINDING_PLAN_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("why"),
};

pub(crate) const ADD_PLAN_APPLICATION_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "add-plan-application",
    schema_id: ADD_PLAN_APPLICATION_SCHEMA_ID,
    schema_version: ADD_PLAN_APPLICATION_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("add"),
};

pub(crate) const LIST_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "list",
    schema_id: LIST_SCHEMA_ID,
    schema_version: LIST_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("list"),
};

pub(crate) const REFRESH_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "refresh",
    schema_id: REFRESH_SCHEMA_ID,
    schema_version: REFRESH_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("refresh"),
};

pub(crate) const MIGRATE_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "migrate",
    schema_id: MIGRATE_SCHEMA_ID,
    schema_version: MIGRATE_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_POLICY_MIGRATION,
    fixed_command: Some("migrate"),
};

pub(crate) const PROPOSE_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "propose",
    schema_id: PROPOSE_SCHEMA_ID,
    schema_version: PROPOSE_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("propose"),
};

pub(crate) const PRUNE_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "prune",
    schema_id: PRUNE_SCHEMA_ID,
    schema_version: PRUNE_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("prune"),
};

pub(crate) const RECEIPT_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "receipt",
    schema_id: RECEIPT_SCHEMA_ID,
    schema_version: RECEIPT_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some(RECEIPT_COMMAND_CHECK),
};

pub(crate) const REPORT_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "report",
    schema_id: REPORT_SCHEMA_ID,
    schema_version: REPORT_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: None,
};

pub(crate) const SPEC_SYSTEM_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "spec-system",
    schema_id: SPEC_SYSTEM_SCHEMA_ID,
    schema_version: SPEC_SYSTEM_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_TREE_GRAPH,
    fixed_command: None,
};

pub(crate) const WORKLIST_ARTIFACT: ArtifactContract = ArtifactContract {
    name: "worklist",
    schema_id: WORKLIST_SCHEMA_ID,
    schema_version: WORKLIST_SCHEMA_VERSION,
    inventory_scanner: INVENTORY_SCANNER_SOURCE_SYNTAX,
    fixed_command: Some("worklist"),
};

pub const ARTIFACT_CONTRACTS: &[ArtifactContract] = &[
    ADD_ARTIFACT,
    ADD_FINDING_PLAN_ARTIFACT,
    ADD_PLAN_APPLICATION_ARTIFACT,
    DOCTOR_ARTIFACT,
    EXPLAIN_ARTIFACT,
    LIST_ARTIFACT,
    MIGRATE_ARTIFACT,
    PROPOSE_ARTIFACT,
    PRUNE_ARTIFACT,
    REFRESH_ARTIFACT,
    RECEIPT_ARTIFACT,
    REPORT_ARTIFACT,
    SPEC_SYSTEM_ARTIFACT,
    WHY_ARTIFACT,
    WORKLIST_ARTIFACT,
];

/// Look up an artifact contract by its full schema_id.
///
/// The schema_id includes the major version (e.g. `cargo-allow.receipt.v1`),
/// so a v2 reader will NOT silently accept a v1 artifact — the schema_id
/// simply won't match (#1856). Unknown major versions are rejected by
/// the exact-match comparison, preventing silent acceptance of future
/// schema versions by current consumers.
pub fn artifact_contract_for_schema_id(schema_id: &str) -> Option<ArtifactContract> {
    ARTIFACT_CONTRACTS
        .iter()
        .copied()
        .find(|contract| contract.schema_id == schema_id)
}

pub const CLAIM_BOUNDARY: &[&str] = &[
    "source_tree_inventory",
    "source_syntax_only",
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "external_evidence_tools_not_invoked",
    "repository_code_not_executed",
    "source_text_in_identity_fields",
];

pub const SCANNER_LIMITATIONS: &[&str] = &[
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "external_evidence_tools_not_invoked",
    "repository_code_not_executed",
];

pub const ADD_FINDING_PLAN_CLAIM_BOUNDARY: &[&str] = &[
    "source_tree_inventory",
    "source_syntax_only",
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "external_evidence_tools_not_invoked",
    "repository_code_not_executed",
    "source_text_in_identity_fields",
    "policy_not_mutated",
    "proof_commands_not_executed",
    "new_at_plan_creation_only",
    "targeted_recheck_not_executed",
    "full_repository_check_not_executed",
];

pub const ADD_PLAN_APPLICATION_CLAIM_BOUNDARY: &[&str] = &[
    "source_tree_inventory",
    "source_syntax_only",
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "external_evidence_tools_not_invoked",
    "repository_code_not_executed",
    "source_text_in_identity_fields",
    "proof_commands_not_executed",
    "targeted_recheck_not_executed",
    "full_repository_check_not_executed",
];

pub const SPEC_SYSTEM_CLAIM_BOUNDARY: &[&str] = &[
    "source_tree_inventory",
    "source_tree_graph_validation",
    "proof_commands_not_executed",
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "external_evidence_tools_not_invoked",
    "repository_code_not_executed",
    "network_not_used",
    "github_api_not_used",
];

pub const SPEC_SYSTEM_SCANNER_LIMITATIONS: &[&str] = &[
    "proof_commands_not_executed",
    "cargo_metadata_not_invoked",
    "cargo_commands_not_invoked",
    "rustc_not_invoked",
    "clippy_not_invoked",
    "build_scripts_not_executed",
    "proc_macros_not_executed",
    "macro_expansion_not_analyzed",
    "macro_token_tree_contents_not_analyzed",
    "type_information_not_analyzed",
    "mir_not_analyzed",
    "build_output_not_analyzed",
    "control_flow_not_analyzed",
    "data_flow_not_analyzed",
    "external_evidence_tools_not_invoked",
    "repository_code_not_executed",
    "network_not_used",
    "github_api_not_used",
];

pub fn claim_boundary_for_schema_id(schema_id: &str) -> &'static [&'static str] {
    if schema_id == SPEC_SYSTEM_SCHEMA_ID {
        SPEC_SYSTEM_CLAIM_BOUNDARY
    } else if schema_id == ADD_FINDING_PLAN_SCHEMA_ID {
        ADD_FINDING_PLAN_CLAIM_BOUNDARY
    } else if schema_id == ADD_PLAN_APPLICATION_SCHEMA_ID {
        ADD_PLAN_APPLICATION_CLAIM_BOUNDARY
    } else {
        CLAIM_BOUNDARY
    }
}

pub fn scanner_limitations_for_schema_id(schema_id: &str) -> &'static [&'static str] {
    if schema_id == SPEC_SYSTEM_SCHEMA_ID {
        SPEC_SYSTEM_SCANNER_LIMITATIONS
    } else {
        SCANNER_LIMITATIONS
    }
}

pub const CLAIM_BOUNDARY_TEXT: &str = "Claim boundary: scanned source-tree/source syntax only; cargo-allow did not invoke Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros, external evidence tools, or repository code. Macro expansion, macro token-tree contents, type information, MIR, build output, control flow, and data flow were not analyzed. Identity fields (symbol, callee, container, module, macro_name, lint) carry source-derived text and are emitted in CI artifacts; set CARGO_ALLOW_REDACT_IDENTITY=1 to redact them (structural hashes are preserved for matching).";

/// The passing result label, shared by every renderer.
///
/// A pass states both the outcome and the mode that produced it, so an
/// enforcing-mode pass is never reported as advisory. Human, markdown, and
/// HTML all read from here: three copies of this string previously disagreed,
/// with only HTML carrying the enforcement mode (#2832).
///
/// `enforcement` is `"enforcing"` or `"advisory"`. It falls back to
/// `"advisory"` for callers that never set a mode, which is the conservative
/// direction: claiming less enforcement than ran, never more.
pub fn passed_result_label(enforcement: Option<&str>) -> String {
    format!("passed ({})", enforcement.unwrap_or("advisory"))
}

/// Check if the `--quiet` flag was set (via `CARGO_ALLOW_QUIET=1` env var).
/// When true, report renderers suppress non-essential output: claim boundary
/// text, matched inventory listings, and advisory outcomes. Only result +
/// counts are shown (#2785).
pub fn is_quiet() -> bool {
    std::env::var_os("CARGO_ALLOW_QUIET").is_some_and(|v| v == "1")
}

#[derive(Debug, Clone, Copy)]
pub struct InventoryContext<'a> {
    pub scope: &'a str,
    pub scanner: &'a str,
    pub source: &'a str,
    pub root: Option<&'a str>,
    pub files_scanned: Option<usize>,
    pub empty_git_tracked: bool,
    pub completeness: Option<&'a str>,
}

impl<'a> InventoryContext<'a> {
    pub const fn new(
        scope: &'a str,
        scanner: &'a str,
        source: &'a str,
        root: Option<&'a str>,
        files_scanned: Option<usize>,
    ) -> Self {
        Self {
            scope,
            scanner,
            source,
            root,
            files_scanned,
            empty_git_tracked: false,
            completeness: None,
        }
    }

    pub const fn source_syntax(
        source: &'a str,
        root: Option<&'a str>,
        files_scanned: Option<usize>,
    ) -> Self {
        Self::new(
            INVENTORY_SCOPE_SOURCE_TREE,
            INVENTORY_SCANNER_SOURCE_SYNTAX,
            source,
            root,
            files_scanned,
        )
    }

    pub const fn unknown_source_syntax() -> InventoryContext<'static> {
        InventoryContext::source_syntax(INVENTORY_SOURCE_UNKNOWN, None, None)
    }

    pub const fn policy_migration(
        source: &'a str,
        root: Option<&'a str>,
        files_scanned: Option<usize>,
    ) -> Self {
        Self::new(
            INVENTORY_SCOPE_SOURCE_TREE,
            INVENTORY_SCANNER_POLICY_MIGRATION,
            source,
            root,
            files_scanned,
        )
    }

    pub fn with_empty_git_tracked(mut self, empty_git_tracked: bool) -> Self {
        self.empty_git_tracked = empty_git_tracked;
        self
    }

    pub fn with_completeness(mut self, completeness: &'a str) -> Self {
        self.completeness = Some(completeness);
        self
    }

    pub fn completeness_suffix(self) -> String {
        self.completeness
            .map(|completeness| format!("; completeness: {completeness}"))
            .unwrap_or_default()
    }

    /// Render the "files scanned" + "completeness" suffix used by all human
    /// report renderers. Centralizes the format so it stays consistent across
    /// add, list, migrate, propose, prune, refresh, and worklist outputs.
    pub fn files_scanned_suffix(self) -> String {
        let mut suffix = self
            .files_scanned
            .map(|files| format!("; files scanned: {files}"))
            .unwrap_or_default();
        suffix.push_str(&self.completeness_suffix());
        suffix
    }
}

impl<'a> Default for InventoryContext<'a> {
    fn default() -> Self {
        Self::unknown_source_syntax()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReportContext<'a> {
    /// Terminal styling for human output. Defaults to [`Style::PLAIN`].
    ///
    /// Only the human renderer reads this. JSON, SARIF, markdown, and HTML
    /// never reference it, so those formats cannot emit ANSI regardless of
    /// what the CLI decides (#2572).
    pub style: crate::style::Style,
    pub inventory: InventoryContext<'a>,
    pub baseline_debt_entries: Option<usize>,
    pub policy_missing_evidence_entries: Option<usize>,
    pub broken_evidence_links: Option<usize>,
    pub weak_evidence_references: Option<usize>,
    pub occurrence_headroom_entries: Option<usize>,
    pub mode: Option<&'a str>,
    pub enforcement: Option<&'a str>,
    pub policy_config: Option<&'a str>,
    pub tool_version: Option<&'a str>,
    pub lane_posture: Option<&'a BTreeMap<String, allow_core::LaneEnforcementMode>>,
    pub federation: Option<FederationReportContext<'a>>,
    /// Advisory count of canonical-versus-mirror divergences during active
    /// drain windows (mirror_divergence / mirror_stale). Feeds the review-item
    /// tally; it deliberately excludes blocking divergences.
    pub mirror_divergence_entries: Option<usize>,
    /// Count of blocking federation divergences (drain_expired) that fail the
    /// run. Kept distinct from `mirror_divergence_entries` so a blocking
    /// divergence surfaces in the receipt instead of being hidden behind a
    /// zero advisory count while CI fails (#1945).
    pub blocking_divergence_entries: Option<usize>,
    /// Git commit SHA when available, for receipt provenance binding (#1850).
    pub git_sha: Option<&'a str>,
    /// SHA-256 hex digest of the policy file content at scan time (#1850).
    pub policy_digest: Option<&'a str>,
    /// RFC 3339 timestamp of when the run started (#1854).
    pub started_at: Option<&'a str>,
    /// Unique run identifier (process-stable) for correlating receipt to CI run (#1854).
    pub run_id: Option<&'a str>,
    /// Count of Rust source files skipped during scan (oversized, binary,
    /// permission-denied). When non-zero and mode is no-new, the check fails
    /// closed (#2667).
    pub rust_files_skipped: usize,
}

impl<'a> ReportContext<'a> {
    pub const fn source_syntax(
        inventory_source: &'a str,
        source_tree_root: Option<&'a str>,
        inventory_files: Option<usize>,
        baseline_debt_entries: Option<usize>,
    ) -> Self {
        Self {
            // Plain by default: a context built here is not yet known to be
            // headed for an interactive terminal.
            style: crate::style::Style::PLAIN,
            inventory: InventoryContext::source_syntax(
                inventory_source,
                source_tree_root,
                inventory_files,
            ),
            baseline_debt_entries,
            policy_missing_evidence_entries: None,
            broken_evidence_links: None,
            weak_evidence_references: None,
            occurrence_headroom_entries: None,
            mode: None,
            enforcement: None,
            policy_config: None,
            tool_version: None,
            lane_posture: None,
            federation: None,
            mirror_divergence_entries: None,
            blocking_divergence_entries: None,
            git_sha: None,
            policy_digest: None,
            started_at: None,
            run_id: None,
            rust_files_skipped: 0,
        }
    }

    pub fn with_empty_git_tracked(mut self, empty_git_tracked: bool) -> Self {
        self.inventory = self.inventory.with_empty_git_tracked(empty_git_tracked);
        self
    }

    pub fn with_inventory_completeness(mut self, completeness: &'a str) -> Self {
        self.inventory = self.inventory.with_completeness(completeness);
        self
    }
}

impl<'a> From<ReportContext<'a>> for InventoryContext<'a> {
    fn from(context: ReportContext<'a>) -> Self {
        context.inventory
    }
}

#[cfg(test)]
mod result_label_tests {
    use super::*;

    /// An enforcing pass must say so. This is the whole point of the shared
    /// helper: the label previously lived in three renderers, and only HTML
    /// carried the mode — and even that never reached a report, because
    /// `enforcement` was populated on the receipt context alone.
    #[test]
    fn an_enforcing_pass_is_not_reported_as_advisory() {
        assert_eq!(passed_result_label(Some("enforcing")), "passed (enforcing)");
        assert_eq!(passed_result_label(Some("advisory")), "passed (advisory)");
    }

    /// A caller that never set a mode must under-claim, not over-claim.
    #[test]
    fn an_unknown_mode_falls_back_to_advisory() {
        assert_eq!(passed_result_label(None), "passed (advisory)");
    }
}
