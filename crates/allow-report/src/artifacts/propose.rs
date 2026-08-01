use crate::{InventoryContext, MutationReceipt};

#[derive(Debug, Clone)]
pub struct ProposeReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub kind: Option<&'a str>,
    pub expires: &'a str,
    pub policy_output: Option<&'a str>,
    pub force: bool,
    pub findings_scanned: usize,
    pub baseline_debt_entries_proposed: usize,
    pub unsafe_baseline_debt_entries_proposed: usize,
    pub truncated_new_findings: usize,
    /// New findings that were deliberately not proposed because the policy's
    /// own requirements forbid receipting them (#3023).
    pub unreceiptable_new_findings: usize,
    /// Why those findings could not be receipted. `None` when the count is 0.
    pub unreceiptable_reason: Option<&'static str>,
    pub mutation_receipt: MutationReceipt<'a>,
}
