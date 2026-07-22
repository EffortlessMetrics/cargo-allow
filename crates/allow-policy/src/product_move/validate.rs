use super::config::{
    MoveDisposition, MoveEntryStatus, ProductMoveLedger, ValidatedProductMoveLedger,
    parse_product_move_ledger_at,
};
use allow_core::CargoAllowResult;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveLedgerDiagnosticKind {
    DuplicateId,
    MissingCurrentPath,
    EmptyEntrySet,
}

impl MoveLedgerDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateId => "duplicate_id",
            Self::MissingCurrentPath => "missing_current_path",
            Self::EmptyEntrySet => "empty_entry_set",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveLedgerDiagnostic {
    pub kind: MoveLedgerDiagnosticKind,
    pub message: String,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MoveLedgerReport {
    pub entry_count: usize,
    pub current_count: usize,
    pub transitional_count: usize,
    pub delete_after_parity_count: usize,
    pub decision_required_count: usize,
    pub disposition_counts: BTreeMap<String, usize>,
}

pub fn validate_product_move_ledger(ledger: ProductMoveLedger) -> ValidatedProductMoveLedger {
    validate_product_move_ledger_with_root(None, ledger)
}

pub fn validate_product_move_ledger_at(
    root: &Path,
    ledger_path: &Path,
) -> CargoAllowResult<(
    ValidatedProductMoveLedger,
    Vec<MoveLedgerDiagnostic>,
    MoveLedgerReport,
)> {
    let text = std::fs::read_to_string(ledger_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "product move ledger unreadable at {}: {err}",
            ledger_path.display()
        ))
    })?;
    let ledger = parse_product_move_ledger_at(Some(ledger_path), &text)?;
    let validated = validate_product_move_ledger_with_root(Some(root), ledger);
    let diagnostics = collect_diagnostics(validated.ledger.clone(), root);
    let report = summarize_report(&validated.ledger);
    Ok((validated, diagnostics, report))
}

fn validate_product_move_ledger_with_root(
    root: Option<&Path>,
    ledger: ProductMoveLedger,
) -> ValidatedProductMoveLedger {
    let diagnostics = collect_diagnostics(ledger.clone(), root.unwrap_or(Path::new(".")));
    let valid = diagnostics.is_empty();
    ValidatedProductMoveLedger { ledger, valid }
}

fn collect_diagnostics(ledger: ProductMoveLedger, root: &Path) -> Vec<MoveLedgerDiagnostic> {
    let mut diagnostics = Vec::new();
    if ledger.entry.is_empty() {
        diagnostics.push(MoveLedgerDiagnostic {
            kind: MoveLedgerDiagnosticKind::EmptyEntrySet,
            message: "product move ledger has no entries".to_string(),
            entry_ids: Vec::new(),
        });
    }

    let mut seen = BTreeSet::new();
    for entry in &ledger.entry {
        if !seen.insert(entry.id.clone()) {
            diagnostics.push(MoveLedgerDiagnostic {
                kind: MoveLedgerDiagnosticKind::DuplicateId,
                message: format!("duplicate move ledger entry id `{}`", entry.id),
                entry_ids: vec![entry.id.clone()],
            });
        }

        if entry.identity_kind.expects_repo_path() {
            let path = root.join(&entry.current_identity);
            let exists = if entry.identity_kind.as_str() == "rust_module_tree" {
                path.is_dir()
            } else {
                path.is_file()
            };
            if !exists {
                diagnostics.push(MoveLedgerDiagnostic {
                    kind: MoveLedgerDiagnosticKind::MissingCurrentPath,
                    message: format!(
                        "current identity path missing for `{}`: {}",
                        entry.id, entry.current_identity
                    ),
                    entry_ids: vec![entry.id.clone()],
                });
            }
        }
    }

    diagnostics
}

fn summarize_report(ledger: &ProductMoveLedger) -> MoveLedgerReport {
    let mut report = MoveLedgerReport {
        entry_count: ledger.entry.len(),
        ..MoveLedgerReport::default()
    };

    for entry in &ledger.entry {
        match entry.status {
            MoveEntryStatus::Current => report.current_count += 1,
            MoveEntryStatus::Transitional => report.transitional_count += 1,
            MoveEntryStatus::DecisionRequired => report.decision_required_count += 1,
            MoveEntryStatus::Moved | MoveEntryStatus::Deleted => {}
        }
        if entry.disposition == MoveDisposition::DeleteAfterParity {
            report.delete_after_parity_count += 1;
        }
        *report
            .disposition_counts
            .entry(entry.disposition.as_str().to_string())
            .or_insert(0) += 1;
    }

    report
}
