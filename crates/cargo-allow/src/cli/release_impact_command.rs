//! The release-impact routing CLI (#3794): classify one exact
//! base/head/merge-base subject into the typed `ReleaseImpactReceiptV1`,
//! validate a receipt against the expected subject, record the
//! rehearsal observation, and evaluate the always-run terminal routing
//! aggregate.
//!
//! Exit contracts mirror the routing law, not a gate:
//! - `classify` exits zero on any completed classification — routing
//!   selects proof, it does not block;
//! - `validate` exits nonzero only on an invalid or foreign-subject
//!   receipt (the rehearsal job refuses to run against unbound input);
//! - `observe` exits nonzero when invocation evidence is missing,
//!   incomplete, or foreign — an observation can never be greener than
//!   its evidence;
//! - `aggregate` prints the typed terminal result first and exits zero
//!   only for a Passed aggregate; missing, skipped, cancelled, stale,
//!   contradictory, or malformed proof fails it.

use std::path::{Path, PathBuf};

use allow_core::sha256_v1_bytes;
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    CargoAllowReleaseImpactReceiptV1, ReleaseImpactClassifierInputV1, ReleaseImpactSubjectV1,
    ReleaseRehearsalObservationOutcomeV1, classify_release_impact_v1,
    evaluate_release_rehearsal_required_v1, is_git_object_name, observe_release_rehearsal_v1,
    release_impact_denominator, render_release_impact_receipt_human,
    render_release_impact_receipt_json, render_release_rehearsal_observation_human,
    render_release_rehearsal_observation_json, render_release_rehearsal_required_human,
    render_release_rehearsal_required_json, validate_release_impact_receipt_v1,
};
use clap::{Parser, Subcommand};

/// Release-impact rehearsal routing (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReleaseImpactArgs {
    #[command(subcommand)]
    pub(crate) command: ReleaseImpactSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReleaseImpactSubcommand {
    /// Classify one exact subject into the typed release-impact receipt.
    #[command(hide = true)]
    Classify(ReleaseImpactClassifyArgs),
    /// Validate a receipt against the exact expected subject.
    #[command(hide = true)]
    Validate(ReleaseImpactValidateArgs),
    /// Record the checked rehearsal observation for one subject.
    #[command(hide = true)]
    Observe(ReleaseImpactObserveArgs),
    /// Evaluate the always-run terminal routing aggregate.
    #[command(hide = true)]
    Aggregate(ReleaseImpactAggregateArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReleaseImpactClassifyArgs {
    /// Exact base commit SHA (40/64 hex).
    #[arg(long)]
    pub(crate) base: String,
    /// Exact reviewed head SHA (40/64 hex).
    #[arg(long)]
    pub(crate) head: String,
    /// Exact merge base SHA (40/64 hex).
    #[arg(long)]
    pub(crate) merge_base: String,
    /// File with one forward-slash repository-relative changed path per
    /// line (`git diff --name-only <merge-base> <head>` output).
    #[arg(long)]
    pub(crate) paths_from: PathBuf,
    /// Repository root the denominator compiles against.
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Write the receipt JSON here (stdout keeps the human/JSON view).
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Output rendering.
    #[arg(long, default_value = "human")]
    pub(crate) format: ReleaseImpactOutputFormat,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReleaseImpactValidateArgs {
    /// Impact receipt JSON to validate.
    #[arg(long)]
    pub(crate) impact_receipt: PathBuf,
    /// Exact expected base commit SHA.
    #[arg(long)]
    pub(crate) expected_base: String,
    /// Exact expected reviewed head SHA.
    #[arg(long)]
    pub(crate) expected_head: String,
    /// Exact expected merge base SHA.
    #[arg(long)]
    pub(crate) expected_merge_base: String,
    /// Output rendering.
    #[arg(long, default_value = "human")]
    pub(crate) format: ReleaseImpactOutputFormat,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReleaseImpactObserveArgs {
    /// Validated impact receipt JSON the observation binds to.
    #[arg(long)]
    pub(crate) impact_receipt: PathBuf,
    /// What the rehearsal job actually did.
    #[arg(long)]
    pub(crate) outcome: ReleaseImpactObservationOutcomeArg,
    /// Recorded harness exit code (required for --outcome executed).
    #[arg(long)]
    pub(crate) harness_exit_code: Option<i32>,
    /// Harness receipt JSON produced by the invocation (required for
    /// --outcome executed).
    #[arg(long)]
    pub(crate) harness_receipt: Option<PathBuf>,
    /// Write the observation JSON here.
    #[arg(long)]
    pub(crate) output: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "human")]
    pub(crate) format: ReleaseImpactOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReleaseImpactObservationOutcomeArg {
    Executed,
    NotApplicable,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReleaseImpactAggregateArgs {
    /// Impact receipt JSON.
    #[arg(long)]
    pub(crate) impact_receipt: PathBuf,
    /// Rehearsal observation JSON produced by the rehearsal job.
    #[arg(long)]
    pub(crate) rehearsal_observation: PathBuf,
    /// Exact expected base commit SHA.
    #[arg(long)]
    pub(crate) expected_base: String,
    /// Exact expected reviewed head SHA.
    #[arg(long)]
    pub(crate) expected_head: String,
    /// Exact expected merge base SHA.
    #[arg(long)]
    pub(crate) expected_merge_base: String,
    /// Write the terminal result JSON here.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Output rendering.
    #[arg(long, default_value = "human")]
    pub(crate) format: ReleaseImpactOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReleaseImpactOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_release_impact(args: &ReleaseImpactArgs) -> CargoAllowResult<()> {
    match &args.command {
        ReleaseImpactSubcommand::Classify(classify) => cmd_classify(classify),
        ReleaseImpactSubcommand::Validate(validate) => cmd_validate(validate),
        ReleaseImpactSubcommand::Observe(observe) => cmd_observe(observe),
        ReleaseImpactSubcommand::Aggregate(aggregate) => cmd_aggregate(aggregate),
    }
}

fn read_bytes(path: &PathBuf, label: &str) -> CargoAllowResult<Vec<u8>> {
    std::fs::read(path).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{label} read {}: {error}", path.display()),
        )
    })
}

fn parse_json<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    label: &str,
    path: &Path,
) -> CargoAllowResult<T> {
    serde_json::from_slice(bytes).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{label} parse {}: {error}", path.display()),
        )
    })
}

