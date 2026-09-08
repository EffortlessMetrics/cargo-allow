//! Read-only drift guard over the retained direct minimum version
//! floor receipts (#3903 PR D).
//!
//! Exit contract: only release-set (`cargo-allow`) drift blocks — a
//! stale, unproven, or incomplete release-set proof exits nonzero so a
//! hosted lane can gate on it. Advisory product drift reports the same
//! verdicts and exits zero: report-only products never gate the
//! release set (negative control 10).

use std::path::{Path, PathBuf};

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    MinimumVersionDriftEvaluationV1, MinimumVersionDriftObservationV1,
    MinimumVersionProofReceiptV1, evaluate_minimum_version_drift,
    render_minimum_version_drift_human, render_minimum_version_drift_json,
};
use clap::{Parser, Subcommand};

use crate::minimum_version_selection::{
    ProductSelection, derive_selection, lock_digest, manifest_set_digest, product_roots, products,
    retained_receipt_path, workspace_msrv,
};

/// Read-only direct-floor drift evaluation (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct MinVersionDriftArgs {
    #[command(subcommand)]
    pub(crate) command: MinVersionDriftSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum MinVersionDriftSubcommand {
    /// Grade one product's retained receipt against the live tree.
    #[command(hide = true)]
    Evaluate(MinVersionDriftEvaluateArgs),
    /// Grade every registered product's retained receipt.
    #[command(hide = true)]
    Check(MinVersionDriftCheckArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum MinVersionDriftOutputFormat {
    Json,
    Human,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct MinVersionDriftEvaluateArgs {
    /// The proof product to grade.
    #[arg(long)]
    pub(crate) product: String,
    /// Repository root to observe (defaults to the current directory).
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Retained receipt JSON path. Missing when the file does not
    /// exist; malformed when it exists but does not parse.
    #[arg(long)]
    pub(crate) receipt: Option<PathBuf>,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: MinVersionDriftOutputFormat,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct MinVersionDriftCheckArgs {
    /// Repository root to observe (defaults to the current directory).
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "human")]
    pub(crate) format: MinVersionDriftOutputFormat,
}

pub(super) fn cmd_min_version_drift(args: &MinVersionDriftArgs) -> CargoAllowResult<()> {
    match &args.command {
        MinVersionDriftSubcommand::Evaluate(evaluate) => {
            let observation = build_observation(&evaluate.root, &evaluate.product)?;
            let receipt = load_receipt(evaluate.receipt.as_deref())?;
            let evaluation = evaluate_minimum_version_drift(&observation, receipt.as_ref());
            render(&evaluation, evaluate.format)?;
            finish(evaluation)
        }
        MinVersionDriftSubcommand::Check(check) => {
            let mut evaluations = Vec::new();
            for product in products() {
                let observation = build_observation(&check.root, product)?;
                let receipt = load_receipt(Some(&check.root.join(retained_receipt_path(product))))?;
                evaluations.push(evaluate_minimum_version_drift(
                    &observation,
                    receipt.as_ref(),
                ));
            }
            // One valid machine-readable document: the JSON view is a
            // single array of the evaluations, while the human view
            // renders one block per product.
            if check.format == MinVersionDriftOutputFormat::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&evaluations).map_err(|error| {
                        CargoAllowError::with_kind(
                            CargoAllowErrorKind::InstrumentFailure,
                            format!("evaluation serialization: {error}"),
                        )
                    })?
                );
            } else {
                for evaluation in &evaluations {
                    render(evaluation, check.format)?;
                }
            }
            let blocked: Vec<&MinimumVersionDriftEvaluationV1> = evaluations
                .iter()
                .filter(|evaluation| evaluation.blocking)
                .collect();
            if blocked.is_empty() {
                Ok(())
            } else {
                Err(CargoAllowError::with_kind(
                    CargoAllowErrorKind::PolicyViolation,
                    format!(
                        "release-set floor drift blocks: {}",
                        blocked
                            .iter()
                            .map(|evaluation| format!(
                                "{} {} ({})",
                                evaluation.product,
                                evaluation.verdict.label(),
                                evaluation.reasons.join("; ")
                            ))
                            .collect::<Vec<_>>()
                            .join(" | ")
                    ),
                ))
            }
        }
    }
}

