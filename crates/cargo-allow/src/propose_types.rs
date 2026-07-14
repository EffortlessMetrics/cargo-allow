#[derive(Debug, Clone)]
pub(crate) struct ProposeContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    pub(super) kind_filter: Option<&'a str>,
    pub(super) mutation_receipt: allow_report::MutationReceipt<'a>,
}
