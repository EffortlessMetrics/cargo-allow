use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::extraction_parity::{ParityComparison, ParityComparisonResult, corpus_digest};
use clap::{Args, ValueEnum};
use serde_json::{Value, json};
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
    let source_identity = source_identity(&root)?;
    let mut records = Vec::new();

    if matches!(
        args.stage,
        ParityStageArg::All | ParityStageArg::RepoSnapshot
    ) {
        let run = crate::extraction_parity_runtime::run_repo_snapshot_parity(&root)?;
        append_snapshot_case(&mut records, "repo-snapshot-committed", run.committed);
        append_snapshot_case(&mut records, "repo-snapshot-staged", run.staged);
    }
    if matches!(args.stage, ParityStageArg::All | ParityStageArg::RepoEdit) {
        let run = crate::extraction_repo_edit_runtime::run_repo_edit_parity(&root)?;
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

    let digest_records = records
        .iter()
        .map(|record| -> CargoAllowResult<_> {
            Ok((
                record["case_id"].as_str().unwrap_or_default().to_string(),
                ParityComparison {
                    result: ParityComparisonResult::parse(
                        record["result"].as_str().unwrap_or_default(),
                    )?,
                    source_identity: record["source_identity"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                },
                record["old_output"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                record["new_output"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;
    let digest = corpus_digest(&digest_records);
    let passed = records.iter().all(|record| {
        record["result"].as_str() == Some(ParityComparisonResult::SemanticallyEquivalent.as_str())
    });
    let payload = json!({
        "schema_id": "cargo-allow.extraction-parity-runtime.v1",
        "schema_version": 1,
        "tool": "cargo-allow extraction-parity",
        "result": if passed { "Passed" } else { "Failed" },
        "source_identity": source_identity,
        "stage": match args.stage {
            ParityStageArg::All => "All",
            ParityStageArg::RepoSnapshot => "RepoSnapshot",
            ParityStageArg::RepoEdit => "RepoEdit",
        },
        "parity_result_digest": digest,
        "records": records,
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
