//! #3922 workflow wire-up contract: the release workflows write remotely
//! durable publication checkpoints at the law's declared points, evidence
//! only. The payload follows the landed `CargoAllowPublicationCheckpointV1`
//! contract; the provider is the selected GitHub Actions artifact surface;
//! every stored payload is fetched back and verified by exact-byte
//! independent readback before the upload it gates may begin, and the
//! post-observation readback blocks dependants. These guards fail closed on
//! drift: if the checkpoint steps disappear, move across the publication
//! step, or start carrying publication authority (tokens, tags, releases),
//! the lane contract is broken.

use std::{error::Error, fs, io, path::Path, path::PathBuf};

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if cond {
        Ok(())
    } else {
        Err(io::Error::other(msg.to_string()))
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

fn workflow(root: &Path, name: &str) -> Result<String, Box<dyn Error>> {
    let path = root.join(".github/workflows").join(name);
    require(path.exists(), &format!("{name} must exist"))?;
    Ok(fs::read_to_string(path)?)
}

/// One step's block: its `- name:` line through the line before the next
/// sibling step marker.
fn step_block(job: &str, name: &str) -> Option<String> {
    let marker = format!("      - name: {name}");
    let lines: Vec<&str> = job.lines().collect();
    let mut collected: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in &lines {
        if !in_block {
            if line.trim_end() == marker.trim_end() {
                in_block = true;
                collected.push(line);
            }
            continue;
        }
        if line.trim_start().starts_with("- name: ") && line.starts_with("      - name: ") {
            break;
        }
        collected.push(line);
    }
    if collected.is_empty() {
        None
    } else {
        Some(collected.join("\n"))
    }
}

fn position(job: &str, needle: &str) -> Option<usize> {
    job.find(needle)
}

/// The evidence-only boundary: checkpoint steps never touch publication
/// authority, credentials, tags, or releases.
fn require_evidence_only(step: &str, label: &str) -> Result<(), Box<dyn Error>> {
    for forbidden in [
        "CARGO_REGISTRY_TOKEN",
        "secrets.",
        "gh release",
        "git push",
        "git tag ",
        "cargo publish",
    ] {
        require(
            !step.contains(forbidden),
            &format!("{label} must stay evidence-only: found {forbidden}"),
        )?;
    }
    Ok(())
}

const PRE_INTENT_STEPS: [&str; 4] = [
    "Write the pre-intent publication checkpoint",
    "Store the pre-intent publication checkpoint",
    "Fetch the stored pre-intent checkpoint for independent readback",
    "Verify the independent pre-intent checkpoint readback",
];

const POST_OBSERVATION_STEPS: [&str; 4] = [
    "Write the post-observation publication checkpoint",
    "Store the post-observation publication checkpoint",
    "Fetch the stored post-observation checkpoint for independent readback",
    "Verify the independent post-observation checkpoint readback",
];

/// Payload-contract markers every write step must carry: the schema id, the
/// canonical stored readback posture, converged provider size, and the
/// fail-closed readback law.
fn require_write_step_contract(step: &str, label: &str) -> Result<(), Box<dyn Error>> {
    for required in [
        "cargo-allow.publication-checkpoint.v1",
        "cargo-allow.publication-journal.v1",
        "\"readback\": \"missing\"",
        "prior_checkpoint_digest",
        "checkpoint_sequence",
        "claim_boundary",
        "checkpoint payload size did not converge",
        "sha256_bytes",
    ] {
        require(
            step.contains(required),
            &format!("{label} must follow the payload contract: missing {required}"),
        )?;
    }
    require_evidence_only(step, label)
}

/// Verify-step markers: exact-byte independent readback with a non-clean
/// fail-closed classification.
fn require_verify_step_contract(
    step: &str,
    label: &str,
    blocked: &str,
) -> Result<(), Box<dyn Error>> {
    for required in [
        "expected_payload_sha256",
        "observed_payload_sha256",
        "\"mismatch\"",
        "\"missing\"",
        blocked,
    ] {
        require(
            step.contains(required),
            &format!("{label} must fail closed on non-clean readback: missing {required}"),
        )?;
    }
    require_evidence_only(step, label)
}

/// The pre-intent checkpoints are written, stored, fetched, and verified
/// before the package upload step begins; the post-observation checkpoints
/// are written only after the publication step settles; the evidence
/// retention step follows the receipt steps.
#[test]
fn test_release_workflow_writes_checkpoints_at_the_law_points() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let content = workflow(&root, "release.yml")?;

    let publish = position(&content, "  publish:")
        .ok_or_else(|| io::Error::other("release.yml must define the publish job"))?;
    let next_job = content[publish..]
        .find("\n  install-smoke:")
        .map(|index| publish + index)
        .unwrap_or(content.len());
    let job = content[publish..next_job].to_string();

    let publish_step = "Publish cargo-allow topology rows in dependency order";
    let publish_at = position(&job, publish_step)
        .ok_or_else(|| io::Error::other("publish job must publish topology rows"))?;

    for name in PRE_INTENT_STEPS {
        let block = step_block(&job, name)
            .ok_or_else(|| io::Error::other(format!("publish job must define {name}")))?;
        let at = position(&job, name)
            .ok_or_else(|| io::Error::other(format!("publish job must define {name}")))?;
        require(
            at < publish_at,
            &format!("{name} must run before the package upload begins"),
        )?;
        if name.starts_with("Write") {
            require_write_step_contract(&block, name)?;
        }
        if name.starts_with("Verify") {
            require_verify_step_contract(&block, name, "publication progress stays blocked")?;
        }
    }

    for name in POST_OBSERVATION_STEPS {
        let block = step_block(&job, name)
            .ok_or_else(|| io::Error::other(format!("publish job must define {name}")))?;
        let at = position(&job, name)
            .ok_or_else(|| io::Error::other(format!("publish job must define {name}")))?;
        require(
            at > publish_at,
            &format!("{name} must run after the publication step settles"),
        )?;
        if name.starts_with("Write") {
            require_write_step_contract(&block, name)?;
        }
        if name.starts_with("Verify") {
            require_verify_step_contract(&block, name, "dependant progress stays blocked")?;
        }
    }

    let retain_at = position(&job, "Retain publication checkpoint evidence")
        .ok_or_else(|| io::Error::other("publish job must retain checkpoint evidence"))?;
    let receipt_at = position(&job, "Record publish receipt")
        .ok_or_else(|| io::Error::other("publish job must record the publish receipt"))?;
    require(
        retain_at > receipt_at,
        "checkpoint evidence retention must follow the receipt steps",
    )?;

    // The post-observation payload links to the exact stored pre-intent
    // bytes and reads the settled publication receipt.
    let post_write = step_block(&job, "Write the post-observation publication checkpoint")
        .ok_or_else(|| io::Error::other("post-observation write step must exist"))?;
    for required in [
        "publication-checkpoint-pre-intent.json",
        "topology-publish.receipt.json",
        "first_irreversible_row",
        "visible_exact",
    ] {
        require(
            post_write.contains(required),
            &format!("post-observation write step must bind {required}"),
        )?;
    }

    // The pre-intent payload binds the canonical operation identity and the
    // settled pre-upload evidence, with class-driven operation ids.
    let pre_write = step_block(&job, "Write the pre-intent publication checkpoint")
        .ok_or_else(|| io::Error::other("pre-intent write step must exist"))?;
    for required in [
        "publish_cargo_allow_final_0_2_0",
        "recover_cargo_allow_final_publication",
        "release-operation-identity.json",
        "shared-registry-preflight.receipt.json",
        "product-package-topology-v2.toml",
        "\"state\": \"intent_durable\"",
    ] {
        require(
            pre_write.contains(required),
            &format!("pre-intent write step must bind {required}"),
        )?;
    }

    Ok(())
}

