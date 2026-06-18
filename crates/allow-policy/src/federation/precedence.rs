use super::config::LedgerEntry;

/// Returns ledger entries in deterministic precedence order: lower `priority`
/// first, then declaration order within the config file.
pub fn ordered_ledgers_by_precedence(ledgers: &[LedgerEntry]) -> Vec<&LedgerEntry> {
    let mut indexed = ledgers.iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by(|(left_index, left), (right_index, right)| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left_index.cmp(right_index))
    });
    indexed.into_iter().map(|(_, ledger)| ledger).collect()
}
