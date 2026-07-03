use super::config::LedgerEntry;

/// # Federation precedence rule
///
/// Ledgers are ordered by `priority` (ascending — **lower priority wins**),
/// with **declaration order** in the config file as the deterministic tiebreak
/// for equal priorities.
///
/// `priority` is **required** on every `[[ledgers]]` entry: a missing value is
/// a parse error (#2044). Requiring it explicitly means reordering the
/// `[[ledgers]]` array for readability cannot silently flip which ledger wins
/// precedence — only an explicit `priority` change can. The declaration-order
/// tiebreak is stable and documented but is intentionally secondary to the
/// explicit field (#2010).
pub fn ordered_ledgers_by_precedence(ledgers: &[LedgerEntry]) -> Vec<&LedgerEntry> {
    let mut indexed = ledgers.iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by(|(left_index, left), (right_index, right)| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left_index.cmp(right_index))
    });
    indexed.into_iter().map(|(_, ledger)| ledger).collect()
}
