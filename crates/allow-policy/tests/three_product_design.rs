use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconstructionFixture {
    schema_version: String,
    design_package_proposal: String,
    ownership_adr: String,
    package_identity_adr: String,
    historical_spec: String,
    current_spec: String,
    design_package_plan: String,
    crate_topology_owner_issue: u32,
    package_topology_owner_issue: u32,
    move_ledger_owner_issue: u32,
    shim_owner_issue: u32,
    parity_owner_issue: u32,
    release_controller_issue: u32,
    repository_extraction_authorized: bool,
    product: Vec<ProductRow>,
    shared: Vec<SharedRow>,
    entry: Vec<DispositionEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductRow {
    id: String,
    status: String,
    current_version: String,
    #[serde(default)]
    published_version: Option<String>,
    #[serde(default)]
    target_version: Option<String>,
    claim: String,
    release_blocked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedRow {
    logical_id: String,
    workspace_path: String,
    current_package: String,
    target_package: String,
    lib_name: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispositionEntry {
    artifact: String,
    disposition: String,
    note: String,
}

fn workspace_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "allow-policy manifest should have a workspace root".to_string())
}

fn read(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(relative);
    std::fs::read_to_string(&path)
        .map_err(|error| format!("read generation-2 artifact {}: {error}", path.display()))
}

fn require_contains(text: &str, needle: &str, label: &str) -> Result<(), String> {
    if text.contains(needle) {
        Ok(())
    } else {
        Err(format!("{label} is missing required generation-2 text: {needle}"))
    }
}

#[test]
fn three_product_generation_two_reconstructs_current_authority() -> Result<(), String> {
    let root = workspace_root()?;
    let fixture_text = read(
        &root,
        "tests/fixtures/three-product-design/disposition-map.toml",
    )?;
    let fixture = toml::from_str::<ReconstructionFixture>(&fixture_text)
        .map_err(|error| format!("parse generation-2 reconstruction fixture: {error}"))?;

    if fixture.schema_version != "2.0" {
        return Err(format!(
            "expected reconstruction schema 2.0, got {}",
            fixture.schema_version
        ));
    }
    if fixture.repository_extraction_authorized {
        return Err("generation-2 fixture must not authorize repository extraction".to_string());
    }

    let expected_authorities = [
        (&fixture.design_package_proposal, "CARGO-ALLOW-PROP-0010"),
        (&fixture.ownership_adr, "CARGO-ALLOW-ADR-0002"),
        (&fixture.package_identity_adr, "CARGO-ALLOW-ADR-0003"),
        (&fixture.historical_spec, "CARGO-ALLOW-SPEC-0010"),
        (&fixture.current_spec, "CARGO-ALLOW-SPEC-0011"),
        (&fixture.design_package_plan, "CARGO-ALLOW-PLAN-0010"),
    ];
    for (observed, expected) in expected_authorities {
        if observed != expected {
            return Err(format!("expected authority {expected}, got {observed}"));
        }
    }

    let expected_issues = [
        (fixture.crate_topology_owner_issue, 2612_u32),
        (fixture.package_topology_owner_issue, 2604_u32),
        (fixture.move_ledger_owner_issue, 2598_u32),
        (fixture.shim_owner_issue, 2607_u32),
        (fixture.parity_owner_issue, 2606_u32),
        (fixture.release_controller_issue, 2371_u32),
    ];
    for (observed, expected) in expected_issues {
        if observed != expected {
            return Err(format!("expected controlling issue #{expected}, got #{observed}"));
        }
    }

    let allow = fixture
        .product
        .iter()
        .find(|row| row.id == "cargo-allow")
        .ok_or_else(|| "fixture is missing cargo-allow product row".to_string())?;
    if allow.status != "SupportedSourceCandidate"
        || allow.current_version != "0.2.0"
        || allow.published_version.as_deref() != Some("0.1.11")
        || !allow.release_blocked
        || allow.claim != "source-exception ledger"
    {
        return Err(format!("unexpected cargo-allow product row: {allow:?}"));
    }

    for product_id in ["cargo-intent", "cargo-proof"] {
        let row = fixture
            .product
            .iter()
            .find(|row| row.id == product_id)
            .ok_or_else(|| format!("fixture is missing {product_id} product row"))?;
        if row.status != "LandedExperimental"
            || row.target_version.as_deref() != Some("0.1.0")
            || !row.release_blocked
            || row.claim.trim().is_empty()
        {
            return Err(format!("unexpected {product_id} product row: {row:?}"));
        }
    }

    let expected_shared = [
        (
            "repo-protocol",
            "crates/repo-protocol",
            "effortless-repo-protocol",
            "repo_protocol",
        ),
        (
            "repo-snapshot",
            "crates/repo-snapshot",
            "effortless-repo-snapshot",
            "repo_snapshot",
        ),
        (
            "repo-edit",
            "crates/repo-edit",
            "effortless-repo-edit",
            "repo_edit",
        ),
        (
            "rust-source-index",
            "crates/rust-source-index",
            "effortless-rust-source-index",
            "rust_source_index",
        ),
    ];
    let mut seen = BTreeSet::new();
    for (logical_id, workspace_path, target_package, lib_name) in expected_shared {
        let row = fixture
            .shared
            .iter()
            .find(|row| row.logical_id == logical_id)
            .ok_or_else(|| format!("fixture is missing shared row {logical_id}"))?;
        if !seen.insert(row.logical_id.as_str()) {
            return Err(format!("duplicate shared logical ID {}", row.logical_id));
        }
        if row.workspace_path != workspace_path
            || row.current_package != logical_id
            || row.target_package != target_package
            || row.lib_name != lib_name
            || row.status != "LandedTransitional"
        {
            return Err(format!("unexpected shared row {logical_id}: {row:?}"));
        }
    }
    if fixture.shared.len() != expected_shared.len() {
        return Err(format!(
            "expected {} shared rows, got {}",
            expected_shared.len(),
            fixture.shared.len()
        ));
    }

    let required_dispositions = [
        ("CARGO-ALLOW-PROP-0010", "CurrentCanonical"),
        ("CARGO-ALLOW-ADR-0002", "CurrentCanonical"),
        ("CARGO-ALLOW-ADR-0003", "CurrentCanonical"),
        ("CARGO-ALLOW-SPEC-0010", "HistoricalOnly"),
        ("CARGO-ALLOW-SPEC-0011", "CurrentCanonical"),
        ("allow-policy::spec_system", "CompatibilityOnly"),
        ("cargo-allow::spec_system", "BlockedOnParity"),
        ("#2598", "CurrentSupporting"),
        ("#2604", "CurrentSupporting"),
        ("#2606", "CurrentSupporting"),
        ("#2607", "CurrentSupporting"),
    ];
    for (artifact, disposition) in required_dispositions {
        let entry = fixture
            .entry
            .iter()
            .find(|entry| entry.artifact == artifact && entry.disposition == disposition)
            .ok_or_else(|| {
                format!("fixture is missing disposition {artifact} = {disposition}")
            })?;
        if entry.note.trim().is_empty() {
            return Err(format!("disposition {artifact} has an empty note"));
        }
    }

    let proposal = read(
        &root,
        "docs/proposals/CARGO-ALLOW-PROP-0010-three-product-design.md",
    )?;
    require_contains(&proposal, "SupportedSourceCandidate", "proposal")?;
    require_contains(&proposal, "LandedExperimental", "proposal")?;
    require_contains(&proposal, "CARGO-ALLOW-ADR-0003", "proposal")?;

    let package_adr = read(
        &root,
        "docs/adr/CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md",
    )?;
    require_contains(
        &package_adr,
        "effortless-repo-protocol",
        "package identity ADR",
    )?;
    require_contains(&package_adr, "RegistryTransitiveOnly", "package identity ADR")?;

    let historical_spec = read(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0010-three-product-boundaries.md",
    )?;
    require_contains(
        &historical_spec,
        "superseded_by: CARGO-ALLOW-SPEC-0011",
        "historical spec",
    )?;
    require_contains(
        &historical_spec,
        "Exact supersession map",
        "historical spec",
    )?;

    let current_spec = read(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md",
    )?;
    require_contains(
        &current_spec,
        "identity-distinguishes-logical-package-lib",
        "generation-2 spec",
    )?;
    require_contains(
        &current_spec,
        "release-requires-evidence-backed-complete",
        "generation-2 spec",
    )?;

    let plan = read(&root, "plans/three-product-crate-extraction.md")?;
    require_contains(&plan, "Stage H — topology-selected exact cargo-allow candidate", "plan")?;
    require_contains(&plan, "#2501 exact candidate refreeze", "plan")?;

    Ok(())
}
