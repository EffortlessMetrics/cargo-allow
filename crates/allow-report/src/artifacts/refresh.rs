use allow_core::{AllowEntry, Finding, LastSeen};

use crate::InventoryContext;

#[derive(Debug, Clone, Copy)]
pub struct RefreshModeContext<'a> {
    pub explicit_dry_run: bool,
    pub write_requested: bool,
    pub written_path: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct RefreshReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    pub finding: &'a Finding,
    pub previous_last_seen: Option<LastSeen>,
    pub drift_message: &'a str,
    pub mode: RefreshModeContext<'a>,
}

impl<'a> RefreshReport<'a> {
    pub fn new(
        inventory: InventoryContext<'a>,
        entry: &'a AllowEntry,
        finding: &'a Finding,
        previous_last_seen: Option<LastSeen>,
        drift_message: &'a str,
        mode: RefreshModeContext<'a>,
    ) -> Self {
        Self {
            inventory,
            entry,
            finding,
            previous_last_seen,
            drift_message,
            mode,
        }
    }
}
