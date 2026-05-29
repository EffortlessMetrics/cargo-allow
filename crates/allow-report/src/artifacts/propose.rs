use crate::InventoryContext;

#[derive(Debug, Clone, Copy)]
pub struct ProposeReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub kind: Option<&'a str>,
    pub expires: &'a str,
    pub policy_output: Option<&'a str>,
    pub force: bool,
    pub findings_scanned: usize,
    pub baseline_debt_entries_proposed: usize,
}

impl<'a> ProposeReport<'a> {
    pub fn new(
        inventory: InventoryContext<'a>,
        kind: Option<&'a str>,
        expires: &'a str,
        policy_output: Option<&'a str>,
        force: bool,
        findings_scanned: usize,
        baseline_debt_entries_proposed: usize,
    ) -> Self {
        Self {
            inventory,
            kind,
            expires,
            policy_output,
            force,
            findings_scanned,
            baseline_debt_entries_proposed,
        }
    }
}
