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
    pub unsafe_baseline_debt_entries_proposed: usize,
}
