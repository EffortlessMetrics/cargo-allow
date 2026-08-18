use super::config::{
    ExtractionParityRegistry, ExtractionStage, parse_extraction_parity_registry_at,
};
use allow_core::CargoAllowResult;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityDiagnosticKind {
    DuplicateCaseId,
    MissingMoveLedgerEntry,
    MissingShimReference,
    UnreferencedShimParityCase,
    EmptyRegistry,
    NonContractDisposition,
}

impl ParityDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateCaseId => "duplicate_case_id",
            Self::MissingMoveLedgerEntry => "missing_move_ledger_entry",
            Self::MissingShimReference => "missing_shim_reference",
            Self::UnreferencedShimParityCase => "unreferenced_shim_parity_case",
            Self::EmptyRegistry => "empty_registry",
            Self::NonContractDisposition => "non_contract_disposition",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityDiagnostic {
    pub kind: ParityDiagnosticKind,
    pub message: String,
    pub case_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParityReport {
    pub case_count: usize,
    pub contract_only_count: usize,
    pub stage_receipt_count: usize,
}

pub fn validate_extraction_parity_registry(
    registry: ExtractionParityRegistry,
    move_ledger_entry_ids: &[String],
    shim_parity_case_ids: &[String],
) -> (
    ExtractionParityRegistry,
    Vec<ParityDiagnostic>,
    ParityReport,
) {
    let mut diagnostics = Vec::new();
    if registry.case.is_empty() {
        diagnostics.push(ParityDiagnostic {
            kind: ParityDiagnosticKind::EmptyRegistry,
            message: "extraction parity registry has no cases".to_string(),
            case_ids: Vec::new(),
        });
    }

    let ledger_ids: BTreeSet<&str> = move_ledger_entry_ids.iter().map(String::as_str).collect();
    let shim_cases: BTreeSet<&str> = shim_parity_case_ids.iter().map(String::as_str).collect();

    let mut seen = BTreeSet::new();
    let mut contract_only_count = 0usize;
    let mut referenced_shim_cases = BTreeSet::new();

    for entry in &registry.case {
        if !seen.insert(entry.id.clone()) {
            diagnostics.push(ParityDiagnostic {
                kind: ParityDiagnosticKind::DuplicateCaseId,
                message: format!("duplicate parity case id `{}`", entry.id),
                case_ids: vec![entry.id.clone()],
            });
        }

        if !ledger_ids.contains(entry.move_ledger_entry.as_str()) {
            diagnostics.push(ParityDiagnostic {
                kind: ParityDiagnosticKind::MissingMoveLedgerEntry,
                message: format!(
                    "parity case `{}` references unknown move ledger entry `{}`",
                    entry.id, entry.move_ledger_entry
                ),
                case_ids: vec![entry.id.clone()],
            });
        }

        if let Some(shim_id) = &entry.shim_id
            && !shim_cases.contains(entry.id.as_str())
        {
            diagnostics.push(ParityDiagnostic {
                kind: ParityDiagnosticKind::MissingShimReference,
                message: format!(
                    "parity case `{}` with shim_id `{shim_id}` is not referenced by any shim parity_case",
                    entry.id
                ),
                case_ids: vec![entry.id.clone()],
            });
        }

        referenced_shim_cases.insert(entry.id.as_str());

        // #3469 slice B: RepoSnapshot and RepoEdit cases are promoted to
        // `proven` after their runtime parity, reachability, ownership, and
        // independent build/package receipts flowed end-to-end in CI
        // (#3552, #3554). #3309 installment 3 promotes the IntentEngine
        // stage the same way: its parity cases are backed by the landed
        // dual-run fixtures and tests (#3643-#3652). Every other stage
        // stays PR1 fail-closed (contract_only only).
        let promotion_stage = matches!(
            entry.stage,
            ExtractionStage::RepoSnapshot
                | ExtractionStage::RepoEdit
                | ExtractionStage::IntentEngine
        );
        match entry.disposition.as_str() {
            "contract_only" => contract_only_count += 1,
            "proven" if promotion_stage => {}
            other => {
                diagnostics.push(ParityDiagnostic {
                    kind: ParityDiagnosticKind::NonContractDisposition,
                    message: format!(
                        "parity case `{}` has unsupported disposition `{other}`; only contract_only is allowed, except proven for the promoted RepoSnapshot/RepoEdit/IntentEngine stages",
                        entry.id
                    ),
                    case_ids: vec![entry.id.clone()],
                });
            }
        }
    }

    for shim_case in &shim_cases {
        if !referenced_shim_cases.contains(shim_case) {
            diagnostics.push(ParityDiagnostic {
                kind: ParityDiagnosticKind::UnreferencedShimParityCase,
                message: format!("shim parity case `{shim_case}` has no parity registry entry"),
                case_ids: vec![shim_case.to_string()],
            });
        }
    }

    let report = ParityReport {
        case_count: registry.case.len(),
        contract_only_count,
        stage_receipt_count: registry.stage_receipt.len(),
    };

    (registry, diagnostics, report)
}

pub fn validate_extraction_parity_registry_at(
    root: &Path,
    registry_path: &Path,
    move_ledger_path: &Path,
    shim_registry_path: &Path,
) -> CargoAllowResult<(
    ExtractionParityRegistry,
    Vec<ParityDiagnostic>,
    ParityReport,
)> {
    let registry_text = std::fs::read_to_string(registry_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "parity registry unreadable at {}: {err}",
            registry_path.display()
        ))
    })?;
    let registry = parse_extraction_parity_registry_at(Some(registry_path), &registry_text)?;

    let ledger_text = std::fs::read_to_string(move_ledger_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "move ledger unreadable at {}: {err}",
            move_ledger_path.display()
        ))
    })?;
    let ledger =
        crate::product_move::parse_product_move_ledger_at(Some(move_ledger_path), &ledger_text)?;
    let move_ids: Vec<String> = ledger.entry.iter().map(|entry| entry.id.clone()).collect();

    let shim_text = std::fs::read_to_string(shim_registry_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "shim registry unreadable at {}: {err}",
            shim_registry_path.display()
        ))
    })?;
    let shims = crate::extraction_shims::parse_extraction_shim_registry_at(
        Some(shim_registry_path),
        &shim_text,
    )?;
    let shim_case_ids: Vec<String> = shims
        .shim
        .iter()
        .filter_map(|entry| entry.parity_case.clone())
        .collect();

    let _ = root;
    Ok(validate_extraction_parity_registry(
        registry,
        &move_ids,
        &shim_case_ids,
    ))
}
