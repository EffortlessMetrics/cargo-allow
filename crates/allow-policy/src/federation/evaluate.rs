use std::path::{Path, PathBuf};

use allow_core::{CargoAllowError, CargoAllowResult, LedgerProvenance};

use super::config::{LedgerEntry, LedgerRole, ValidatedFederationConfig};
use super::load::{FederationLoadOutcome, load_federation_config};
use super::precedence::ordered_ledgers_by_precedence;
use super::FederationConfig;
use crate::discover_config;
use crate::SkippedPolicyCandidate;

pub const FEDERATION_VERSION: &str = "1";
pub const SOURCE_EXCEPTION_LANE: &str = "source-exception";
pub const SPEC_SYSTEM_LANE: &str = "spec-system";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecedenceTier {
    CliOverride,
    FederationRegistry,
    DiscoveryFallback,
}

impl PrecedenceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CliOverride => "cli_override",
            Self::FederationRegistry => "federation_registry",
            Self::DiscoveryFallback => "discovery_fallback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerContributor {
    pub id: String,
    pub path: String,
    pub role: LedgerRole,
    pub dialect: String,
    pub mode: allow_core::LaneEnforcementMode,
    pub lanes: Vec<String>,
    pub priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederationEvaluation {
    pub federation_version: &'static str,
    pub precedence_applied: PrecedenceTier,
    pub active_provenance: Option<LedgerProvenance>,
    pub ledger_contributors: Vec<LedgerContributor>,
}

pub fn canonical_ledgers_in_precedence_order(config: &FederationConfig) -> Vec<&LedgerEntry> {
    ordered_ledgers_by_precedence(&config.ledgers)
        .into_iter()
        .filter(|ledger| ledger.role == LedgerRole::Canonical)
        .collect()
}

pub fn resolve_canonical_ledger_for_lane<'a>(
    config: &'a FederationConfig,
    lane: &str,
) -> Option<&'a LedgerEntry> {
    canonical_ledgers_in_precedence_order(config)
        .into_iter()
        .find(|ledger| ledger.lanes.iter().any(|registered| registered == lane))
}

pub fn ledger_contributors_from_config(config: &FederationConfig) -> Vec<LedgerContributor> {
    canonical_ledgers_in_precedence_order(config)
        .into_iter()
        .map(LedgerContributor::from_ledger)
        .collect()
}

pub fn ledger_provenance_from_entry(ledger: &LedgerEntry, lane: &str) -> LedgerProvenance {
    LedgerProvenance {
        ledger_id: ledger.id.clone(),
        ledger_path: ledger.path.clone(),
        lane: lane.to_string(),
        mode: ledger.mode.as_str().to_string(),
        role: ledger.role.as_str().to_string(),
    }
}

impl LedgerContributor {
    pub fn from_ledger(ledger: &LedgerEntry) -> Self {
        Self {
            id: ledger.id.clone(),
            path: ledger.path.clone(),
            role: ledger.role,
            dialect: ledger.dialect.clone(),
            mode: ledger.mode,
            lanes: ledger.lanes.clone(),
            priority: ledger.priority,
        }
    }
}

pub fn evaluate_source_exception_policy(
    root: &Path,
    cli_config: Option<&Path>,
) -> CargoAllowResult<(PathBuf, FederationEvaluation)> {
    let contributors = load_ledger_contributors(root)?;
    if let Some(config) = cli_config {
        let path = root.join(config);
        let active_provenance = contributors
            .iter()
            .find(|contributor| contributor.path == normalize_repo_relative(config))
            .map(|contributor| {
                ledger_provenance_from_lane_contributor(contributor, SOURCE_EXCEPTION_LANE)
            });
        return Ok((
            path,
            FederationEvaluation {
                federation_version: FEDERATION_VERSION,
                precedence_applied: PrecedenceTier::CliOverride,
                active_provenance,
                ledger_contributors: contributors,
            },
        ));
    }

    if let Some(validated) = load_validated_federation_config(root)? {
        if validated.valid {
            if let Some(ledger) =
                resolve_canonical_ledger_for_lane(&validated.config, SOURCE_EXCEPTION_LANE)
            {
                let path = root.join(&ledger.path);
                return Ok((
                    path,
                    FederationEvaluation {
                        federation_version: FEDERATION_VERSION,
                        precedence_applied: PrecedenceTier::FederationRegistry,
                        active_provenance: Some(ledger_provenance_from_entry(
                            ledger,
                            SOURCE_EXCEPTION_LANE,
                        )),
                        ledger_contributors: ledger_contributors_from_config(&validated.config),
                    },
                ));
            }
        }
    }

    let discovery = discover_config(root);
    let path = discovery
        .selected
        .ok_or_else(|| missing_policy_config_error(&discovery.skipped))?;
    let active_provenance = contributors
        .iter()
        .find(|contributor| contributor.path == normalize_repo_relative_path(&path, root))
        .map(|contributor| {
            ledger_provenance_from_lane_contributor(contributor, SOURCE_EXCEPTION_LANE)
        });
    Ok((
        path,
        FederationEvaluation {
            federation_version: FEDERATION_VERSION,
            precedence_applied: PrecedenceTier::DiscoveryFallback,
            active_provenance,
            ledger_contributors: contributors,
        },
    ))
}

pub fn evaluate_spec_system_ledger(root: &Path) -> Option<FederationEvaluation> {
    let validated = load_validated_federation_config(root).ok()??;
    if !validated.valid {
        return None;
    }
    let ledger = resolve_canonical_ledger_for_lane(&validated.config, SPEC_SYSTEM_LANE)?;
    Some(FederationEvaluation {
        federation_version: FEDERATION_VERSION,
        precedence_applied: PrecedenceTier::FederationRegistry,
        active_provenance: Some(ledger_provenance_from_entry(ledger, SPEC_SYSTEM_LANE)),
        ledger_contributors: ledger_contributors_from_config(&validated.config),
    })
}

