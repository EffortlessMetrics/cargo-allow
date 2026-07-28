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
        "CARGO-ALLOW-ADR-0003",
        "CARGO-ALLOW-SPEC-0010",
        "CARGO-ALLOW-SPEC-0011",
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

    let prop = artifact(&ledger, "CARGO-ALLOW-PROP-0010")?;
    let ownership_adr = artifact(&ledger, "CARGO-ALLOW-ADR-0002")?;
    let package_adr = artifact(&ledger, "CARGO-ALLOW-ADR-0003")?;
    let historical_spec = artifact(&ledger, "CARGO-ALLOW-SPEC-0010")?;
    let current_spec = artifact(&ledger, "CARGO-ALLOW-SPEC-0011")?;
    let plan = artifact(&ledger, "CARGO-ALLOW-PLAN-0010")?;
    let support = artifact(&ledger, "CARGO-ALLOW-SUPPORT-0001")?;

    require_link(
        ownership_adr.linked_proposal.as_deref(),
        "CARGO-ALLOW-PROP-0010",
        "ownership ADR linked_proposal",
    )?;
    require_link(
        ownership_adr.linked_spec.as_deref(),
        "CARGO-ALLOW-SPEC-0011",
        "ownership ADR linked_spec",
    )?;
    require_link(
        package_adr.linked_proposal.as_deref(),
        "CARGO-ALLOW-PROP-0010",
        "package ADR linked_proposal",
    )?;
    require_link(
        package_adr.linked_spec.as_deref(),
        "CARGO-ALLOW-SPEC-0011",
        "package ADR linked_spec",
    )?;
    require_link(
        historical_spec.linked_proposal.as_deref(),
        "CARGO-ALLOW-PROP-0010",
        "historical spec linked_proposal",
    )?;
    if historical_spec.status != allow_policy::spec_system::ArtifactStatus::Superseded {
        return Err(format!(
            "historical spec should be Superseded, got {:?}",
            historical_spec.status
        ));
    }
    let historical_text = std::fs::read_to_string(root.join(&historical_spec.path))
        .map_err(|err| format!("historical spec readable: {err}"))?;
    if !historical_text.contains("superseded_by: CARGO-ALLOW-SPEC-0011") {
        return Err("historical spec source is missing its exact successor".to_string());
    }
    require_link(
        current_spec.linked_proposal.as_deref(),
        "CARGO-ALLOW-PROP-0010",
        "current spec linked_proposal",
    )?;
    require_link(
        current_spec.linked_adr.as_deref(),
        "CARGO-ALLOW-ADR-0002",
        "current spec linked_adr",
    )?;
    require_link(
        plan.linked_proposal.as_deref(),
        "CARGO-ALLOW-PROP-0010",
        "plan linked_proposal",
    )?;
    require_link(
        plan.linked_spec.as_deref(),
        "CARGO-ALLOW-SPEC-0011",
        "plan linked_spec",
    )?;
    require_link(
        plan.linked_adr.as_deref(),
        "CARGO-ALLOW-ADR-0002",
        "plan linked_adr",
    )?;
    require_link(
        plan.linked_support_tier.as_deref(),
        "CARGO-ALLOW-SUPPORT-0001",
        "plan linked_support_tier",
    )?;
    require_link(
        support.linked_proposal.as_deref(),
        "CARGO-ALLOW-PROP-0010",
        "support linked_proposal",
    )?;
    require_link(
        support.linked_spec.as_deref(),
        "CARGO-ALLOW-SPEC-0011",
        "support linked_spec",
    )?;

    validate_doc_artifact_links(&ledger)
        .map_err(|err| format!("artifact graph links should resolve: {err}"))?;
    validate_doc_artifact_files(&root, &ledger, &test_roots())
        .map_err(|err| format!("artifact files should exist under configured roots: {err}"))?;

    let fixture_readme = root.join("tests/fixtures/three-product-design/README.md");
    let fixture_text = std::fs::read_to_string(&fixture_readme)
        .map_err(|err| format!("fixture readme readable: {err}"))?;
    for needle in [
        "CARGO-ALLOW-PROP-0010",
        "CARGO-ALLOW-ADR-0003",
        "CARGO-ALLOW-SPEC-0011",
        "#2612",
        "#2501",
        "effortless-repo-protocol",
    ] {
        if !fixture_text.contains(needle) {
            return Err(format!("fixture missing generation-2 marker {needle}"));
        }
    }
    if !prop.path.ends_with("three-product-design.md") {
        return Err("proposal path mismatch".to_string());
    }

    Ok(())
}

fn artifact<'a>(
    ledger: &'a allow_policy::spec_system::DocArtifactLedger,
    id: &str,
) -> Result<&'a allow_policy::spec_system::DocArtifact, String> {
    ledger
        .artifact
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| format!("artifact ledger missing {id}"))
}

fn require_link(observed: Option<&str>, expected: &str, label: &str) -> Result<(), String> {
    if observed == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "{label} mismatch: expected {expected}, got {}",
            observed.unwrap_or("<missing>")
        ))
    }
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
