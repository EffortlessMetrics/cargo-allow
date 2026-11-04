use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
fn test_release_workflow_structure() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let release_wf_path = root.join(".github/workflows/release.yml");
    if !release_wf_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(release_wf_path)?;

    // Must have workflow_dispatch trigger
    require(
        content.contains("workflow_dispatch:"),
        "release.yml must support workflow_dispatch",
    )?;

    // Must not contain hardcoded publish tokens in cleartext
    require(
        !content.contains("CARGO_REGISTRY_TOKEN: \\\""),
        "cleartext CARGO_REGISTRY_TOKEN is prohibited",
    )?;

    Ok(())
}

/// #3790: every final tag/token/upload path is downstream of exact typed
/// authorization validation. The authorize gate compiles the out-of-tree
/// document before any token access; token steps cannot run when the gate
/// is skipped, failed, or stale.
#[test]
fn test_release_workflow_authorize_gate_precedes_token_access() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let release_wf_path = root.join(".github/workflows/release.yml");
    if !release_wf_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(release_wf_path)?;

    let authorize = job_block(&content, "authorize")
        .ok_or_else(|| io::Error::other("release.yml must define the authorize gate job"))?;
    require(
        authorize.contains("needs: preflight") || authorize.contains("needs: [preflight"),
        "the authorize gate must run after preflight",
    )?;
    // The gate compiles the document with the production validator and
    // transitions one-use state; tag pushes and dispatches alone prove nothing.
    for required in [
        "release-authorization validate",
        "--expected-context",
        "--transition-to selected-for-run",
        "authorization-run-id",
    ] {
        require(
            authorize.contains(required),
            &format!("authorize gate must enforce {required}"),
        )?;
    }
    // Tag peel binding: the tag must resolve to the preflight commit/tree.
    require(
        authorize.contains("does not peel to the preflight"),
        "authorize gate must bind the tag peel to preflight identities",
    )?;

    let publish = job_block(&content, "publish")
        .ok_or_else(|| io::Error::other("release.yml must define the publish job"))?;
    require(
        publish.contains("authorize"),
        "the publish job must depend on the authorize gate",
    )?;
    // Token lookup requires a Complete gate (or the incident-owned recovery
    // path); rehearsal and failed gates stay zero-token.
    require(
        publish.contains("needs.authorize.outputs.valid == 'true'"),
        "token access must require a valid authorization gate",
    )?;
    // The compiled digest propagates to evidence.
    require(
        content.contains("authorization_digest"),
        "the authorization digest must propagate to publish evidence",
    )?;
    Ok(())
}

/// #3790 repair: the authorize gate reads the annotated tag object itself,
/// never the peeled commit message. Lightweight tags carry no authorization
/// references and are rejected before any bundle download.
#[test]
fn test_release_workflow_tag_object_authority() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let release_wf_path = root.join(".github/workflows/release.yml");
    if !release_wf_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(release_wf_path)?;
    let authorize = job_block(&content, "authorize")
        .ok_or_else(|| io::Error::other("release.yml must define the authorize gate job"))?;
    require(
        authorize.contains("^{tag}") && authorize.contains("cat-file"),
        "the authorize gate must read the annotated tag object",
    )?;
    require(
        authorize.contains("lightweight tags never authorize"),
        "the authorize gate must reject lightweight tags",
    )?;
    require(
        !authorize.contains("git log -1 --format=%B \"refs/tags/"),
        "the authorize gate must not read the peeled commit message as authority",
    )?;
    Ok(())
}

/// #3790 repair: a pushed tag name is evidence, never authority. Preflight
/// must not mint an authorization reference from the ref name; the publish
/// job selects the compiled gate digest (or the incident-owned recovery
/// reference) and refuses an empty authorization before any upload.
#[test]
fn test_release_workflow_publisher_authorization_binding() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let release_wf_path = root.join(".github/workflows/release.yml");
    if !release_wf_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(release_wf_path)?;
    require(
        !content.contains("authorization=\"${GITHUB_REF_NAME}\""),
        "preflight must not mint authorization from the pushed tag name",
    )?;
    let publish = job_block(&content, "publish")
        .ok_or_else(|| io::Error::other("release.yml must define the publish job"))?;
    require(
        publish.contains("GATE_AUTHORIZATION_DIGEST")
            && publish.contains("needs.authorize.outputs.authorization_digest"),
        "the publish job must select the compiled gate digest",
    )?;
    require(
        publish.contains("no compiled authorization for publication"),
        "the publish job must refuse an empty authorization before upload",
    )?;
    require(
        !publish.contains("--authorization \"${{ needs.preflight.outputs.authorization }}\""),
        "the publisher must not receive tag-name authority on the clean path",
    )?;
    Ok(())
}

