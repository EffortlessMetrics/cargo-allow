use allow_core::{CargoAllowError, CargoAllowResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ExtractionStage {
    ArchitectureInventory,
    RepoProtocol,
    RepoSnapshot,
    RepoEdit,
    IntentModel,
    IntentProtocol,
    IntentEngine,
    IntentEdit,
    CargoIntentFrontDoor,
    CargoAllowCompatibilityCutover,
    EmbeddedIntentDeletion,
    RustSourceIndex,
    ProofProtocol,
    ProofProviderApi,
    ProofAdapterCommand,
    ProofEngineAndCli,
    ProviderAdapters,
    IndependentPackaging,
}

impl ExtractionStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArchitectureInventory => "ArchitectureInventory",
            Self::RepoProtocol => "RepoProtocol",
            Self::RepoSnapshot => "RepoSnapshot",
            Self::RepoEdit => "RepoEdit",
            Self::IntentModel => "IntentModel",
            Self::IntentProtocol => "IntentProtocol",
            Self::IntentEngine => "IntentEngine",
            Self::IntentEdit => "IntentEdit",
            Self::CargoIntentFrontDoor => "CargoIntentFrontDoor",
            Self::CargoAllowCompatibilityCutover => "CargoAllowCompatibilityCutover",
            Self::EmbeddedIntentDeletion => "EmbeddedIntentDeletion",
            Self::RustSourceIndex => "RustSourceIndex",
            Self::ProofProtocol => "ProofProtocol",
            Self::ProofProviderApi => "ProofProviderApi",
            Self::ProofAdapterCommand => "ProofAdapterCommand",
            Self::ProofEngineAndCli => "ProofEngineAndCli",
            Self::ProviderAdapters => "ProviderAdapters",
            Self::IndependentPackaging => "IndependentPackaging",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "ArchitectureInventory" => Ok(Self::ArchitectureInventory),
            "RepoProtocol" => Ok(Self::RepoProtocol),
            "RepoSnapshot" => Ok(Self::RepoSnapshot),
            "RepoEdit" => Ok(Self::RepoEdit),
            "IntentModel" => Ok(Self::IntentModel),
            "IntentProtocol" => Ok(Self::IntentProtocol),
            "IntentEngine" => Ok(Self::IntentEngine),
            "IntentEdit" => Ok(Self::IntentEdit),
            "CargoIntentFrontDoor" => Ok(Self::CargoIntentFrontDoor),
            "CargoAllowCompatibilityCutover" => Ok(Self::CargoAllowCompatibilityCutover),
            "EmbeddedIntentDeletion" => Ok(Self::EmbeddedIntentDeletion),
            "RustSourceIndex" => Ok(Self::RustSourceIndex),
            "ProofProtocol" => Ok(Self::ProofProtocol),
            "ProofProviderApi" => Ok(Self::ProofProviderApi),
            "ProofAdapterCommand" => Ok(Self::ProofAdapterCommand),
            "ProofEngineAndCli" => Ok(Self::ProofEngineAndCli),
            "ProviderAdapters" => Ok(Self::ProviderAdapters),
            "IndependentPackaging" => Ok(Self::IndependentPackaging),
            other => Err(CargoAllowError::new(format!(
                "unsupported extraction stage `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityComparisonResult {
    SemanticallyEquivalent,
    EquivalentWithCanonicalRenaming,
    EquivalentWithOrderingNormalization,
    IntentionalDifferenceAccepted,
    OldBehaviorIncorrectAndSuperseded,
    PartialComparison,
    UnsupportedComparison,
    InstrumentFailure,
    SourceIdentityMismatch,
    UnreviewedDifference,
}

impl ParityComparisonResult {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SemanticallyEquivalent => "SemanticallyEquivalent",
            Self::EquivalentWithCanonicalRenaming => "EquivalentWithCanonicalRenaming",
            Self::EquivalentWithOrderingNormalization => "EquivalentWithOrderingNormalization",
            Self::IntentionalDifferenceAccepted => "IntentionalDifferenceAccepted",
            Self::OldBehaviorIncorrectAndSuperseded => "OldBehaviorIncorrectAndSuperseded",
            Self::PartialComparison => "PartialComparison",
            Self::UnsupportedComparison => "UnsupportedComparison",
            Self::InstrumentFailure => "InstrumentFailure",
            Self::SourceIdentityMismatch => "SourceIdentityMismatch",
            Self::UnreviewedDifference => "UnreviewedDifference",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "SemanticallyEquivalent" => Ok(Self::SemanticallyEquivalent),
            "EquivalentWithCanonicalRenaming" => Ok(Self::EquivalentWithCanonicalRenaming),
            "EquivalentWithOrderingNormalization" => Ok(Self::EquivalentWithOrderingNormalization),
            "IntentionalDifferenceAccepted" => Ok(Self::IntentionalDifferenceAccepted),
            "OldBehaviorIncorrectAndSuperseded" => Ok(Self::OldBehaviorIncorrectAndSuperseded),
            "PartialComparison" => Ok(Self::PartialComparison),
            "UnsupportedComparison" => Ok(Self::UnsupportedComparison),
            "InstrumentFailure" => Ok(Self::InstrumentFailure),
            "SourceIdentityMismatch" => Ok(Self::SourceIdentityMismatch),
            "UnreviewedDifference" => Ok(Self::UnreviewedDifference),
            other => Err(CargoAllowError::new(format!(
                "unsupported parity comparison result `{other}`"
            ))),
        }
    }

    pub fn satisfies_migration(self) -> bool {
        matches!(
            self,
            Self::SemanticallyEquivalent
                | Self::EquivalentWithCanonicalRenaming
                | Self::EquivalentWithOrderingNormalization
                | Self::IntentionalDifferenceAccepted
                | Self::OldBehaviorIncorrectAndSuperseded
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityDisposition {
    Pending,
    ContractOnly,
    FixtureSeeded,
    Proven,
}

impl ParityDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::ContractOnly => "contract_only",
            Self::FixtureSeeded => "fixture_seeded",
            Self::Proven => "proven",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "pending" => Ok(Self::Pending),
            "contract_only" => Ok(Self::ContractOnly),
            "fixture_seeded" => Ok(Self::FixtureSeeded),
            "proven" => Ok(Self::Proven),
            other => Err(CargoAllowError::new(format!(
                "unsupported parity disposition `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionParityCase {
    pub id: String,
    pub stage: ExtractionStage,
    pub move_ledger_entry: String,
    pub shim_id: Option<String>,
    pub old_producer: String,
    pub new_producer: String,
    pub expected_result: ParityComparisonResult,
    pub disposition: ParityDisposition,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageReceiptTemplate {
    pub stage: ExtractionStage,
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionParityRegistry {
    pub schema_version: String,
    pub registry_id: String,
    pub controlling_issue: u32,
    pub linked_shim_registry: String,
    pub case: Vec<ExtractionParityCase>,
    pub stage_receipt: Vec<StageReceiptTemplate>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractionParityRegistryToml {
    schema_version: Option<String>,
    registry_id: Option<String>,
    controlling_issue: Option<u32>,
    linked_shim_registry: Option<String>,
    #[serde(default)]
    case: Vec<ExtractionParityCaseToml>,
    #[serde(default)]
    stage_receipt: Vec<StageReceiptTemplateToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractionParityCaseToml {
    id: String,
    stage: String,
    move_ledger_entry: String,
    shim_id: Option<String>,
    old_producer: String,
    new_producer: String,
    expected_result: String,
    disposition: String,
    claim_boundary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageReceiptTemplateToml {
    stage: String,
    required_fields: Vec<String>,
}

impl ExtractionParityRegistryToml {
    fn into_registry(self) -> CargoAllowResult<ExtractionParityRegistry> {
        let registry_id = self
            .registry_id
            .ok_or_else(|| CargoAllowError::new("parity registry missing registry_id"))?;
        let controlling_issue = self
            .controlling_issue
            .ok_or_else(|| CargoAllowError::new("parity registry missing controlling_issue"))?;
        let linked_shim_registry = self
            .linked_shim_registry
            .ok_or_else(|| CargoAllowError::new("parity registry missing linked_shim_registry"))?;

        let mut case = Vec::with_capacity(self.case.len());
        for entry in self.case {
            case.push(ExtractionParityCase {
                id: entry.id,
                stage: ExtractionStage::parse(&entry.stage)?,
                move_ledger_entry: entry.move_ledger_entry,
                shim_id: entry.shim_id,
                old_producer: entry.old_producer,
                new_producer: entry.new_producer,
                expected_result: ParityComparisonResult::parse(&entry.expected_result)?,
                disposition: ParityDisposition::parse(&entry.disposition)?,
                claim_boundary: entry.claim_boundary,
            });
        }

        let mut stage_receipt = Vec::with_capacity(self.stage_receipt.len());
        for entry in self.stage_receipt {
            stage_receipt.push(StageReceiptTemplate {
                stage: ExtractionStage::parse(&entry.stage)?,
                required_fields: entry.required_fields,
            });
        }

        Ok(ExtractionParityRegistry {
            schema_version: self.schema_version.unwrap_or_else(|| "1.0".to_string()),
            registry_id,
            controlling_issue,
            linked_shim_registry,
            case,
            stage_receipt,
        })
    }
}

pub fn parse_extraction_parity_registry(input: &str) -> CargoAllowResult<ExtractionParityRegistry> {
    parse_extraction_parity_registry_at(None, input)
}

pub fn parse_extraction_parity_registry_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ExtractionParityRegistry> {
    let parsed = toml::from_str::<ExtractionParityRegistryToml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse extraction parity registry TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    parsed.into_registry()
}
