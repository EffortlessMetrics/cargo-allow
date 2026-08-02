use allow_core::CargoAllowResult;
use allow_policy::federation::{
    FederationLoadOutcome, FederationLoadResult, ValidatedFederationConfig,
    detect_mirror_divergences, load_federation_config,
};
use allow_report::{ConfiguredLedgerSummary, FederationDiagnosticSummary};
use std::path::Path;

#[derive(Debug, Default)]
pub(crate) struct FederationDoctorFacts {
    pub path: Option<String>,
    pub found: bool,
    pub valid: Option<bool>,
    ledgers: Vec<FederationLedgerFacts>,
    diagnostics: Vec<FederationDiagnosticFacts>,
    divergence_facts: Vec<FederationDivergenceFacts>,
}

#[derive(Debug)]
struct FederationDiagnosticFacts {
    kind: String,
    message: String,
    ledger_ids: Vec<String>,
}

#[derive(Debug)]
struct FederationDivergenceFacts {
    kind: String,
    message: String,
    ledger_ids: Vec<String>,
}

#[derive(Debug)]
struct FederationLedgerFacts {
    id: String,
    path: String,
    dialect: String,
    role: String,
    mode: String,
    priority: u32,
    lanes: Vec<String>,
    mirrors: Option<String>,
}

impl FederationDoctorFacts {
    pub fn load(root: &Path) -> CargoAllowResult<Self> {
        let loaded = load_federation_config(root)?;
        Ok(Self::from_load_result(loaded))
    }

    fn from_load_result(loaded: FederationLoadResult) -> Self {
        match loaded.outcome {
            FederationLoadOutcome::Missing => Self::default(),
            FederationLoadOutcome::Parsed(validated) => {
                Self::from_validated(loaded.path, validated)
            }
        }
    }

    fn from_validated(path: String, validated: ValidatedFederationConfig) -> Self {
        Self {
            path: Some(path),
            found: true,
            valid: Some(validated.valid),
            ledgers: validated
                .config
                .ledgers
                .into_iter()
                .map(|ledger| FederationLedgerFacts {
                    id: ledger.id,
                    path: ledger.path,
                    dialect: ledger.dialect,
                    role: ledger.role.as_str().to_string(),
                    mode: ledger.mode.as_str().to_string(),
                    priority: ledger.priority,
                    lanes: ledger.lanes,
                    mirrors: ledger.mirrors,
                })
                .collect(),
            diagnostics: validated
                .diagnostics
                .into_iter()
                .map(|diagnostic| FederationDiagnosticFacts {
                    kind: diagnostic.kind.as_str().to_string(),
                    message: diagnostic.message,
                    ledger_ids: diagnostic.ledger_ids,
                })
                .collect(),
            divergence_facts: Vec::new(),
        }
    }

    pub fn enrich_runtime_divergences(&mut self, root: &Path) -> CargoAllowResult<()> {
        let loaded = load_federation_config(root)?;
        let Some(validated) = loaded.validated() else {
            return Ok(());
        };
        if validated
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.is_blocking())
        {
            return Ok(());
        }
        let divergences = detect_mirror_divergences(root, &validated.config)?;
        self.divergence_facts = divergences
            .into_iter()
            .map(|record| FederationDivergenceFacts {
                kind: record.kind.as_str().to_string(),
                message: record.message,
                ledger_ids: vec![record.mirror_ledger_id, record.canonical_ledger_id],
            })
            .collect();
        Ok(())
    }

    pub fn federation_config_path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn configured_ledger_summaries(&self) -> Vec<ConfiguredLedgerSummary<'_>> {
        self.ledgers
            .iter()
            .map(|ledger| ConfiguredLedgerSummary {
                id: ledger.id.as_str(),
                path: ledger.path.as_str(),
                dialect: ledger.dialect.as_str(),
                role: ledger.role.as_str(),
                mode: ledger.mode.as_str(),
                priority: ledger.priority,
                lanes: ledger.lanes.as_slice(),
                mirrors: ledger.mirrors.as_deref(),
            })
            .collect()
    }

    pub fn diagnostic_summaries(&self) -> Vec<FederationDiagnosticSummary<'_>> {
        self.diagnostics
            .iter()
            .map(|diagnostic| FederationDiagnosticSummary {
                kind: diagnostic.kind.as_str(),
                message: diagnostic.message.as_str(),
                ledger_ids: diagnostic.ledger_ids.as_slice(),
            })
            .collect()
    }

    pub fn divergence_summaries(&self) -> Vec<FederationDiagnosticSummary<'_>> {
        self.divergence_facts
            .iter()
            .map(|record| FederationDiagnosticSummary {
                kind: record.kind.as_str(),
                message: record.message.as_str(),
                ledger_ids: record.ledger_ids.as_slice(),
            })
            .collect()
    }
}
