//! Ledger provenance attached to federation-aware findings and work items.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerProvenance {
    pub ledger_id: String,
    pub ledger_path: String,
    pub lane: String,
    pub mode: String,
    pub role: String,
}
