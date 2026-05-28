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
pub const MIGRATE_SCHEMA_VERSION: u32 = 1;
pub const MIGRATE_SCHEMA_ID: &str = "cargo-allow.migrate.v1";

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
    "repository_code_not_executed",
];

pub const CLAIM_BOUNDARY_TEXT: &str = "Claim boundary: scanned source-tree/source syntax only; cargo-allow did not invoke Cargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros, or repository code. Macro expansion, macro token-tree contents, type information, MIR, build output, control flow, and data flow were not analyzed.";

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
        Self::new("source_tree", "source_syntax", source, root, files_scanned)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReportContext<'a> {
    pub inventory_source: &'a str,
    pub source_tree_root: Option<&'a str>,
    pub inventory_files: Option<usize>,
    pub baseline_debt_entries: Option<usize>,
}

impl<'a> ReportContext<'a> {
    pub const fn source_syntax(
        inventory_source: &'a str,
        source_tree_root: Option<&'a str>,
        inventory_files: Option<usize>,
        baseline_debt_entries: Option<usize>,
    ) -> Self {
        Self {
            inventory_source,
            source_tree_root,
            inventory_files,
            baseline_debt_entries,
        }
    }
}

impl Default for ReportContext<'static> {
    fn default() -> Self {
        Self::source_syntax("unknown", None, None, None)
    }
}

impl<'a> From<ReportContext<'a>> for InventoryContext<'a> {
    fn from(context: ReportContext<'a>) -> Self {
        Self::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        )
    }
}