/// The authorized namespace workflow carries the same law around its
/// namespace publication step, with the namespace operation identity and
/// the namespace evidence bindings.
#[test]
fn test_release_authorized_workflow_writes_checkpoints_at_the_law_points()
-> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let content = workflow(&root, "release-authorized.yml")?;

    let job_at = position(&content, "  publish-namespace:")
        .ok_or_else(|| io::Error::other("release-authorized.yml must define publish-namespace"))?;
    let job = content[job_at..].to_string();

    let publish_step = "Publish and verify shared, intent, and proof 0.1.0 rows";
    let publish_at = position(&job, publish_step)
        .ok_or_else(|| io::Error::other("namespace job must publish the 0.1.0 rows"))?;

    for name in PRE_INTENT_STEPS {
        let block = step_block(&job, name)
            .ok_or_else(|| io::Error::other(format!("namespace job must define {name}")))?;
        let at = position(&job, name)
            .ok_or_else(|| io::Error::other(format!("namespace job must define {name}")))?;
        require(
            at < publish_at,
            &format!("{name} must run before the package upload begins"),
        )?;
        if name.starts_with("Write") {
            require_write_step_contract(&block, name)?;
        }
        if name.starts_with("Verify") {
            require_verify_step_contract(&block, name, "publication progress stays blocked")?;
        }
    }

    for name in POST_OBSERVATION_STEPS {
        let block = step_block(&job, name)
            .ok_or_else(|| io::Error::other(format!("namespace job must define {name}")))?;
        let at = position(&job, name)
            .ok_or_else(|| io::Error::other(format!("namespace job must define {name}")))?;
        require(
            at > publish_at,
            &format!("{name} must run after the publication step settles"),
        )?;
        if name.starts_with("Write") {
            require_write_step_contract(&block, name)?;
        }
        if name.starts_with("Verify") {
            require_verify_step_contract(&block, name, "dependant progress stays blocked")?;
        }
    }

    let retain_at = position(&job, "Retain publication checkpoint evidence")
        .ok_or_else(|| io::Error::other("namespace job must retain checkpoint evidence"))?;
    let receipt_at = position(&job, "Upload namespace publication receipt")
        .ok_or_else(|| io::Error::other("namespace job must upload its receipt"))?;
    require(
        retain_at > receipt_at,
        "checkpoint evidence retention must follow the receipt steps",
    )?;

    // Namespace bindings: the verified authorization document, the settled
    // no-new receipt, and the namespace operation identity.
    let pre_write = step_block(&job, "Write the pre-intent publication checkpoint")
        .ok_or_else(|| io::Error::other("namespace pre-intent write step must exist"))?;
    for required in [
        "publish_shared_namespace_for_v0_2_0",
        "release/authorize-v0.2.0.json",
        "target/cargo-allow/check.receipt.json",
        "product-package-topology-v2.toml",
        "\"state\": \"intent_durable\"",
    ] {
        require(
            pre_write.contains(required),
            &format!("namespace pre-intent write step must bind {required}"),
        )?;
    }
    let post_write = step_block(&job, "Write the post-observation publication checkpoint")
        .ok_or_else(|| io::Error::other("namespace post-observation write step must exist"))?;
    require(
        post_write.contains("namespace-publish.receipt.json"),
        "namespace post-observation write step must bind the namespace receipt",
    )?;

    Ok(())
}

