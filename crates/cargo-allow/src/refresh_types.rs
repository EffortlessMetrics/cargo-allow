use allow_core::{AllowEntry, Finding, LastSeen};
use allow_report::{InventoryContext, MutationReceipt};

#[derive(Debug, Clone, Copy)]
pub(super) struct RefreshContext<'a> {
    pub inventory: InventoryContext<'a>,
}

pub(super) struct RefreshRenderInput<'a> {
    pub entry: &'a AllowEntry,
    pub finding: &'a Finding,
    pub previous_last_seen: Option<LastSeen>,
    pub drift_message: &'a str,
    pub dry_run: bool,
    pub write_requested: bool,
    pub written_path: Option<&'a str>,
    pub context: RefreshContext<'a>,
    pub mutation_receipt: MutationReceipt<'a>,
}

pub(super) struct RefreshEmitInput<'a> {
    pub entry: &'a AllowEntry,
    pub finding: &'a Finding,
    pub previous_last_seen: Option<LastSeen>,
    pub drift_message: &'a str,
    pub root: &'a std::path::Path,
    pub policy_path: &'a std::path::Path,
    pub inventory_facts: crate::InventoryFacts,
    pub written_path: Option<&'a std::path::Path>,
    pub mutation_receipt: MutationReceipt<'a>,
}