/// Extract one top-level job block from a workflow document.
fn job_block(content: &str, job: &str) -> Option<String> {
    let header = format!("  {job}:");
    let lines: Vec<&str> = content.lines().collect();
    let mut collected: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in &lines {
        if !in_block {
            if line.trim_end() == header.trim_end() {
                in_block = true;
                collected.push(line);
            }
            continue;
        }
        if line.starts_with("  ") && !line.starts_with("    ") && line.ends_with(':') {
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

/// Drift guard for #3915 PR C: every platform row of the release-set lane
/// must run allow-rust's persistent scan-cache suites. A platform-wide
/// `--skip` exclusion (the pre-PR-C macOS workaround) must not silently
/// return, and the lane manifest must not re-describe the suite as bounded.
#[test]
fn test_ci_workflow_runs_the_full_cache_suite_on_every_platform() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let ci_path = root.join(".github/workflows/ci.yml");
    // ci.yml is a checked-in repository asset: its absence means this
    // contract cannot be evaluated, so the guard fails closed.
    require(ci_path.exists(), ".github/workflows/ci.yml must exist")?;
    let content = fs::read_to_string(&ci_path)?;

    for skipped in [
        "--skip persistent_scan_cache",
        "--skip scan_cache_store",
        "cache_scope: bounded",
    ] {
        require(
            !content.contains(skipped),
            &format!("ci.yml must not reintroduce the platform cache exclusion: {skipped}"),
        )?;
    }

    let block = test_core_platforms_job_block(&content)
        .ok_or_else(|| io::Error::other("test-core-platforms job missing from ci.yml"))?;
    require(
        block.contains("-p allow-inventory -p allow-files -p allow-rust"),
        "the test-core-platforms suite must include allow-rust (the \
         persistent scan-cache tests run on every platform row)",
    )?;

    let lanes_path = root.join("docs/ci-lanes.toml");
    require(lanes_path.exists(), "docs/ci-lanes.toml must exist")?;
    let lanes = fs::read_to_string(&lanes_path)?;
    require(
        !lanes.contains("cache exclusions remain bounded"),
        "ci-lanes.toml must not re-describe the cache suite as bounded",
    )?;

    Ok(())
}

/// PR C: the authorize gate emits one canonical operation identity artifact
/// and downstream jobs consume it with agreement checks instead of
/// reconstructing operation identity from their own checkout or inputs.
#[test]
fn test_release_workflow_canonical_operation_identity_transport() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let release_wf_path = root.join(".github/workflows/release.yml");
    if !release_wf_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(release_wf_path)?;

    let authorize = job_block(&content, "authorize")
        .ok_or_else(|| io::Error::other("release.yml must define the authorize gate job"))?;
    require(
        authorize.contains("release-operation-identity.json"),
        "the authorize gate must emit the canonical operation identity artifact",
    )?;
    require(
        authorize.contains("name: release-operation-identity"),
        "the authorize gate must upload the operation identity artifact",
    )?;

    let publish = job_block(&content, "publish")
        .ok_or_else(|| io::Error::other("release.yml must define the publish job"))?;
    for required in [
        "name: release-operation-identity",
        "canonical operation identity artifact is absent",
        "operation identity version differs",
        "operation identity commit differs",
        "operation identity tree differs",
        "operation identity authorization differs",
    ] {
        require(
            publish.contains(required),
            &format!("publish must verify canonical identity agreement: {required}"),
        )?;
    }

    let release = job_block(&content, "github-release")
        .ok_or_else(|| io::Error::other("release.yml must define the github-release job"))?;
    for required in [
        "publish receipt is absent",
        "publish receipt version differs",
        "publish receipt authorization differs",
    ] {
        require(
            release.contains(required),
            &format!("github-release must verify publish receipt agreement: {required}"),
        )?;
    }
    Ok(())
}

/// #2267: the publication path is gated on cross-platform source-candidate
/// smoke. The release workflow must run the landed Stage A + #2266 harness
/// per selected platform on the exact release identity, aggregate the rows
/// through the fail-closed gate script, depend publish on that aggregate,
/// and re-verify the aggregate binding inside publish before any upload
/// step. The selected matrix (#3737: Linux + Windows on Rust 1.95) and the
/// explicit macOS / Rust-1.85 NotIncluded classification are pinned here so
/// the claimed platform set cannot drift through YAML edits alone.
#[test]
fn test_release_workflow_gates_publication_on_source_candidate_smoke() -> Result<(), Box<dyn Error>>
{
    /// #2497: the release manifest's semantic authority is the typed Rust
    /// command. The shell script is gone from the authoritative path, generation
    /// requires the typed Complete result, the exact bytes are re-validated
    /// immediately before attestation, and attestation plus every release step
    /// are gated on the typed result output.
    #[test]
    fn test_release_workflow_manifest_is_typed_and_complete_gated() -> Result<(), Box<dyn Error>> {
        let root = repo_root()?;
        let release_wf_path = root.join(".github/workflows/release.yml");
        if !release_wf_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(release_wf_path)?;

        let smoke = job_block(&content, "source-candidate-smoke").ok_or_else(|| {
            io::Error::other("release.yml must define the source-candidate-smoke matrix job")
        })?;
        let aggregate =
            job_block(&content, "source-candidate-smoke-aggregate").ok_or_else(|| {
                io::Error::other(
                    "release.yml must define the source-candidate-smoke-aggregate gate job",
                )
            })?;
        let publish = job_block(&content, "publish")
            .ok_or_else(|| io::Error::other("release.yml must define the publish job"))?;

        // Every required platform row is separately encoded on Rust 1.95 and
        // one row's failure must not cancel its sibling.
        for required in [
            "platform: linux",
            "platform: windows",
            "toolchain: \"1.95.0\"",
            "fail-fast: false",
        ] {
            require(
                smoke.contains(required),
                &format!("the source-candidate-smoke matrix must encode {required}"),
            )?;
        }
        // The rows reuse the landed harnesses; no forked smoke implementation.
        for required in [
            "scripts/package-candidate-smoke.sh",
            "scripts/source-candidate-smoke.sh",
        ] {
            require(
                smoke.contains(required),
                &format!("smoke rows must reuse the landed harness {required}"),
            )?;
        }
        // The gate law lives in one fail-closed script, never in scattered YAML.
        for required in [
            "release-source-candidate-smoke-gate.py aggregate",
            "release-source-candidate-smoke-gate.py verify",
        ] {
            require(
                content.contains(required),
                &format!("release.yml must invoke the gate script: {required}"),
            )?;
        }
        // No aggregation shortcut may report green after a required failure.
        for banned in ["continue-on-error", "|| true"] {
            require(
                !smoke.contains(banned) && !aggregate.contains(banned),
                &format!("the smoke gate must not use {banned}"),
            )?;
        }
        // The job itself runs under !cancelled() — never always() — so a failed
        // or skipped matrix yields a red gate, while artifact retention may
        // still run under always() (#2267 aggregate law).
        require(
            aggregate.contains("if: ${{ !cancelled() }}"),
            "the aggregate job must run under !cancelled() so a failed or \
         skipped matrix fails the gate instead of silently skipping it",
        )?;
        require(
            aggregate.contains("needs: [preflight, source-candidate-smoke]")
                && aggregate.contains("needs.source-candidate-smoke.result"),
            "the aggregate must consume the raw matrix needs result and refuse \
         missing, failed, cancelled, timed-out, or skipped rows",
        )?;
        // Publication depends on the aggregate and re-verifies its binding.
        require(
            publish.contains("needs: [preflight, authorize, source-candidate-smoke-aggregate]"),
            "publish must need the source-candidate smoke aggregate",
        )?;
        for required in [
            "Download the source-candidate smoke aggregate",
            "source-candidate-smoke-aggregate.receipt.json",
            "source-candidate-smoke-verify.receipt.json",
        ] {
            require(
                publish.contains(required),
                &format!("publish must re-verify the smoke aggregate: {required}"),
            )?;
        }
        // The explicit NotIncluded classification is declared, not inferred.
        require(
            content.contains("NotIncluded"),
            "release.yml must record the explicit NotIncluded platform \
         classification (#3737) instead of inferring it from absent rows",
        )?;
        Ok(())
    }

    let release = job_block(&content, "github-release")
        .ok_or_else(|| io::Error::other("release.yml must define the github-release job"))?;

    require(
        release.contains("cargo run -p cargo-allow --locked --quiet -- release-manifest"),
        "the manifest must be produced by the typed release-manifest command",
    )?;
    require(
        release.contains("--require-result complete"),
        "manifest generation must require the typed complete result",
    )?;
    require(
        !content.contains("generate-release-manifest.sh"),
        "release.yml must not route manifest authority through a shell script",
    )?;

    require(
        release.contains("Require the typed Complete manifest before attestation"),
        "a pre-attestation typed Complete gate must exist",
    )?;
    require(
        release.contains("--manifest target/cargo-allow/release-manifest-v2.json"),
        "the pre-attestation gate must re-validate the exact manifest bytes",
    )?;

    let gate = "steps.release_manifest.outputs.result == 'complete'";
    for step in [
        "Attest release manifest",
        "Create GitHub Release (draft until manifest attached)",
        "Attach release manifest to GitHub Release",
        "Publish release after manifest attached",
    ] {
        let block = step_block(&release, step)
            .ok_or_else(|| io::Error::other(format!("release.yml must define the {step} step")))?;
        require(
            block.contains(&format!("if: success() && {gate}")),
            &format!("the {step} step must be gated on the typed Complete result"),
        )?;
    }
    Ok(())
}

/// Extract one step's block (its `- name:` line through the line before the
/// next sibling `- ` entry) without slicing, so the helper itself stays
/// free of panic findings.
fn step_block(job: &str, name: &str) -> Option<String> {
    let header = format!("      - name: {name}");
    let mut collected: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in job.lines() {
        if !in_block {
            if line.trim_end() == header {
                in_block = true;
                collected.push(line);
            }
            continue;
        }
        if line.starts_with("      - ") {
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

/// Extract one job's block (its header line through the last line before the
/// next same-indent job key) so assertions bind to that job only.
fn test_core_platforms_job_block(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut collected: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in &lines {
        if !in_block {
            if *line == "  test-core-platforms:" {
                in_block = true;
                collected.push(line);
            }
            continue;
        }
        // A same-indent bare key opens the next job; nested keys are
        // indented deeper and stay in this block.
        if line.starts_with("  ") && !line.starts_with("    ") && line.ends_with(':') {
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
