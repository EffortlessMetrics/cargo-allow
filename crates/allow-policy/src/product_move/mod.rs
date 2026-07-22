//! Three-product move/deletion ledger (#2598).
//!
//! Parses and validates the machine-readable move ledger. Report-only in Wave 0
//! PR1: inventory and schema validation without moving implementation code.

mod config;
mod validate;

pub use config::{
    MoveDisposition, MoveEntry, MoveEntryStatus, MoveIdentityKind, ProductMoveLedger,
    ValidatedProductMoveLedger, parse_product_move_ledger, parse_product_move_ledger_at,
};
pub use validate::{
    MoveLedgerDiagnostic, MoveLedgerDiagnosticKind, MoveLedgerReport, validate_product_move_ledger,
    validate_product_move_ledger_at,
};

#[cfg(test)]
mod tests;
