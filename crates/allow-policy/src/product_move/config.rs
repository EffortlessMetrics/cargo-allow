use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDisposition {
    MoveToSharedProtocol,
    MoveToSharedSnapshot,
    MoveToIntentModel,
    MoveToIntentProtocol,
    MoveToIntentEngine,
    MoveToIntentEdit,
    MoveToCargoIntentApp,
    MoveToProofProtocol,
    MoveToProofProviderApi,
    MoveToProofAdapterCommand,
    MoveToProofEngine,
    MoveToProofAdapter,
    RemainCargoAllowCore,
    RemainProviderOwned,
    CompatibilityAdapter,
    HistoricalReaderOnly,
    GeneratedProjection,
    DeleteAfterParity,
    DeleteImmediatelyAsDead,
    RepositoryDecisionRequired,
}

impl MoveDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MoveToSharedProtocol => "MoveToSharedProtocol",
            Self::MoveToSharedSnapshot => "MoveToSharedSnapshot",
            Self::MoveToIntentModel => "MoveToIntentModel",
            Self::MoveToIntentProtocol => "MoveToIntentProtocol",
            Self::MoveToIntentEngine => "MoveToIntentEngine",
            Self::MoveToIntentEdit => "MoveToIntentEdit",
            Self::MoveToCargoIntentApp => "MoveToCargoIntentApp",
            Self::MoveToProofProtocol => "MoveToProofProtocol",
            Self::MoveToProofProviderApi => "MoveToProofProviderApi",
            Self::MoveToProofAdapterCommand => "MoveToProofAdapterCommand",
            Self::MoveToProofEngine => "MoveToProofEngine",
            Self::MoveToProofAdapter => "MoveToProofAdapter",
            Self::RemainCargoAllowCore => "RemainCargoAllowCore",
            Self::RemainProviderOwned => "RemainProviderOwned",
            Self::CompatibilityAdapter => "CompatibilityAdapter",
            Self::HistoricalReaderOnly => "HistoricalReaderOnly",
            Self::GeneratedProjection => "GeneratedProjection",
            Self::DeleteAfterParity => "DeleteAfterParity",
            Self::DeleteImmediatelyAsDead => "DeleteImmediatelyAsDead",
            Self::RepositoryDecisionRequired => "RepositoryDecisionRequired",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "MoveToSharedProtocol" => Ok(Self::MoveToSharedProtocol),
            "MoveToSharedSnapshot" => Ok(Self::MoveToSharedSnapshot),
            "MoveToIntentModel" => Ok(Self::MoveToIntentModel),
            "MoveToIntentProtocol" => Ok(Self::MoveToIntentProtocol),
            "MoveToIntentEngine" => Ok(Self::MoveToIntentEngine),
            "MoveToIntentEdit" => Ok(Self::MoveToIntentEdit),
            "MoveToCargoIntentApp" => Ok(Self::MoveToCargoIntentApp),
            "MoveToProofProtocol" => Ok(Self::MoveToProofProtocol),
            "MoveToProofProviderApi" => Ok(Self::MoveToProofProviderApi),
            "MoveToProofAdapterCommand" => Ok(Self::MoveToProofAdapterCommand),
            "MoveToProofEngine" => Ok(Self::MoveToProofEngine),
            "MoveToProofAdapter" => Ok(Self::MoveToProofAdapter),
            "RemainCargoAllowCore" => Ok(Self::RemainCargoAllowCore),
            "RemainProviderOwned" => Ok(Self::RemainProviderOwned),
            "CompatibilityAdapter" => Ok(Self::CompatibilityAdapter),
            "HistoricalReaderOnly" => Ok(Self::HistoricalReaderOnly),
            "GeneratedProjection" => Ok(Self::GeneratedProjection),
            "DeleteAfterParity" => Ok(Self::DeleteAfterParity),
            "DeleteImmediatelyAsDead" => Ok(Self::DeleteImmediatelyAsDead),
            "RepositoryDecisionRequired" => Ok(Self::RepositoryDecisionRequired),
            other => Err(CargoAllowError::new(format!(
                "unsupported move disposition `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveEntryStatus {
    Current,
    Transitional,
    Moved,
    Deleted,
    DecisionRequired,
}

impl MoveEntryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Transitional => "transitional",
            Self::Moved => "moved",
            Self::Deleted => "deleted",
            Self::DecisionRequired => "decision_required",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "current" => Ok(Self::Current),
            "transitional" => Ok(Self::Transitional),
            "moved" => Ok(Self::Moved),
            "deleted" => Ok(Self::Deleted),
            "decision_required" => Ok(Self::DecisionRequired),
            other => Err(CargoAllowError::new(format!(
                "unsupported move entry status `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveIdentityKind {
    RustModuleTree,
    RustModule,
    Schema,
    Command,
    Fixture,
    Workflow,
    Issue,
}

impl MoveIdentityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RustModuleTree => "rust_module_tree",
            Self::RustModule => "rust_module",
            Self::Schema => "schema",
            Self::Command => "command",
            Self::Fixture => "fixture",
            Self::Workflow => "workflow",
            Self::Issue => "issue",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "rust_module_tree" => Ok(Self::RustModuleTree),
            "rust_module" => Ok(Self::RustModule),
            "schema" => Ok(Self::Schema),
            "command" => Ok(Self::Command),
            "fixture" => Ok(Self::Fixture),
            "workflow" => Ok(Self::Workflow),
            "issue" => Ok(Self::Issue),
            other => Err(CargoAllowError::new(format!(
                "unsupported move identity kind `{other}`"
            ))),
        }
    }

    pub fn expects_repo_path(self) -> bool {
        matches!(
            self,
            Self::RustModuleTree | Self::RustModule | Self::Schema | Self::Fixture | Self::Workflow
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveEntry {
    pub id: String,
    pub current_identity: String,
    pub identity_kind: MoveIdentityKind,
    pub current_owner_product: String,
    pub current_owner_crate: String,
    pub target_owner_product: String,
    pub target_owner_crate: String,
    pub disposition: MoveDisposition,
    pub status: MoveEntryStatus,
    pub claim_boundary: String,
    pub parity_fixture: Option<String>,
    pub removal_condition: Option<String>,
    pub controlling_issue: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductMoveLedger {
    pub schema_version: String,
    pub controlling_issue: u32,
    pub ledger_id: String,
    pub linked_plan: String,
    pub linked_adr: String,
    pub entry: Vec<MoveEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProductMoveLedger {
    pub ledger: ProductMoveLedger,
    pub valid: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductMoveLedgerToml {
    schema_version: Option<String>,
    controlling_issue: Option<u32>,
    ledger_id: Option<String>,
    linked_plan: Option<String>,
    linked_adr: Option<String>,
    #[serde(default)]
    entry: Vec<MoveEntryToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveEntryToml {
    id: String,
    current_identity: String,
    identity_kind: String,
    current_owner_product: String,
    current_owner_crate: String,
    target_owner_product: String,
    target_owner_crate: String,
    disposition: String,
    status: String,
    claim_boundary: String,
    parity_fixture: Option<String>,
    removal_condition: Option<String>,
    controlling_issue: Option<u32>,
}

impl ProductMoveLedgerToml {
    fn into_ledger(self) -> CargoAllowResult<ProductMoveLedger> {
        let schema_version = self.schema_version.unwrap_or_else(|| "1.0".to_string());
        let controlling_issue = self
            .controlling_issue
            .ok_or_else(|| CargoAllowError::new("product move ledger missing controlling_issue"))?;
        let linked_plan = self
            .linked_plan
            .ok_or_else(|| CargoAllowError::new("product move ledger missing linked_plan"))?;
        let linked_adr = self
            .linked_adr
            .ok_or_else(|| CargoAllowError::new("product move ledger missing linked_adr"))?;
        let ledger_id = self
            .ledger_id
            .ok_or_else(|| CargoAllowError::new("product move ledger missing ledger_id"))?;

        let mut entry = Vec::with_capacity(self.entry.len());
        for raw in self.entry {
            entry.push(MoveEntry {
                id: raw.id,
                current_identity: raw.current_identity,
                identity_kind: MoveIdentityKind::parse(&raw.identity_kind)?,
                current_owner_product: raw.current_owner_product,
                current_owner_crate: raw.current_owner_crate,
                target_owner_product: raw.target_owner_product,
                target_owner_crate: raw.target_owner_crate,
                disposition: MoveDisposition::parse(&raw.disposition)?,
                status: MoveEntryStatus::parse(&raw.status)?,
                claim_boundary: raw.claim_boundary,
                parity_fixture: raw.parity_fixture,
                removal_condition: raw.removal_condition,
                controlling_issue: raw.controlling_issue,
            });
        }

        Ok(ProductMoveLedger {
            schema_version,
            controlling_issue,
            ledger_id,
            linked_plan,
            linked_adr,
            entry,
        })
    }
}

pub fn parse_product_move_ledger(input: &str) -> CargoAllowResult<ProductMoveLedger> {
    parse_product_move_ledger_at(None, input)
}

pub fn parse_product_move_ledger_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ProductMoveLedger> {
    let parsed = toml::from_str::<ProductMoveLedgerToml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse product move ledger TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    parsed.into_ledger()
}