fn invalid_input(message: String) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, message)
}

fn expected_subject(
    base: &str,
    head: &str,
    merge_base: &str,
) -> CargoAllowResult<ReleaseImpactSubjectV1> {
    for (field, sha) in [
        ("expected-base", base),
        ("expected-head", head),
        ("expected-merge-base", merge_base),
    ] {
        if !is_git_object_name(sha) {
            return Err(invalid_input(format!(
                "{field} is not a lowercase 40/64-hex Git object name: {sha:?}"
            )));
        }
    }
    Ok(ReleaseImpactSubjectV1 {
        base_sha: base.to_string(),
        head_sha: head.to_string(),
        merge_base_sha: merge_base.to_string(),
    })
}

fn write_output(path: &Option<PathBuf>, text: &str) -> CargoAllowResult<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Artifact,
                    format!("create {}: {error}", parent.display()),
                )
            })?;
        }
        std::fs::write(path, text).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("write {}: {error}", path.display()),
            )
        })?;
    }
    Ok(())
}

fn cmd_classify(args: &ReleaseImpactClassifyArgs) -> CargoAllowResult<()> {
    let subject = expected_subject(&args.base, &args.head, &args.merge_base)?;
    let paths_bytes = read_bytes(&args.paths_from, "changed-paths file")?;
    let changed_paths: Vec<String> = String::from_utf8(paths_bytes)
        .map_err(|error| invalid_input(format!("changed-paths file is not UTF-8: {error}")))?
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let denominator = release_impact_denominator(&args.root).map_err(invalid_input)?;
    let input = ReleaseImpactClassifierInputV1 {
        subject,
        changed_paths,
    };
    let receipt = classify_release_impact_v1(&input, &denominator).map_err(invalid_input)?;
    let rendered = render_release_impact_receipt_json(&receipt)
        .map_err(|error| invalid_input(format!("receipt serialization: {error}")))?;
    write_output(&args.output, &rendered)?;
    match args.format {
        ReleaseImpactOutputFormat::Json => {
            println!("{rendered}");
        }
        ReleaseImpactOutputFormat::Human => {
            println!("{}", render_release_impact_receipt_human(&receipt));
        }
    }
    // Routing selects proof; a completed classification never blocks.
    Ok(())
}

