use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZeroMutationProof {
    pub tag_mutation_prevented: bool,
    pub token_read_prevented: bool,
    pub cargo_publish_prevented: bool,
    pub registry_mutation_prevented: bool,
    pub github_release_mutation_prevented: bool,
    pub live_setting_mutation_prevented: bool,
    pub external_repository_mutation_prevented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReleaseRehearsalReceiptV1 {
    pub schema_version: String,
    pub receipt_id: String,
    pub commit_sha: String,
    pub subject_lockfile_digest: String,
    pub subject_topology_digest: String,
    pub zero_mutation_proof: ZeroMutationProof,
    pub phases: std::collections::BTreeMap<String, String>,
    pub aggregate_status: String,
    pub claim_boundary: String,
}

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("no crates dir parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("no repo root"))?;
    Ok(root.to_path_buf())
}

#[test]
fn rehearsal_harness_generates_valid_receipt() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let script = root.join("scripts/release-rehearsal.py");
    if !script.exists() {
        return Ok(());
    }

    let output = Command::new("python")
        .arg(&script)
        .arg("--commit")
        .arg("0123456789abcdef0123456789abcdef01234567")
        .output()?;

    require(
        output.status.success(),
        &format!(
            "harness execution failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;

    let receipt: ReleaseRehearsalReceiptV1 = serde_json::from_slice(&output.stdout)?;
    require(
        receipt.schema_version == "1.0",
        "schema version must be 1.0",
    )?;
    require(
        receipt.aggregate_status == "Complete",
        "aggregate status must be Complete",
    )?;

    let proof = &receipt.zero_mutation_proof;
    require(
        proof.tag_mutation_prevented,
        "tag mutation must be prevented",
    )?;
    require(proof.token_read_prevented, "token read must be prevented")?;
    require(
        proof.cargo_publish_prevented,
        "cargo publish must be prevented",
    )?;
    require(
        proof.registry_mutation_prevented,
        "registry mutation must be prevented",
    )?;
    require(
        proof.github_release_mutation_prevented,
        "github release mutation must be prevented",
    )?;
    require(
        proof.live_setting_mutation_prevented,
        "live setting mutation must be prevented",
    )?;
    require(
        proof.external_repository_mutation_prevented,
        "external repository mutation must be prevented",
    )?;

    for required_phase in [
        "release_identity",
        "candidate_package_set",
        "shared_prerequisites",
        "publisher_state_machine",
        "docs_and_support_identity",
        "manifest_and_assets",
        "authorization_boundary",
        "workflow_graph_permissions",
    ] {
        require(
            receipt.phases.get(required_phase).map(|s| s.as_str()) == Some("Complete"),
            &format!("phase {required_phase} must be Complete"),
        )?;
    }

    Ok(())
}
