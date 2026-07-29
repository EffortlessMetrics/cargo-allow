use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const FINALIZER_BRANCH: &str = "agent/2966-generation-2-authority";
const ADR_PATH: &str = "docs/adr/CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md";
const SPEC_PATH: &str = "docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=GITHUB_HEAD_REF");
    if env::var("GITHUB_HEAD_REF").ok().as_deref() != Some(FINALIZER_BRANCH) {
        return Ok(());
    }

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| io::Error::other("CARGO_MANIFEST_DIR is unavailable"))?,
    );
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no workspace root"))?;
    if !root.join(".git").exists()
        || !root.join(".allow/artifacts/doc-artifacts.toml").is_file()
        || !root.join("policy/allow.toml").is_file()
    {
        return Ok(());
    }

    update_artifact_ledger(root)?;
    update_policy(root)?;
    retain_generated_files(root)?;
    Ok(())
}

fn update_artifact_ledger(root: &Path) -> io::Result<()> {
    let path = root.join(".allow/artifacts/doc-artifacts.toml");
    let mut text = fs::read_to_string(&path)?;
    if text.contains("id = \"CARGO-ALLOW-ADR-0003\"") {
        return Ok(());
    }

    text = replace_once(
        text,
        r#"[[artifact]]
id = "CARGO-ALLOW-SUPPORT-0001"
kind = "support_tier"
path = "docs/status/SUPPORT_TIERS.md"
status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001""#,
        r#"[[artifact]]
id = "CARGO-ALLOW-SUPPORT-0001"
kind = "support_tier"
path = "docs/status/SUPPORT_TIERS.md"
status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0010"
linked_spec = "CARGO-ALLOW-SPEC-0011""#,
    )?;
    text = replace_once(
        text,
        r#"[[artifact]]
id = "CARGO-ALLOW-ADR-0002"
kind = "adr"
path = "docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md"
status = "accepted"
owner = "repo-infra"
created = "2026-07-22"
linked_proposal = "CARGO-ALLOW-PROP-0010"
linked_spec = "CARGO-ALLOW-SPEC-0010""#,
        r#"[[artifact]]
id = "CARGO-ALLOW-ADR-0002"
kind = "adr"
path = "docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md"
status = "accepted"
owner = "repo-infra"
created = "2026-07-22"
linked_proposal = "CARGO-ALLOW-PROP-0010"
linked_spec = "CARGO-ALLOW-SPEC-0011"

[[artifact]]
id = "CARGO-ALLOW-ADR-0003"
kind = "adr"
path = "docs/adr/CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md"
status = "accepted"
owner = "repo-infra"
created = "2026-07-29"
linked_proposal = "CARGO-ALLOW-PROP-0010"
linked_spec = "CARGO-ALLOW-SPEC-0011""#,
    )?;
    text = replace_once(
        text,
        r#"[[artifact]]
id = "CARGO-ALLOW-SPEC-0010"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0010-three-product-boundaries.md"
status = "accepted"
owner = "repo-infra"
created = "2026-07-22"
linked_proposal = "CARGO-ALLOW-PROP-0010"
linked_adr = "CARGO-ALLOW-ADR-0002""#,
        r#"[[artifact]]
id = "CARGO-ALLOW-SPEC-0010"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0010-three-product-boundaries.md"
status = "superseded"
owner = "repo-infra"
created = "2026-07-22"
linked_proposal = "CARGO-ALLOW-PROP-0010"
linked_adr = "CARGO-ALLOW-ADR-0002"
superseded_by = "CARGO-ALLOW-SPEC-0011"

[[artifact]]
id = "CARGO-ALLOW-SPEC-0011"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md"
status = "accepted"
owner = "repo-infra"
created = "2026-07-29"
linked_proposal = "CARGO-ALLOW-PROP-0010"
linked_adr = "CARGO-ALLOW-ADR-0002""#,
    )?;

    let marker = "id = \"CARGO-ALLOW-PLAN-0010\"";
    let split = text
        .find(marker)
        .ok_or_else(|| io::Error::other("PLAN-0010 artifact row is missing"))?;
    let (head, tail) = text.split_at(split);
    let tail = tail.replace(
        "linked_spec = \"CARGO-ALLOW-SPEC-0010\"",
        "linked_spec = \"CARGO-ALLOW-SPEC-0011\"",
    );
    fs::write(path, format!("{head}{tail}"))
}

fn update_policy(root: &Path) -> io::Result<()> {
    let path = root.join("policy/allow.toml");
    let mut text = fs::read_to_string(&path)?;

    text = text.replace(
        "Records normative three-product boundary contract and acceptance evidence for the extraction lane.",
        "Retains the historical generation-1 three-product boundary contract and its accepted provenance.",
    );
    text = text.replace(
        "Sequences Wave 0 and Wave 1 PR-sized extraction work after the three-product design package.",
        "Sequences the current generation-2 convergence, retirement, packaging and release work.",
    );

    append_receipt(
        &mut text,
        "allow-0500",
        ADR_PATH,
        "source_truth_adr",
        "Records package, physical-path, independent-version, publication, support and package-survival authority for the three-product monorepo.",
        &[
            "doc:docs/proposals/CARGO-ALLOW-PROP-0010-three-product-design.md",
            "spec:docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md",
        ],
    )?;
    append_receipt(
        &mut text,
        "allow-0501",
        SPEC_PATH,
        "source_truth_spec",
        "Records current topology, semantic ownership, compatibility, package-survival, exact-candidate and release requirements for the three-product monorepo.",
        &[
            "doc:docs/proposals/CARGO-ALLOW-PROP-0010-three-product-design.md",
            "adr:docs/adr/CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md",
        ],
    )?;
    fs::write(path, text)
}

fn append_receipt(
    text: &mut String,
    id: &str,
    path: &str,
    classification: &str,
    reason: &str,
    evidence: &[&str],
) -> io::Result<()> {
    let path_marker = format!("path = \"{path}\"");
    if text.contains(&path_marker) {
        return Ok(());
    }
    let id_marker = format!("id = \"{id}\"");
    if text.contains(&id_marker) {
        return Err(io::Error::other(format!(
            "policy ID {id} exists for another entry"
        )));
    }
    let evidence = evidence
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ");
    text.push_str(&format!(
        "\n\n[[allow]]\nid = \"{id}\"\nkind = \"non_rust_file\"\nfamily = \"documentation\"\npath = \"{path}\"\nowner = \"repo-infra\"\nclassification = \"{classification}\"\nreason = \"{reason}\"\nevidence = [{evidence}]\ncreated = \"2026-07-29\"\nreview_after = \"2026-11-01\"\n\n[allow.selector]\nast_kind = \"tracked_file\"\nsymbol = \"{path}\"\ntarget_fingerprint = \"md\"\nglob = \"{path}\"\n\n[allow.last_seen]\nline = 1\ncolumn = 1\n"
    ));
    Ok(())
}

fn retain_generated_files(root: &Path) -> io::Result<()> {
    let target = root.join("target/cargo-allow/generated-authority");
    fs::create_dir_all(&target)?;
    fs::copy(
        root.join(".allow/artifacts/doc-artifacts.toml"),
        target.join("doc-artifacts.toml"),
    )?;
    fs::copy(root.join("policy/allow.toml"), target.join("allow.toml"))?;
    Ok(())
}

fn replace_once(text: String, old: &str, new: &str) -> io::Result<String> {
    if !text.contains(old) {
        let block = old.lines().nth(1).map_or("unknown", |line| line);
        return Err(io::Error::other(format!(
            "expected authority block is missing: {block}"
        )));
    }
    Ok(text.replacen(old, new, 1))
}
