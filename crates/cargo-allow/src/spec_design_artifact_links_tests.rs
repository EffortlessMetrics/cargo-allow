use allow_policy::spec_system::{
    SpecSystemRoots, load_doc_artifacts, validate_doc_artifact_files, validate_doc_artifact_links,
};
use std::path::PathBuf;

#[test]
fn spec_design_artifact_links() -> Result<(), String> {
    let root = repo_root();
    let ledger_path = root.join(".allow/artifacts/doc-artifacts.toml");
    let ledger = load_doc_artifacts(&ledger_path)
        .map_err(|err| format!("doc artifact ledger should load: {err}"))?;

    let design_ids = [
        "CARGO-ALLOW-PROP-0010",
        "CARGO-ALLOW-ADR-0002",
        "CARGO-ALLOW-SPEC-0010",
        "CARGO-ALLOW-PLAN-0010",
    ];
    for id in design_ids {
        let artifact = ledger
            .artifact
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| format!("ledger missing {id}"))?;
        let source_path = root.join(&artifact.path);
        if !source_path.is_file() {
            return Err(format!("{id} source file missing at {}", artifact.path));
        }
        let text = std::fs::read_to_string(&source_path)
            .map_err(|err| format!("artifact {id} readable: {err}"))?;
        if !text.contains(id) {
            return Err(format!("{id} not visible in {}", artifact.path));
        }
    }

    let prop = ledger
        .artifact
        .iter()
        .find(|entry| entry.id == "CARGO-ALLOW-PROP-0010")
        .ok_or_else(|| "proposal registered".to_string())?;
    let spec = ledger
        .artifact
        .iter()
        .find(|entry| entry.id == "CARGO-ALLOW-SPEC-0010")
        .ok_or_else(|| "spec registered".to_string())?;
    let adr = ledger
        .artifact
        .iter()
        .find(|entry| entry.id == "CARGO-ALLOW-ADR-0002")
        .ok_or_else(|| "adr registered".to_string())?;
    let plan = ledger
        .artifact
        .iter()
        .find(|entry| entry.id == "CARGO-ALLOW-PLAN-0010")
        .ok_or_else(|| "plan registered".to_string())?;

    if spec.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010") {
        return Err("spec linked_proposal mismatch".to_string());
    }
    if adr.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010") {
        return Err("adr linked_proposal mismatch".to_string());
    }
    if adr.linked_spec.as_deref() != Some("CARGO-ALLOW-SPEC-0010") {
        return Err("adr linked_spec mismatch".to_string());
    }
    if plan.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010") {
        return Err("plan linked_proposal mismatch".to_string());
    }
    if plan.linked_spec.as_deref() != Some("CARGO-ALLOW-SPEC-0010") {
        return Err("plan linked_spec mismatch".to_string());
    }
    if plan.linked_adr.as_deref() != Some("CARGO-ALLOW-ADR-0002") {
        return Err("plan linked_adr mismatch".to_string());
    }
    if plan.linked_support_tier.as_deref() != Some("CARGO-ALLOW-SUPPORT-0001") {
        return Err("plan linked_support_tier mismatch".to_string());
    }

    validate_doc_artifact_links(&ledger)
        .map_err(|err| format!("artifact graph links should resolve: {err}"))?;
    validate_doc_artifact_files(&root, &ledger, &test_roots())
        .map_err(|err| format!("artifact files should exist under configured roots: {err}"))?;

    let fixture_readme = root.join("tests/fixtures/three-product-design/README.md");
    let fixture_text = std::fs::read_to_string(&fixture_readme)
        .map_err(|err| format!("fixture readme readable: {err}"))?;
    if !fixture_text.contains("CARGO-ALLOW-PROP-0010") {
        return Err("fixture missing proposal id".to_string());
    }
    if !fixture_text.contains("#2612") {
        return Err("fixture missing topology owner".to_string());
    }
    if !prop.path.ends_with("three-product-design.md") {
        return Err("proposal path mismatch".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn test_roots() -> SpecSystemRoots {
    SpecSystemRoots {
        proposals: "docs/proposals".to_string(),
        specs: "docs/specs".to_string(),
        adrs: "docs/adr".to_string(),
        plans: "plans".to_string(),
        goals: Some(".allow/goals".to_string()),
        support_tiers: "docs/status/SUPPORT_TIERS.md".to_string(),
        artifact_ledger: ".allow/artifacts/doc-artifacts.toml".to_string(),
    }
}