fn build_observation(
    root: &Path,
    product: &str,
) -> CargoAllowResult<MinimumVersionDriftObservationV1> {
    if product_roots(product).is_none() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("product {product} is not in the floor-proof registry"),
        ));
    }
    let selection: ProductSelection = derive_selection(root, product).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("selection derivation for {product}: {error}"),
        )
    })?;
    let observation = MinimumVersionDriftObservationV1 {
        product: product.to_string(),
        package_roots: selection.roots.clone(),
        msrv: workspace_msrv(root).map_err(|error| {
            CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, error)
        })?,
        // The proof lane's constant target posture: the emitter records
        // exactly this string, so a receipt produced under a different
        // target is named by the drift evaluation.
        target: "host (product closure default target)".to_string(),
        manifest_set_digest: manifest_set_digest(root)
            .map_err(|error| CargoAllowError::with_kind(CargoAllowErrorKind::Inventory, error))?,
        lock_digest: lock_digest(root)
            .map_err(|error| CargoAllowError::with_kind(CargoAllowErrorKind::Inventory, error))?,
        floors: selection
            .floors
            .into_iter()
            .map(|(package, declared_requirement, selected_floor)| {
                allow_report::MinimumVersionDriftFloorRowV1 {
                    package,
                    declared_requirement,
                    selected_floor,
                }
            })
            .collect(),
    };
    Ok(observation)
}

/// `None` when no receipt path was given or the file does not exist
/// (an unproven product is a graded state, not an error); a malformed
/// receipt file fails closed.
fn load_receipt(path: Option<&Path>) -> CargoAllowResult<Option<MinimumVersionProofReceiptV1>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Inventory,
            format!("receipt {} reads: {error}", path.display()),
        )
    })?;
    let receipt: MinimumVersionProofReceiptV1 =
        serde_json::from_slice(&bytes).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("receipt {} parses: {error}", path.display()),
            )
        })?;
    Ok(Some(receipt))
}

fn render(
    evaluation: &MinimumVersionDriftEvaluationV1,
    format: MinVersionDriftOutputFormat,
) -> CargoAllowResult<()> {
    match format {
        MinVersionDriftOutputFormat::Json => {
            println!(
                "{}",
                render_minimum_version_drift_json(evaluation).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("evaluation serialization: {error}"),
                    )
                })?
            );
        }
        MinVersionDriftOutputFormat::Human => {
            println!("{}", render_minimum_version_drift_human(evaluation));
        }
    }
    Ok(())
}

fn finish(evaluation: MinimumVersionDriftEvaluationV1) -> CargoAllowResult<()> {
    if evaluation.blocking {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "release-set floor proof is {} for {}; rerun scripts/proof-direct-floors.sh: {}",
                evaluation.verdict.label(),
                evaluation.product,
                evaluation.reasons.join("; ")
            ),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
            .join("../..")
            .canonicalize()
            .expect("workspace root resolves")
    }

    #[test]
    fn evaluate_rejects_an_unregistered_product() {
        let args = MinVersionDriftArgs {
            command: MinVersionDriftSubcommand::Evaluate(MinVersionDriftEvaluateArgs {
                product: "not-a-product".to_string(),
                root: workspace_root(),
                receipt: None,
                format: MinVersionDriftOutputFormat::Human,
            }),
        };
        let outcome = cmd_min_version_drift(&args);
        let error = outcome.expect_err("an unregistered product fails closed");
        assert!(
            error
                .to_string()
                .contains("not in the floor-proof registry"),
            "the error names the registry: {error}"
        );
    }

    #[test]
    fn evaluate_fails_closed_on_a_malformed_receipt() {
        let path =
            std::env::temp_dir().join(format!("min-drift-malformed-{}.json", std::process::id()));
        std::fs::write(&path, b"{ not json }").expect("fixture write succeeds");
        let args = MinVersionDriftArgs {
            command: MinVersionDriftSubcommand::Evaluate(MinVersionDriftEvaluateArgs {
                product: "shared".to_string(),
                root: workspace_root(),
                receipt: Some(path.clone()),
                format: MinVersionDriftOutputFormat::Human,
            }),
        };
        let outcome = cmd_min_version_drift(&args);
        let _ = std::fs::remove_file(&path);
        assert!(outcome.is_err(), "a malformed receipt fails closed");
    }

    #[test]
    fn check_exits_zero_when_every_release_set_receipt_is_current() {
        let args = MinVersionDriftArgs {
            command: MinVersionDriftSubcommand::Check(MinVersionDriftCheckArgs {
                root: workspace_root(),
                format: MinVersionDriftOutputFormat::Human,
            }),
        };
        // The retained receipts on the current tree are current by the
        // live drift test; this asserts the command's exit contract is
        // tied to that same graded state.
        let outcome = cmd_min_version_drift(&args);
        assert!(outcome.is_ok(), "all four retained receipts are current");
    }
}
