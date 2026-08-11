use super::config::{ExtractionShimRegistry, parse_extraction_shim_registry_at};
use allow_core::CargoAllowResult;
use std::collections::BTreeSet;
use std::path::Path;

pub const EXTRACTION_SHIM_REGISTRY_RELATIVE_PATH: &str = "policy/extraction-shims.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShimDiagnosticKind {
    DuplicateShimId,
    MissingMoveLedgerEntry,
    EmptyRegistry,
    MissingParityCase,
}

impl ShimDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateShimId => "duplicate_shim_id",
            Self::MissingMoveLedgerEntry => "missing_move_ledger_entry",
            Self::EmptyRegistry => "empty_registry",
            Self::MissingParityCase => "missing_parity_case",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimDiagnostic {
    pub kind: ShimDiagnosticKind,
    pub message: String,
    pub shim_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShimReport {
    pub shim_count: usize,
    pub planned_count: usize,
    pub active_count: usize,
}

pub fn validate_extraction_shim_registry(
    registry: ExtractionShimRegistry,
    move_ledger_entry_ids: &[String],
) -> (ExtractionShimRegistry, Vec<ShimDiagnostic>, ShimReport) {
    let mut diagnostics = Vec::new();
    if registry.shim.is_empty() {
        diagnostics.push(ShimDiagnostic {
            kind: ShimDiagnosticKind::EmptyRegistry,
            message: "extraction shim registry has no entries".to_string(),
            shim_ids: Vec::new(),
        });
    }

    let ledger_ids: BTreeSet<&str> = move_ledger_entry_ids.iter().map(String::as_str).collect();

    let mut seen = BTreeSet::new();
    let mut planned_count = 0usize;
    let mut active_count = 0usize;

    for entry in &registry.shim {
        if !seen.insert(entry.id.clone()) {
            diagnostics.push(ShimDiagnostic {
                kind: ShimDiagnosticKind::DuplicateShimId,
                message: format!("duplicate shim id `{}`", entry.id),
                shim_ids: vec![entry.id.clone()],
            });
        }

        if !ledger_ids.contains(entry.move_ledger_entry.as_str()) {
            diagnostics.push(ShimDiagnostic {
                kind: ShimDiagnosticKind::MissingMoveLedgerEntry,
                message: format!(
                    "shim `{}` references unknown move ledger entry `{}`",
                    entry.id, entry.move_ledger_entry
                ),
                shim_ids: vec![entry.id.clone()],
            });
        }

        if entry.parity_case.is_none() {
            diagnostics.push(ShimDiagnostic {
                kind: ShimDiagnosticKind::MissingParityCase,
                message: format!("shim `{}` missing parity_case reference", entry.id),
                shim_ids: vec![entry.id.clone()],
            });
        }

        match entry.status {
            super::config::ShimStatus::Planned => planned_count += 1,
            super::config::ShimStatus::Active => active_count += 1,
            super::config::ShimStatus::Removed => {}
        }
    }

    let report = ShimReport {
        shim_count: registry.shim.len(),
        planned_count,
        active_count,
    };

    (registry, diagnostics, report)
}

pub fn validate_extraction_shim_registry_at(
    _root: &Path,
    registry_path: &Path,
    move_ledger_path: &Path,
) -> CargoAllowResult<(ExtractionShimRegistry, Vec<ShimDiagnostic>, ShimReport)> {
    let registry_text = std::fs::read_to_string(registry_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "shim registry unreadable at {}: {err}",
            registry_path.display()
        ))
    })?;
    let registry = parse_extraction_shim_registry_at(Some(registry_path), &registry_text)?;

    let ledger_text = std::fs::read_to_string(move_ledger_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "move ledger unreadable at {}: {err}",
            move_ledger_path.display()
        ))
    })?;
    let ledger =
        crate::product_move::parse_product_move_ledger_at(Some(move_ledger_path), &ledger_text)?;
    let move_ids: Vec<String> = ledger.entry.iter().map(|entry| entry.id.clone()).collect();

    Ok(validate_extraction_shim_registry(registry, &move_ids))
}

pub fn extraction_shim_registry_blocks_enforced_check(root: &Path) -> CargoAllowResult<bool> {
    let registry_path = root.join(EXTRACTION_SHIM_REGISTRY_RELATIVE_PATH);
    if !registry_path.is_file() {
        return Ok(false);
    }

    let move_ledger_path = root.join(crate::product_move::PRODUCT_MOVE_LEDGER_RELATIVE_PATH);
    let (_, diagnostics, _) =
        validate_extraction_shim_registry_at(root, &registry_path, &move_ledger_path)?;
    Ok(!diagnostics.is_empty())
}