fn cmd_validate(args: &ReleaseImpactValidateArgs) -> CargoAllowResult<()> {
    let bytes = read_bytes(&args.impact_receipt, "impact receipt")?;
    let receipt: CargoAllowReleaseImpactReceiptV1 =
        parse_json(&bytes, "impact receipt", &args.impact_receipt)?;
    validate_release_impact_receipt_v1(&receipt).map_err(invalid_input)?;
    let expected = expected_subject(
        &args.expected_base,
        &args.expected_head,
        &args.expected_merge_base,
    )?;
    if receipt.head_sha != expected.head_sha
        || receipt.base_sha != expected.base_sha
        || receipt.merge_base_sha != expected.merge_base_sha
    {
        return Err(invalid_input(format!(
            "impact receipt binds base {} head {} merge-base {}, not the expected subject base {} head {} merge-base {}",
            receipt.base_sha,
            receipt.head_sha,
            receipt.merge_base_sha,
            expected.base_sha,
            expected.head_sha,
            expected.merge_base_sha,
        )));
    }
    match args.format {
        ReleaseImpactOutputFormat::Json => {
            println!(
                "{}",
                render_release_impact_receipt_json(&receipt).map_err(|error| {
                    invalid_input(format!("receipt serialization: {error}"))
                })?
            );
        }
        ReleaseImpactOutputFormat::Human => {
            println!("Release impact receipt is valid and bound to the expected subject.",);
            println!("{}", render_release_impact_receipt_human(&receipt));
        }
    }
    Ok(())
}

fn cmd_observe(args: &ReleaseImpactObserveArgs) -> CargoAllowResult<()> {
    let bytes = read_bytes(&args.impact_receipt, "impact receipt")?;
    let receipt: CargoAllowReleaseImpactReceiptV1 =
        parse_json(&bytes, "impact receipt", &args.impact_receipt)?;
    let digest = sha256_v1_bytes(&bytes);
    let outcome = match args.outcome {
        ReleaseImpactObservationOutcomeArg::Executed => {
            ReleaseRehearsalObservationOutcomeV1::Executed
        }
        ReleaseImpactObservationOutcomeArg::NotApplicable => {
            ReleaseRehearsalObservationOutcomeV1::NotApplicable
        }
    };
    let harness_receipt = match &args.harness_receipt {
        Some(path) => {
            let receipt_bytes = read_bytes(path, "harness receipt")?;
            Some(parse_json::<serde_json::Value>(
                &receipt_bytes,
                "harness receipt",
                path,
            )?)
        }
        None => None,
    };
    let observation = observe_release_rehearsal_v1(
        &receipt,
        &digest,
        outcome,
        args.harness_exit_code,
        harness_receipt.as_ref(),
    )
    .map_err(invalid_input)?;
    let rendered = render_release_rehearsal_observation_json(&observation)
        .map_err(|error| invalid_input(format!("observation serialization: {error}")))?;
    write_output(&Some(args.output.clone()), &rendered)?;
    match args.format {
        ReleaseImpactOutputFormat::Json => {
            println!("{rendered}");
        }
        ReleaseImpactOutputFormat::Human => {
            println!(
                "{}",
                render_release_rehearsal_observation_human(&observation)
            );
        }
    }
    Ok(())
}

