use allow_core::{AllowEntry, Finding};

use crate::{InventoryContext, MutationReceipt};

#[derive(Debug, Clone)]
pub struct AddReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    pub selected_finding: &'a Finding,
    pub policy_output: Option<&'a str>,
    pub force: bool,
    pub mutation_receipt: MutationReceipt<'a>,
}

impl<'a> AddReport<'a> {
    pub fn new(
        inventory: InventoryContext<'a>,
        entry: &'a AllowEntry,
        selected_finding: &'a Finding,
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
