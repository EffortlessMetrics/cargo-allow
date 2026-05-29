use allow_core::{AllowEntry, Finding};

use crate::InventoryContext;

#[derive(Debug, Clone, Copy)]
pub struct AddReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    pub selected_finding: &'a Finding,
    pub policy_output: Option<&'a str>,
    pub force: bool,
}

impl<'a> AddReport<'a> {
    pub fn new(
        inventory: InventoryContext<'a>,
        entry: &'a AllowEntry,
        selected_finding: &'a Finding,
        policy_output: Option<&'a str>,
        force: bool,
    ) -> Self {
        Self {
            inventory,
            entry,
            selected_finding,
            policy_output,
            force,
        }
    }
}
