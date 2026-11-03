use allow_core::SimpleDate;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExplainContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    /// The command-boundary as-of date (#4237): evaluation and every
    /// lifecycle projection in this command read this one snapshot instead
    /// of re-sampling the clock. Callers choose the date source explicitly.
    pub(super) as_of: SimpleDate,
}