fn load_ledger_contributors(root: &Path) -> CargoAllowResult<Vec<LedgerContributor>> {
    Ok(load_validated_federation_config(root)?
        .map(|validated| ledger_contributors_from_config(&validated.config))
        .unwrap_or_default())
}

fn load_validated_federation_config(
    root: &Path,
) -> CargoAllowResult<Option<ValidatedFederationConfig>> {
    let loaded = load_federation_config(root)?;
    Ok(match loaded.outcome {
        FederationLoadOutcome::Missing => None,
        FederationLoadOutcome::Parsed(validated) => Some(validated),
    })
}

fn ledger_provenance_from_lane_contributor(
    contributor: &LedgerContributor,
    lane: &str,
) -> LedgerProvenance {
    LedgerProvenance {
        ledger_id: contributor.id.clone(),
        ledger_path: contributor.path.clone(),
        lane: lane.to_string(),
        mode: contributor.mode.as_str().to_string(),
        role: contributor.role.as_str().to_string(),
    }
}

fn normalize_repo_relative(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalize_repo_relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

fn missing_policy_config_error(skipped: &[SkippedPolicyCandidate]) -> CargoAllowError {
    if skipped.is_empty() {
        return CargoAllowError::new(
            "no policy config found; run `cargo-allow init` or pass --config",
        );
    }
    let details = skipped
        .iter()
        .map(|candidate| format!("{} ({})", candidate.path.display(), candidate.reason))
        .collect::<Vec<_>>()
        .join("; ");
    CargoAllowError::new(format!(
        "no cargo-allow policy config found; skipped {} foreign-dialect candidate(s): {}; run `cargo-allow init` or pass --config",
        skipped.len(),
        details
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn evaluate_source_exception_policy_uses_federation_registry_for_lane() {
        let root = fixture_root("federation-eval-source");
        write_federation_config(
            &root,
            r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10

[[ledgers]]
id = "doc-artifacts"
path = ".allow/artifacts/doc-artifacts.toml"
dialect = "cargo-allow-doc-artifacts"
role = "canonical"
lanes = ["spec-system"]
priority = 20
"#,
        );
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::write(root.join("policy/allow.toml"), "schema_version = \"1.0\"\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

        let (path, evaluation) = evaluate_source_exception_policy(&root, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("evaluate federation policy: {err}")));

        assert_eq!(path, root.join("policy/allow.toml"));
        assert_eq!(
            evaluation.precedence_applied,
            PrecedenceTier::FederationRegistry
        );
        assert_eq!(evaluation.ledger_contributors.len(), 2);
        assert_eq!(evaluation.ledger_contributors[0].id, "source-policy");
        assert_eq!(evaluation.ledger_contributors[1].id, "doc-artifacts");
        let provenance = evaluation
            .active_provenance
            .unwrap_or_else(|| std::panic::panic_any("expected active provenance"));
        assert_eq!(provenance.ledger_id, "source-policy");
        assert_eq!(provenance.lane, SOURCE_EXCEPTION_LANE);
        assert_eq!(provenance.role, "canonical");
        cleanup_fixture(&root);
    }

    #[test]
    fn evaluate_source_exception_policy_honors_cli_override() {
        let root = fixture_root("federation-eval-cli");
        write_federation_config(
            &root,
            r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
priority = 10
"#,
        );
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::write(root.join("policy/allow.toml"), "schema_version = \"1.0\"\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("allow write: {err}")));
        fs::write(root.join("policy/cargo-allow.toml"), "schema_version = \"1.0\"\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("native write: {err}")));

        let (path, evaluation) =
            evaluate_source_exception_policy(&root, Some(Path::new("policy/cargo-allow.toml")))
                .unwrap_or_else(|err| std::panic::panic_any(format!("evaluate cli override: {err}")));

        assert_eq!(path, root.join("policy/cargo-allow.toml"));
        assert_eq!(evaluation.precedence_applied, PrecedenceTier::CliOverride);
        cleanup_fixture(&root);
    }

    #[test]
    fn evaluate_spec_system_ledger_returns_doc_artifacts_provenance() {
        let root = fixture_root("federation-eval-spec-system");
        write_federation_config(
            &root,
            r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
priority = 10

[[ledgers]]
id = "doc-artifacts"
path = ".allow/artifacts/doc-artifacts.toml"
dialect = "cargo-allow-doc-artifacts"
role = "canonical"
lanes = ["spec-system"]
priority = 20
"#,
        );

        let evaluation = evaluate_spec_system_ledger(&root)
            .unwrap_or_else(|| std::panic::panic_any("expected spec-system federation evaluation"));
        let provenance = evaluation
            .active_provenance
            .unwrap_or_else(|| std::panic::panic_any("expected active provenance"));
        assert_eq!(provenance.ledger_id, "doc-artifacts");
        assert_eq!(provenance.lane, SPEC_SYSTEM_LANE);
        assert_eq!(evaluation.ledger_contributors.len(), 2);
        cleanup_fixture(&root);
    }

    fn write_federation_config(root: &Path, text: &str) {
        fs::create_dir_all(root.join(".allow"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("allow dir: {err}")));
        fs::write(root.join(".allow/config.toml"), text)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config write: {err}")));
    }

    fn fixture_root(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "cargo-allow-{label}-{}-{stamp}",
            std::process::id()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .unwrap_or_else(|err| std::panic::panic_any(format!("reset fixture dir: {err}")));
        }
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
        dir
    }

    fn cleanup_fixture(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }
}
