use allow_core::{AllowEntry, Finding};

use crate::{InventoryContext, MutationReceipt};

#[derive(Debug, Clone)]
pub struct AddReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    /// The single finding this entry receipts, for the `--path`/`--line`
    /// route. `None` for a broad (`--glob`) scope baseline, which selects no
    /// individual finding: the governed summary's `selected_finding` is then
    /// null, and that null is the machine-readable route marker (#4604).
    pub selected_finding: Option<&'a Finding>,
    pub policy_output: Option<&'a str>,
    pub force: bool,
    pub mutation_receipt: MutationReceipt<'a>,
}

impl<'a> AddReport<'a> {
    pub fn new(
        inventory: InventoryContext<'a>,
        entry: &'a AllowEntry,
        selected_finding: Option<&'a Finding>,
        policy_output: Option<&'a str>,
        force: bool,
        mutation_receipt: MutationReceipt<'a>,
    ) -> Self {
        Self {
            inventory,
            entry,
            selected_finding,
            policy_output,
            force,
            mutation_receipt,
        }
    }
}
