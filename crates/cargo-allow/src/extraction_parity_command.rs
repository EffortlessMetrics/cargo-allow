use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::extraction_parity::{ParityComparison, ParityComparisonResult, corpus_digest};
use clap::{Args, ValueEnum};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{RootArgs, current_dir, emit_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ParityStageArg {
    All,
    RepoSnapshot,
    RepoEdit,
}

#[derive(Debug, Args)]
pub(crate) struct ParityArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Extraction stage to execute.
    #[arg(long, value_enum, default_value_t = ParityStageArg::All)]
    pub(crate) stage: ParityStageArg,
    /// Write the runtime evidence JSON to this path instead of stdout.
    #[arg(long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
}

pub(crate) fn cmd_parity(args: &ParityArgs) -> CargoAllowResult<()> {
    let root = resolve_source_tree_root(args.root.root.as_deref(), current_dir()?)?;
    let initial_source_identity = source_identity(&root)?;
    let mut records = Vec::new();

    if matches!(
        args.stage,
        ParityStageArg::All | ParityStageArg::RepoSnapshot
    ) {
        let run = crate::extraction_parity_runtime::run_repo_snapshot_parity(&root)?;
        append_snapshot_case(
            &mut records,
            "parity-repo-snapshot-revision-identity-v1",
            run.committed,
        );
        append_snapshot_case(
            &mut records,
            "parity-repo-snapshot-staged-index-v1",
            run.staged,
        );
    }
    if matches!(args.stage, ParityStageArg::All | ParityStageArg::RepoEdit) {
        let run = crate::extraction_repo_edit_runtime::run_repo_edit_parity()?;
        for case in run.cases {
            append_record(
                &mut records,
                case.id,
                case.comparison,
                case.old_output,
                case.new_output,
            );
        }
    }

    if source_identity(&root)? != initial_source_identity {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            "source Git commit/tree changed during parity execution",
        ));
    }

    let registry = load_registry(&root)?;
    let expected_case_ids: BTreeSet<_> = registry
        .case
        .iter()
        .filter(|case| stage_matches(case.stage.as_str(), args.stage))
        .map(|case| case.id.as_str())
        .collect();
    let actual_case_ids: BTreeSet<_> = records
        .iter()
        .filter_map(|record| record.get("case_id").and_then(Value::as_str))
        .collect();
    let missing_case_ids: Vec<_> = expected_case_ids
        .difference(&actual_case_ids)
        .copied()
        .collect();
    let unexpected_case_ids: Vec<_> = actual_case_ids
        .difference(&expected_case_ids)
        .copied()
        .collect();
    let complete = missing_case_ids.is_empty() && unexpected_case_ids.is_empty();

    let digest_records = records
        .iter()
        .map(|record| -> CargoAllowResult<_> {
            Ok((
                record
                    .get("case_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                ParityComparison {
                    result: ParityComparisonResult::parse(
                        record
                            .get("result")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )?,
                    source_identity: record
                        .get("source_identity")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                },
                record
                    .get("old_output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                record
                    .get("new_output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;
    let digest = corpus_digest(&digest_records);
    let equivalent = records.iter().all(|record| {
        record.get("result").and_then(Value::as_str)
            == Some(ParityComparisonResult::SemanticallyEquivalent.as_str())
    });
    let passed = complete && equivalent;
    let payload = json!({
        "schema_id": "cargo-allow.extraction-parity-runtime.v1",
        "schema_version": 1,
        "tool": "cargo-allow extraction-parity",
        "result": if passed { "Passed" } else if !complete { "Incomplete" } else { "Failed" },
        "completeness": if complete { "Complete" } else { "Partial" },
        "source_identity": initial_source_identity,
        "stage": stage_name(args.stage),
        "parity_result_digest": digest,
        "records": records,
        "expected_case_count": expected_case_ids.len(),
        "emitted_case_count": actual_case_ids.len(),
        "missing_case_ids": missing_case_ids,
        "unexpected_case_ids": unexpected_case_ids,
        "claim_boundary": [
            "runtime_old_new_parity_only",
            "exact_git_commit_and_tree_identity",
            "no_cutover_receipt_claim",
            "no_policy_disposition_promotion",
            "no_reachability_or_package_ownership_claim",
        ],
    });
    let rendered = serde_json::to_string_pretty(&payload)
        .map_err(|error| CargoAllowError::new(format!("render parity evidence: {error}")))?
        + "\n";
    emit_text(args.output.as_deref(), &rendered)?;
    if passed {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "runtime parity evidence contains a non-equivalent case",
        ))
    }
}

fn load_registry(
    root: &Path,
) -> CargoAllowResult<allow_policy::extraction_parity::ExtractionParityRegistry> {
    let path = root.join("policy/extraction-parity.toml");
    let input = fs::read_to_string(&path).map_err(|error| {
        CargoAllowError::new(format!(
            "read extraction parity registry {}: {error}",
            path.display()
        ))
    })?;
    allow_policy::extraction_parity::parse_extraction_parity_registry_at(Some(&path), &input)
}

fn stage_name(stage: ParityStageArg) -> &'static str {
    match stage {
        ParityStageArg::All => "All",
        ParityStageArg::RepoSnapshot => "RepoSnapshot",
        ParityStageArg::RepoEdit => "RepoEdit",
    }
}

fn stage_matches(registered: &str, requested: ParityStageArg) -> bool {
    match requested {
        ParityStageArg::All => matches!(registered, "RepoSnapshot" | "RepoEdit"),
        ParityStageArg::RepoSnapshot => registered == "RepoSnapshot",
        ParityStageArg::RepoEdit => registered == "RepoEdit",
    }
}

fn append_snapshot_case(
    records: &mut Vec<Value>,
    label: &str,
    case: crate::extraction_parity_runtime::RepoSnapshotParityCase,
) {
    append_record(
        records,
        label.to_string(),
        case.comparison,
        case.old_output,
        case.new_output,
    );
}

fn append_record(
    records: &mut Vec<Value>,
    case_id: String,
    comparison: ParityComparison,
    old_output: String,
    new_output: String,
) {
    records.push(json!({
        "case_id": case_id,
        "result": comparison.result.as_str(),
        "source_identity": comparison.source_identity,
        "old_output": old_output,
        "new_output": new_output,
    }));
}

fn source_identity(root: &Path) -> CargoAllowResult<String> {
    let commit = git_value(root, &["rev-parse", "HEAD"])?;
    let tree = git_value(root, &["rev-parse", "HEAD^{tree}"])?;
    Ok(format!("commit:{commit}/tree:{tree}"))
}

fn git_value(root: &Path, args: &[&str]) -> CargoAllowResult<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| CargoAllowError::new(format!("run git {:?}: {error}", args)))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        return Err(CargoAllowError::new(format!(
            "git {:?} returned an empty value",
            args
        )));
    }
    Ok(value)
}
