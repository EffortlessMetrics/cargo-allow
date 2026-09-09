//! Read-only policy and proof enrichment over one exact dependency
//! graph delta (#3920 PR C).
//!
//! Advisory exit contract: a completed enrichment — whatever its
//! disposition — exits zero, so the signal producer reports without
//! blocking. Only malformed or stale input (identity mismatch, free
//! text where a typed reference belongs, bound overflow) exits
//! nonzero as an instrument failure. Blocking-consequence selection
//! stays with the owning policy (PR D's routed surface).

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    DependencyEvidenceBundleV1, DependencyGraphDeltaReceiptV1, DependencyGraphEvidenceFormat,
    attach_dependency_graph_evidence, render_dependency_graph_evidence,
};
use clap::{Parser, Subcommand};

/// Read-only dependency graph evidence adjacency (hidden automation
/// tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct DependencyGraphEvidenceArgs {
    #[command(subcommand)]
    pub(crate) command: DependencyGraphEvidenceSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum DependencyGraphEvidenceSubcommand {
    /// Attach evidence-authority records to one compiled delta
    /// receipt by exact package identity.
    #[command(hide = true)]
    Evaluate(DependencyGraphEvidenceEvaluateArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct DependencyGraphEvidenceEvaluateArgs {
    /// Delta receipt JSON emitted by the #3920 PR B compiler lane.
    #[arg(long)]
    pub(crate) delta: PathBuf,
    /// Evidence bundle JSON with typed references from the linked
    /// authorities (#3903, #2038, #1897, #2924, #3359).
    #[arg(long)]
    pub(crate) bundle: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: DependencyGraphEvidenceOutputFormat,
    /// Write the enriched receipt to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum DependencyGraphEvidenceOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_dependency_graph_evidence(
    args: &DependencyGraphEvidenceArgs,
) -> CargoAllowResult<()> {
    let DependencyGraphEvidenceSubcommand::Evaluate(evaluate) = &args.command;
    let delta_bytes = std::fs::read(&evaluate.delta).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("delta receipt read {}: {error}", evaluate.delta.display()),
        )
    })?;
    let delta: DependencyGraphDeltaReceiptV1 =
        serde_json::from_slice(&delta_bytes).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("delta receipt parses: {error}"),
            )
        })?;
    let bundle_bytes = std::fs::read(&evaluate.bundle).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "evidence bundle read {}: {error}",
                evaluate.bundle.display()
            ),
        )
    })?;
    let bundle: DependencyEvidenceBundleV1 =
        serde_json::from_slice(&bundle_bytes).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("evidence bundle parses: {error}"),
            )
        })?;

    let receipt = attach_dependency_graph_evidence(&delta, &bundle).map_err(|reason| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            format!("evidence enrichment fails closed: {reason}"),
        )
    })?;
    let format = match evaluate.format {
        DependencyGraphEvidenceOutputFormat::Json => DependencyGraphEvidenceFormat::Json,
        DependencyGraphEvidenceOutputFormat::Human => DependencyGraphEvidenceFormat::Human,
    };
    let rendered = render_dependency_graph_evidence(&receipt, format)
        .map_err(|reason| CargoAllowError::with_kind(CargoAllowErrorKind::Artifact, reason))?;
    if let Some(output) = &evaluate.output {
        std::fs::write(output, rendered).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("receipt write {}: {error}", output.display()),
            )
        })?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use allow_report::{
        DependencyEvidenceAuthorityV1, DependencyEvidenceBundleV1, DependencyEvidenceRecordV1,
        DependencyGraphDeltaIdentityV1, DependencyGraphDeltaKindV1, DependencyGraphDeltaReceiptV1,
        DependencyGraphDeltaRowV1,
    };
    use std::path::PathBuf;

    use super::{
        DependencyGraphEvidenceArgs, DependencyGraphEvidenceEvaluateArgs,
        DependencyGraphEvidenceOutputFormat, DependencyGraphEvidenceSubcommand,
        cmd_dependency_graph_evidence,
    };

    fn workspace_root() -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
            .join("../..")
            .canonicalize()
            .expect("workspace root resolves")
    }

    fn unique_temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dep-graph-evidence-{name}-{}-{}.json",
            std::process::id(),
            uuid_like_counter()
        ))
    }

    fn uuid_like_counter() -> usize {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    fn delta_receipt_json() -> String {
        let identity = DependencyGraphDeltaIdentityV1 {
            base_commit: "aaa111".to_string(),
            head_commit: "bbb222".to_string(),
            base_manifest_set_digest: "sha256:v1:base".to_string(),
            head_manifest_set_digest: "sha256:v1:head".to_string(),
            base_lock_digest: "sha256:v1:base-lock".to_string(),
            head_lock_digest: "sha256:v1:head-lock".to_string(),
            product: "cargo-allow".to_string(),
            target: "x86_64-unknown-linux-gnu".to_string(),
        };
        let row = DependencyGraphDeltaRowV1 {
            kind: DependencyGraphDeltaKindV1::PackageUpgraded,
            class: allow_report::DependencyClassV1::Normal,
            package_name: "serde".to_string(),
            base_version: "1.0.200".to_string(),
            head_version: "1.0.228".to_string(),
            base_requirement: String::new(),
            head_requirement: String::new(),
            base_source: String::new(),
            head_source: String::new(),
            base_checksum: String::new(),
            head_checksum: String::new(),
        };
        let receipt = DependencyGraphDeltaReceiptV1 {
            schema_id: allow_report::DEPENDENCY_GRAPH_DELTA_SCHEMA_ID.to_string(),
            schema_version: allow_report::DEPENDENCY_GRAPH_DELTA_SCHEMA_VERSION,
            identity,
            rows: vec![row],
            complete: true,
            limitations: vec![],
            claim_boundary: "bounded".to_string(),
        };
        serde_json::to_string_pretty(&receipt).expect("delta receipt serializes")
    }

    fn bundle_json(reference: &str) -> String {
        let bundle = DependencyEvidenceBundleV1 {
            authorities_in_scope: vec![
                DependencyEvidenceAuthorityV1::MinimumVersion,
                DependencyEvidenceAuthorityV1::CargoDeny,
            ],
            base_commit: "aaa111".to_string(),
            head_commit: "bbb222".to_string(),
            product: "cargo-allow".to_string(),
            target: "x86_64-unknown-linux-gnu".to_string(),
            records: vec![DependencyEvidenceRecordV1 {
                authority: DependencyEvidenceAuthorityV1::CargoDeny,
                package_name: "serde".to_string(),
                version: "1.0.228".to_string(),
                reference: reference.to_string(),
                finding: false,
                advisory: false,
            }],
        };
        serde_json::to_string_pretty(&bundle).expect("bundle serializes")
    }

    #[test]
    fn evaluate_renders_the_enriched_receipt_to_the_output_file() {
        let delta_path = unique_temp("delta");
        let bundle_path = unique_temp("bundle");
        let output_path = unique_temp("out");
        std::fs::write(&delta_path, delta_receipt_json()).expect("delta fixture writes");
        std::fs::write(&bundle_path, bundle_json("deny:cargo-deny-run-42"))
            .expect("bundle fixture writes");
        let args = DependencyGraphEvidenceArgs {
            command: DependencyGraphEvidenceSubcommand::Evaluate(
                DependencyGraphEvidenceEvaluateArgs {
                    delta: delta_path.clone(),
                    bundle: bundle_path.clone(),
                    format: DependencyGraphEvidenceOutputFormat::Json,
                    output: Some(output_path.clone()),
                },
            ),
        };
        let outcome = cmd_dependency_graph_evidence(&args);
        let _ = std::fs::remove_file(&delta_path);
        let _ = std::fs::remove_file(&bundle_path);
        assert!(outcome.is_ok(), "the happy path exits zero: {outcome:?}");
        let written = std::fs::read_to_string(&output_path).expect("receipt written");
        let _ = std::fs::remove_file(&output_path);
        assert!(
            written.contains("dependency-graph-evidence"),
            "the receipt carries its schema identity"
        );
        assert!(written.contains("decision_required"));
    }

    #[test]
    fn evaluate_fails_closed_on_identity_mismatch() {
        let delta_path = unique_temp("delta-stale");
        let bundle_path = unique_temp("bundle-stale");
        std::fs::write(&delta_path, delta_receipt_json()).expect("delta fixture writes");
        let mut bundle = bundle_json("deny:cargo-deny-run-42");
        // Point the bundle at a different head commit: stale evidence.
        bundle = bundle.replace("\"bbb222\"", "\"ccc333\"");
        std::fs::write(&bundle_path, bundle).expect("bundle fixture writes");
        let args = DependencyGraphEvidenceArgs {
            command: DependencyGraphEvidenceSubcommand::Evaluate(
                DependencyGraphEvidenceEvaluateArgs {
                    delta: delta_path.clone(),
                    bundle: bundle_path.clone(),
                    format: DependencyGraphEvidenceOutputFormat::Json,
                    output: None,
                },
            ),
        };
        let outcome = cmd_dependency_graph_evidence(&args);
        let _ = std::fs::remove_file(&delta_path);
        let _ = std::fs::remove_file(&bundle_path);
        assert!(outcome.is_err(), "a stale bundle fails closed");
    }

    #[test]
    fn evaluate_fails_closed_on_free_text_references() {
        let delta_path = unique_temp("delta-comment");
        let bundle_path = unique_temp("bundle-comment");
        std::fs::write(&delta_path, delta_receipt_json()).expect("delta fixture writes");
        std::fs::write(
            &bundle_path,
            bundle_json("looks fine to me, no concerns raised"),
        )
        .expect("bundle fixture writes");
        let args = DependencyGraphEvidenceArgs {
            command: DependencyGraphEvidenceSubcommand::Evaluate(
                DependencyGraphEvidenceEvaluateArgs {
                    delta: delta_path.clone(),
                    bundle: bundle_path.clone(),
                    format: DependencyGraphEvidenceOutputFormat::Json,
                    output: None,
                },
            ),
        };
        let outcome = cmd_dependency_graph_evidence(&args);
        let _ = std::fs::remove_file(&delta_path);
        let _ = std::fs::remove_file(&bundle_path);
        assert!(
            outcome.is_err(),
            "commentary where a typed reference belongs fails closed"
        );
    }

    #[test]
    fn evaluate_fails_closed_on_a_malformed_delta() {
        let root = workspace_root();
        let bad = unique_temp("delta-malformed");
        std::fs::write(&bad, "{ not json }").expect("fixture writes");
        let args = DependencyGraphEvidenceArgs {
            command: DependencyGraphEvidenceSubcommand::Evaluate(
                DependencyGraphEvidenceEvaluateArgs {
                    delta: bad.clone(),
                    bundle: root.join("policy/allow.toml"),
                    format: DependencyGraphEvidenceOutputFormat::Human,
                    output: None,
                },
            ),
        };
        let outcome = cmd_dependency_graph_evidence(&args);
        let _ = std::fs::remove_file(&bad);
        assert!(outcome.is_err(), "a malformed delta receipt fails closed");
    }
}
