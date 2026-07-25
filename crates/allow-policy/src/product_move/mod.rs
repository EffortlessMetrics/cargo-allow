//! Three-product move/deletion ledger (#2598).
//!
//! Parses and validates the machine-readable extraction denominator. This is a
//! report-only architecture control surface: it does not move implementation or
//! select runtime product behavior.

mod config;
mod validate;

pub use config::{
    MoveDiscovery, MoveEntry, PRODUCT_MOVE_LEDGER_SCHEMA_ID, PRODUCT_MOVE_LEDGER_SCHEMA_VERSION,
    ProductMoveLedger, parse_product_move_ledger, parse_product_move_ledger_at,
};
pub use validate::{
    MoveLedgerDiagnostic, MoveLedgerDiagnosticKind, MoveLedgerReport, ValidatedProductMoveLedger,
    render_product_move_map, validate_product_move_ledger, validate_product_move_ledger_at,
};

#[cfg(test)]
mod tests;