fn cmd_aggregate(args: &ReleaseImpactAggregateArgs) -> CargoAllowResult<()> {
    let expected = expected_subject(
        &args.expected_base,
        &args.expected_head,
        &args.expected_merge_base,
    )?;
    // A missing or malformed artifact must still yield the typed
    // terminal result: the evidence is never masked by the exit.
    let impact_bytes = read_bytes(&args.impact_receipt, "impact receipt");
    let impact_digest = impact_bytes
        .as_ref()
        .map(|bytes| sha256_v1_bytes(bytes))
        .unwrap_or_else(|_| "sha256:v1:unavailable".to_string());
    let impact_result: Result<CargoAllowReleaseImpactReceiptV1, String> = impact_bytes
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            serde_json::from_slice::<CargoAllowReleaseImpactReceiptV1>(&bytes)
                .map_err(|error| error.to_string())
        });
    let observation_result: Result<allow_report::ReleaseRehearsalObservationV1, String> =
        read_bytes(&args.rehearsal_observation, "rehearsal observation")
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<allow_report::ReleaseRehearsalObservationV1>(&bytes)
                    .map_err(|error| error.to_string())
            });

    let result = evaluate_release_rehearsal_required_v1(
        impact_result.as_ref().map_err(|error| error.clone()),
        Some(impact_digest.as_str()),
        Some(observation_result.as_ref().map_err(|error| error.clone())),
        &expected,
    );

    let rendered = render_release_rehearsal_required_json(&result)
        .map_err(|error| invalid_input(format!("aggregate serialization: {error}")))?;
    write_output(&args.output, &rendered)?;
    match args.format {
        ReleaseImpactOutputFormat::Json => {
            println!("{rendered}");
        }
        ReleaseImpactOutputFormat::Human => {
            println!("{}", render_release_rehearsal_required_human(&result));
        }
    }

    // The typed terminal result is always emitted first; only a Failed
    // aggregate exits nonzero so the required-check surface sees it.
    if result.aggregate == allow_report::ReleaseRehearsalRequiredAggregateV1::Passed {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "release rehearsal routing aggregate failed: {}",
                result.failure_reasons.join("; ")
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const MERGE_BASE: &str = "cccccccccccccccccccccccccccccccccccccccc";

    struct TempFile(std::path::PathBuf);

    impl TempFile {
        fn write(label: &str, text: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "release-impact-cmd-{}-{}-{label}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or_default(),
            ));
            std::fs::write(&path, text).unwrap_or_else(|err| {
                std::panic::panic_any(format!("fixture write {label}: {err}"))
            });
            Self(path)
        }

        fn path(&self) -> PathBuf {
            self.0.clone()
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn receipt_json(paths: &[&str]) -> String {
        let root = workspace_root();
        let denominator = release_impact_denominator(&root)
            .unwrap_or_else(|error| std::panic::panic_any(format!("denominator: {error}")));
        let input = ReleaseImpactClassifierInputV1 {
            subject: ReleaseImpactSubjectV1 {
                base_sha: BASE.to_string(),
                head_sha: HEAD.to_string(),
                merge_base_sha: MERGE_BASE.to_string(),
            },
            changed_paths: paths.iter().map(|path| (*path).to_string()).collect(),
        };
        let receipt = classify_release_impact_v1(&input, &denominator)
            .unwrap_or_else(|error| std::panic::panic_any(format!("classify: {error}")));
        render_release_impact_receipt_json(&receipt)
            .unwrap_or_else(|error| std::panic::panic_any(format!("render: {error}")))
    }

    fn harness_receipt_json() -> String {
        serde_json::json!({
            "schema_version": "1.0",
            "receipt_id": "rehearsal-fixture",
            "commit_sha": HEAD,
            "aggregate_status": "Incomplete",
            "phases": {"release_identity": "Complete"}
        })
        .to_string()
    }

    #[test]
    fn classify_writes_and_renders_the_live_denominator_decision() {
        let paths = TempFile::write("paths", "docs/README.md\nCargo.lock\n");
        let output = TempFile::write("out", "");
        let args = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Classify(ReleaseImpactClassifyArgs {
                base: BASE.to_string(),
                head: HEAD.to_string(),
                merge_base: MERGE_BASE.to_string(),
                paths_from: paths.path(),
                root: workspace_root(),
                output: Some(output.path()),
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        let outcome = cmd_release_impact(&args);
        assert!(
            outcome.is_ok(),
            "a completed classification exits zero: {outcome:?}"
        );
        let written = std::fs::read_to_string(output.path())
            .unwrap_or_else(|err| std::panic::panic_any(format!("read back: {err}")));
        assert!(written.contains("\"schema_id\": \"cargo-allow.release-impact.v1\""));
        assert!(written.contains("\"rehearsal_required\": true"));
    }

    #[test]
    fn classify_rejects_a_non_hex_subject() {
        let paths = TempFile::write("paths", "docs/README.md\n");
        let args = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Classify(ReleaseImpactClassifyArgs {
                base: "HEAD".to_string(),
                head: HEAD.to_string(),
                merge_base: MERGE_BASE.to_string(),
                paths_from: paths.path(),
                root: workspace_root(),
                output: None,
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(cmd_release_impact(&args).is_err());
    }

    #[test]
    fn validate_accepts_the_bound_subject_and_rejects_a_foreign_one() {
        let receipt = TempFile::write("receipt", &receipt_json(&["Cargo.lock"]));
        let bound = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Validate(ReleaseImpactValidateArgs {
                impact_receipt: receipt.path(),
                expected_base: BASE.to_string(),
                expected_head: HEAD.to_string(),
                expected_merge_base: MERGE_BASE.to_string(),
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(cmd_release_impact(&bound).is_ok());
        let moved = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Validate(ReleaseImpactValidateArgs {
                impact_receipt: receipt.path(),
                expected_base: BASE.to_string(),
                expected_head: HEAD.to_string(),
                expected_merge_base: "dddddddddddddddddddddddddddddddddddddddd".to_string(),
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(cmd_release_impact(&moved).is_err());
    }

    #[test]
    fn observe_records_execution_and_aggregate_binds_the_pair() {
        let receipt = TempFile::write("receipt", &receipt_json(&["Cargo.lock"]));
        let harness = TempFile::write("harness", &harness_receipt_json());
        let observation = TempFile::write("observation", "");
        let observe = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Observe(ReleaseImpactObserveArgs {
                impact_receipt: receipt.path(),
                outcome: ReleaseImpactObservationOutcomeArg::Executed,
                harness_exit_code: Some(1),
                harness_receipt: Some(harness.path()),
                output: observation.path(),
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(cmd_release_impact(&observe).is_ok());
        let aggregate = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Aggregate(ReleaseImpactAggregateArgs {
                impact_receipt: receipt.path(),
                rehearsal_observation: observation.path(),
                expected_base: BASE.to_string(),
                expected_head: HEAD.to_string(),
                expected_merge_base: MERGE_BASE.to_string(),
                output: None,
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(
            cmd_release_impact(&aggregate).is_ok(),
            "a bound executed pair passes"
        );
    }

    #[test]
    fn aggregate_fails_when_the_observation_is_missing_or_foreign() {
        let receipt = TempFile::write("receipt", &receipt_json(&["Cargo.lock"]));
        let harness = TempFile::write("harness", &harness_receipt_json());
        let other_observation = TempFile::write("other", "");
        // Simulate a skipped rehearsal: no observation artifact was
        // produced; the aggregate input is malformed/empty.
        let empty = TempFile::write("empty", "");
        let missing = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Aggregate(ReleaseImpactAggregateArgs {
                impact_receipt: receipt.path(),
                rehearsal_observation: empty.path(),
                expected_base: BASE.to_string(),
                expected_head: HEAD.to_string(),
                expected_merge_base: MERGE_BASE.to_string(),
                output: None,
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(
            cmd_release_impact(&missing).is_err(),
            "missing proof is not clean"
        );

        // A foreign observation (compiled from a different impact
        // artifact) cannot ride along.
        let unrelated_receipt = TempFile::write("unrelated", &receipt_json(&["docs/README.md"]));
        let observe = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Observe(ReleaseImpactObserveArgs {
                impact_receipt: unrelated_receipt.path(),
                outcome: ReleaseImpactObservationOutcomeArg::NotApplicable,
                harness_exit_code: None,
                harness_receipt: None,
                output: other_observation.path(),
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(cmd_release_impact(&observe).is_ok());
        let foreign = ReleaseImpactArgs {
            command: ReleaseImpactSubcommand::Aggregate(ReleaseImpactAggregateArgs {
                impact_receipt: receipt.path(),
                rehearsal_observation: other_observation.path(),
                expected_base: BASE.to_string(),
                expected_head: HEAD.to_string(),
                expected_merge_base: MERGE_BASE.to_string(),
                output: None,
                format: ReleaseImpactOutputFormat::Human,
            }),
        };
        assert!(
            cmd_release_impact(&foreign).is_err(),
            "a sibling's receipt cannot authorize this subject"
        );
        let _ = &harness;
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|err| {
            std::panic::panic_any(format!("CARGO_MANIFEST_DIR is set: {err}"))
        }))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("workspace root resolves: {err}")))
    }
}
