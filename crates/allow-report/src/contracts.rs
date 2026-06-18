use std::collections::BTreeMap;

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
    WORKLIST_ARTIFACT,
];

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

pub const CLAIM_BOUNDARY_TEXT: &str = "Claim boundary: scanned source-tree/source syntax only; cargo-allow did not invoke Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros, external evidence tools, or repository code. Macro expansion, macro token-tree contents, type information, MIR, build output, control flow, and data flow were not analyzed.";

#[derive(Debug, Clone, Copy)]
pub struct InventoryContext<'a> {
    pub scope: &'a str,
    pub scanner: &'a str,
    pub source: &'a str,
    pub root: Option<&'a str>,
    pub files_scanned: Option<usize>,
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
}

impl<'a> Default for InventoryContext<'a> {
    fn default() -> Self {
        Self::unknown_source_syntax()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReportContext<'a> {
    pub inventory: InventoryContext<'a>,
    pub baseline_debt_entries: Option<usize>,
    pub policy_missing_evidence_entries: Option<usize>,
    pub broken_evidence_links: Option<usize>,
    pub weak_evidence_references: Option<usize>,
    pub mode: Option<&'a str>,
    pub enforcement: Option<&'a str>,
    pub policy_config: Option<&'a str>,
    pub tool_version: Option<&'a str>,
    pub lane_posture: Option<&'a BTreeMap<String, allow_core::LaneEnforcementMode>>,
}

impl<'a> ReportContext<'a> {
    pub const fn source_syntax(
        inventory_source: &'a str,
        source_tree_root: Option<&'a str>,
        inventory_files: Option<usize>,
        baseline_debt_entries: Option<usize>,
    ) -> Self {
        Self {
            inventory: InventoryContext::source_syntax(
                inventory_source,
                source_tree_root,
                inventory_files,
            ),
            baseline_debt_entries,
            policy_missing_evidence_entries: None,
            broken_evidence_links: None,
            weak_evidence_references: None,
            mode: None,
            enforcement: None,
            policy_config: None,
            tool_version: None,
            lane_posture: None,
        }
    }
}

impl<'a> From<ReportContext<'a>> for InventoryContext<'a> {
    fn from(context: ReportContext<'a>) -> Self {
        context.inventory
    }
}