/// The durable provider is the selected GitHub Actions artifact surface:
/// store, fetch, and retention follow the landed substrate decision, and the
/// readback receipts retain the provider-assigned artifact identity.
#[test]
fn test_checkpoint_steps_use_the_selected_durable_provider() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    for name in ["release.yml", "release-authorized.yml"] {
        let content = workflow(&root, name)?;
        for marker in [
            "github_actions_artifact",
            "publication-checkpoint-pre-intent",
            "publication-checkpoint-post-observation",
            "publication-checkpoint-readbacks",
            "retention-days: 90",
            "if-no-files-found: error",
            "artifact_id",
        ] {
            require(
                content.contains(marker),
                &format!("{name} must carry the durable provider marker {marker}"),
            )?;
        }
    }
    Ok(())
}

/// The existing authorization and publication gates stay byte-identical:
/// the exact gate markers from the #3790 law must still be present, and the
/// checkpoint additions must not have replaced or reworded them.
#[test]
fn test_release_workflow_gates_remain_untouched() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let content = workflow(&root, "release.yml")?;
    for gate in [
        "Require crates.io API token for publication",
        "no compiled authorization for publication",
        "needs.authorize.outputs.valid == 'true'",
        "--transition-to selected-for-run",
    ] {
        require(
            content.contains(gate),
            &format!("release.yml gate marker must remain: {gate}"),
        )?;
    }
    let authorized = workflow(&root, "release-authorized.yml")?;
    for gate in [
        "Require token-backed crates.io authentication",
        "Reject an existing v0.2.0 tag before first publication",
        "Verify exact release authorization",
    ] {
        require(
            authorized.contains(gate),
            &format!("release-authorized.yml gate marker must remain: {gate}"),
        )?;
    }
    Ok(())
}
