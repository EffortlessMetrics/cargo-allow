use allow_policy::spec_system::{
    ArtifactStatus, SpecSystemRoots, SupportTierLevel, load_doc_artifacts,
    validate_doc_artifact_files, validate_doc_artifact_links, validate_support_tier_claims,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconstructionFixture {
    schema_version: String,
    authority_generation: u32,
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
    observed_package_count: usize,
    target_package_count: usize,
    observed_shared_package_count: usize,
    target_shared_package_count: usize,
    governance_model_owner: String,
    governance_validation_owner: String,
    governance_receipt_owner: String,
    proof_protocol_role: String,
    proof_engine_role: String,
    repository_extraction_authorized: bool,
    release_authorized: bool,
    cargo_proof_qualification_blocks_cargo_allow_release: bool,
    product: Vec<ProductRow>,
    shared: Vec<SharedRow>,
    collapse: Vec<CollapseRow>,
    entry: Vec<DispositionEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductRow {
    id: String,
    status: String,
    current_version: String,
    target_version: String,
    #[serde(default)]
    published_version: Option<String>,
    claim: String,
    observed_package_count: usize,
    target_package_count: usize,
    release_blocked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedRow {
    logical_id: String,
    current_workspace_path: String,
    target_workspace_path: String,
    current_package: String,
    target_package: String,
    lib_name: String,
    status: String,
    target_disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollapseRow {
    logical_id: String,
    current_workspace_path: String,
    current_package: String,
    target_container: String,
    target_module: String,
    disposition: String,
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

#[test]
fn three_product_generation_two_reconstructs_exact_current_and_target_authority()
-> Result<(), String> {
    let root = workspace_root()?;
    let fixture_text = read(
        &root,
        "tests/fixtures/three-product-design/disposition-map.toml",
    )?;
    let fixture = toml::from_str::<ReconstructionFixture>(&fixture_text)
        .map_err(|error| format!("parse generation-2 reconstruction fixture: {error}"))?;

    if fixture.schema_version != "2.0" || fixture.authority_generation != 2 {
        return Err("fixture does not select exact generation 2".to_string());
    }
    if fixture.repository_extraction_authorized || fixture.release_authorized {
        return Err("fixture must not authorize repository extraction or release".to_string());
    }
    if fixture.cargo_proof_qualification_blocks_cargo_allow_release {
        return Err(
            "cargo-proof qualification must remain independent of cargo-allow release".to_string(),
        );
    }

    let authorities = [
        (&fixture.design_package_proposal, "CARGO-ALLOW-PROP-0010"),
        (&fixture.ownership_adr, "CARGO-ALLOW-ADR-0002"),
        (&fixture.package_identity_adr, "CARGO-ALLOW-ADR-0003"),
        (&fixture.historical_spec, "CARGO-ALLOW-SPEC-0010"),
        (&fixture.current_spec, "CARGO-ALLOW-SPEC-0011"),
        (&fixture.design_package_plan, "CARGO-ALLOW-PLAN-0010"),
    ];
    for (observed, expected) in authorities {
        if observed != expected {
            return Err(format!("expected authority {expected}, got {observed}"));
        }
    }

    let issue_owners = [
        (fixture.crate_topology_owner_issue, 2612_u32),
        (fixture.package_topology_owner_issue, 2604_u32),
        (fixture.move_ledger_owner_issue, 2598_u32),
        (fixture.shim_owner_issue, 2607_u32),
        (fixture.parity_owner_issue, 2606_u32),
        (fixture.release_controller_issue, 2371_u32),
    ];
    for (observed, expected) in issue_owners {
        if observed != expected {
            return Err(format!(
                "expected controlling issue #{expected}, got #{observed}"
            ));
        }
    }
    if fixture.governance_model_owner != "intent-model"
        || fixture.governance_validation_owner != "intent-engine"
        || fixture.governance_receipt_owner != "cargo-intent/repository-ci"
        || fixture.proof_protocol_role != "data-contracts"
        || fixture.proof_engine_role != "semantic-evaluation"
    {
        return Err("generation-2 semantic ownership fields do not match".to_string());
    }

    let expected_products = [
        (
            "cargo-allow",
            "SupportedSourceCandidate",
            "0.2.0",
            "0.2.0",
            Some("0.1.11"),
            "source-exception ledger",
            10_usize,
            10_usize,
        ),
        (
            "cargo-intent",
            "LandedExperimental",
            "0.2.0-workspace-transitional",
            "0.1.0",
            None,
            "durable authored intent and obligation compiler",
            5_usize,
            5_usize,
        ),
        (
            "cargo-proof",
            "LandedExperimental",
            "0.2.0-workspace-transitional",
            "0.1.0",
            None,
            "exact-snapshot evidence orchestration",
            8_usize,
            3_usize,
        ),
    ];
    if fixture.product.len() != expected_products.len() {
        return Err(format!(
            "unexpected product row count {}",
            fixture.product.len()
        ));
    }
    let mut seen_products = BTreeSet::new();
    for (id, status, current, target, published, claim, observed_count, target_count) in
        expected_products
    {
        let row = fixture
            .product
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| format!("fixture is missing product {id}"))?;
        if !seen_products.insert(row.id.as_str()) {
            return Err(format!("duplicate product {}", row.id));
        }
        if row.status != status
            || row.current_version != current
            || row.target_version != target
            || row.published_version.as_deref() != published
            || row.claim != claim
            || row.observed_package_count != observed_count
            || row.target_package_count != target_count
            || !row.release_blocked
        {
            return Err(format!("unexpected product row {id}: {row:?}"));
        }
    }

    let observed_total = fixture
        .product
        .iter()
        .map(|row| row.observed_package_count)
        .sum::<usize>()
        + fixture.observed_shared_package_count;
    let target_total = fixture
        .product
        .iter()
        .map(|row| row.target_package_count)
        .sum::<usize>()
        + fixture.target_shared_package_count;
    if observed_total != 27 || observed_total != fixture.observed_package_count {
        return Err(format!(
            "unexpected observed topology denominator {observed_total}"
        ));
    }
    if target_total != 22 || target_total != fixture.target_package_count {
        return Err(format!(
            "unexpected target topology denominator {target_total}"
        ));
    }

    let expected_shared = [
        (
            "repo-protocol",
            "crates/repo-protocol",
            "crates/effortless-repo-protocol",
            "repo-protocol",
            "effortless-repo-protocol",
            "repo_protocol",
        ),
        (
            "repo-snapshot",
            "crates/repo-snapshot",
            "crates/effortless-repo-snapshot",
            "repo-snapshot",
            "effortless-repo-snapshot",
            "repo_snapshot",
        ),
        (
            "repo-edit",
            "crates/repo-edit",
            "crates/effortless-repo-edit",
            "repo-edit",
            "effortless-repo-edit",
            "repo_edit",
        ),
        (
            "rust-source-index",
            "crates/rust-source-index",
            "crates/effortless-rust-source-index",
            "rust-source-index",
            "effortless-rust-source-index",
            "rust_source_index",
        ),
    ];
    if fixture.shared.len() != expected_shared.len() {
        return Err(format!(
            "unexpected shared row count {}",
            fixture.shared.len()
        ));
    }
    let mut seen_shared = BTreeSet::new();
    for (logical, current_path, target_path, current_package, target_package, lib_name) in
        expected_shared
    {
        let row = fixture
            .shared
            .iter()
            .find(|row| row.logical_id == logical)
            .ok_or_else(|| format!("fixture is missing shared logical ID {logical}"))?;
        if !seen_shared.insert(row.logical_id.as_str()) {
            return Err(format!("duplicate shared logical ID {}", row.logical_id));
        }
        if row.current_workspace_path != current_path
            || row.target_workspace_path != target_path
            || row.current_package != current_package
            || row.target_package != target_package
            || row.lib_name != lib_name
            || row.status != "LandedTransitional"
            || row.target_disposition != "RetainPackage"
            || row.logical_id == row.target_package
        {
            return Err(format!("unexpected shared row {logical}: {row:?}"));
        }
    }

    let expected_collapses = BTreeMap::from([
        (
            "proof-provider-api",
            ("proof-engine", "proof_engine::provider"),
        ),
        (
            "proof-adapter-command",
            ("cargo-proof", "cargo_proof::providers::command"),
        ),
        (
            "proof-adapter-cargo-allow",
            ("cargo-proof", "cargo_proof::providers::cargo_allow"),
        ),
        (
            "proof-adapter-ripr",
            ("cargo-proof", "cargo_proof::providers::ripr"),
        ),
        (
            "proof-adapter-hawk",
            ("cargo-proof", "cargo_proof::providers::hawk"),
        ),
    ]);
    if fixture.collapse.len() != expected_collapses.len() {
        return Err(format!(
            "unexpected collapse row count {}",
            fixture.collapse.len()
        ));
    }
    let mut seen_collapses = BTreeSet::new();
    for row in &fixture.collapse {
        if !seen_collapses.insert(row.logical_id.as_str()) {
            return Err(format!("duplicate collapse logical ID {}", row.logical_id));
        }
        let Some((container, module)) = expected_collapses.get(row.logical_id.as_str()) else {
            return Err(format!("unexpected collapsed package {}", row.logical_id));
        };
        if row.current_package != row.logical_id
            || row.current_workspace_path != format!("crates/{}", row.logical_id)
            || row.target_container != *container
            || row.target_module != *module
            || row.target_module.trim().is_empty()
            || row.disposition != "CollapseIntoPackage"
        {
            return Err(format!(
                "unexpected collapse row {}: {row:?}",
                row.logical_id
            ));
        }
    }

    let expected_dispositions = [
        ("CARGO-ALLOW-PROP-0001", "CurrentSupporting"),
        ("CARGO-ALLOW-SPEC-0001", "CurrentSupporting"),
        ("CARGO-ALLOW-PROP-0010", "CurrentCanonical"),
        ("CARGO-ALLOW-ADR-0002", "CurrentCanonical"),
        ("CARGO-ALLOW-ADR-0003", "CurrentCanonical"),
        ("CARGO-ALLOW-SPEC-0010", "HistoricalOnly"),
        ("CARGO-ALLOW-SPEC-0011", "CurrentCanonical"),
        ("plans/spec-system/implementation-plan.md", "HistoricalOnly"),
        ("allow-policy::spec_system", "CompatibilityOnly"),
        ("cargo-allow::spec_system", "BlockedOnParity"),
        ("#2550", "CurrentCanonical"),
        ("#2612", "CurrentSupporting"),
        ("#2598", "CurrentSupporting"),
        ("#2604", "CurrentSupporting"),
        ("#2606", "CurrentSupporting"),
        ("#2607", "CurrentSupporting"),
    ];
    if fixture.entry.len() != expected_dispositions.len() {
        return Err(format!(
            "unexpected disposition row count {}",
            fixture.entry.len()
        ));
    }
    let mut seen_entries = BTreeSet::new();
    for (artifact, disposition) in expected_dispositions {
        let row = fixture
            .entry
            .iter()
            .find(|row| row.artifact == artifact)
            .ok_or_else(|| format!("fixture is missing disposition {artifact}"))?;
        if !seen_entries.insert(row.artifact.as_str()) {
            return Err(format!("duplicate disposition {}", row.artifact));
        }
        if row.disposition != disposition || row.note.trim().is_empty() {
            return Err(format!("unexpected disposition {artifact}: {row:?}"));
        }
    }

    validate_artifact_lifecycle(&root)?;
    validate_support_contract(&root)?;
    validate_current_spec(&root)?;
    Ok(())
}

fn validate_artifact_lifecycle(root: &Path) -> Result<(), String> {
    let ledger_path = root.join(".allow/artifacts/doc-artifacts.toml");
    let ledger = load_doc_artifacts(&ledger_path)
        .map_err(|error| format!("load document artifact ledger: {error}"))?;
    let package_adr = ledger
        .artifact
        .iter()
        .find(|artifact| artifact.id == "CARGO-ALLOW-ADR-0003")
        .ok_or_else(|| "artifact ledger is missing ADR-0003".to_string())?;
    if package_adr.status != ArtifactStatus::Accepted
        || package_adr.created != "2026-07-29"
        || package_adr.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010")
        || package_adr.linked_spec.as_deref() != Some("CARGO-ALLOW-SPEC-0011")
    {
        return Err(format!("unexpected package ADR lifecycle: {package_adr:?}"));
    }
    let historical = ledger
        .artifact
        .iter()
        .find(|artifact| artifact.id == "CARGO-ALLOW-SPEC-0010")
        .ok_or_else(|| "artifact ledger is missing SPEC-0010".to_string())?;
    if historical.status != ArtifactStatus::Superseded
        || historical.superseded_by.as_deref() != Some("CARGO-ALLOW-SPEC-0011")
    {
        return Err(format!(
            "unexpected historical spec lifecycle: {historical:?}"
        ));
    }
    let current = ledger
        .artifact
        .iter()
        .find(|artifact| artifact.id == "CARGO-ALLOW-SPEC-0011")
        .ok_or_else(|| "artifact ledger is missing SPEC-0011".to_string())?;
    if current.status != ArtifactStatus::Accepted
        || current.created != "2026-07-29"
        || current.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010")
        || current.linked_adr.as_deref() != Some("CARGO-ALLOW-ADR-0002")
    {
        return Err(format!("unexpected current spec lifecycle: {current:?}"));
    }
    validate_doc_artifact_links(&ledger)
        .map_err(|error| format!("validate document artifact links: {error}"))?;
    validate_doc_artifact_files(root, &ledger, &test_roots())
        .map_err(|error| format!("validate document artifact files: {error}"))?;
    Ok(())
}

fn validate_support_contract(root: &Path) -> Result<(), String> {
    let support_text = read(root, "docs/status/SUPPORT_TIERS.md")?;
    if support_text.contains("cargo-intent (planned)")
        || support_text.contains("cargo-proof (planned)")
        || support_text.contains("Generation-1 parser compatibility table")
    {
        return Err("support tiers retain a competing generation-1 table".to_string());
    }
    if support_text
        .matches("| Surface | Tier | Claim | Proof command | Limitations |")
        .count()
        != 1
    {
        return Err("support tiers must contain exactly one current claims table".to_string());
    }

    let support_rows = validate_support_tier_claims(&support_text)
        .map_err(|error| format!("validate generation-2 support tiers: {error}"))?;
    let expected_tiers = BTreeMap::from([
        (
            "cargo-allow published source-exception ledger",
            SupportTierLevel::Stable,
        ),
        (
            "cargo-allow 0.2 source candidate",
            SupportTierLevel::Stabilizing,
        ),
        ("cargo-intent", SupportTierLevel::Experimental),
        ("cargo-proof", SupportTierLevel::Experimental),
        (
            "Historical spec-system artifacts",
            SupportTierLevel::Compatibility,
        ),
        ("target 22-package topology", SupportTierLevel::Advisory),
        (
            "physical repository extraction",
            SupportTierLevel::NotIncluded,
        ),
    ]);
    for (surface, tier) in expected_tiers {
        let row = support_rows
            .iter()
            .find(|row| row.surface == surface)
            .ok_or_else(|| format!("support tiers are missing {surface}"))?;
        if row.tier != tier {
            return Err(format!(
                "support tier {surface} expected {tier:?}, got {:?}",
                row.tier
            ));
        }
    }
    let compatibility = support_rows
        .iter()
        .find(|row| row.surface == "Historical spec-system artifacts")
        .ok_or_else(|| "missing historical compatibility support row".to_string())?;
    let compatibility_text = format!("{} {}", compatibility.claim, compatibility.notes);
    if !compatibility_text.contains("Cargo-intent owns current intent authority")
        || !compatibility_text.contains("fails explicitly")
        || !compatibility_text.contains("retire")
    {
        return Err("historical compatibility row lacks owner/failure/retirement law".to_string());
    }
    Ok(())
}

fn validate_current_spec(root: &Path) -> Result<(), String> {
    let historical_text = read(
        root,
        "docs/specs/CARGO-ALLOW-SPEC-0010-three-product-boundaries.md",
    )?;
    if !historical_text.contains("superseded_by: CARGO-ALLOW-SPEC-0011") {
        return Err("historical spec source does not name its exact successor".to_string());
    }
    let current_text = read(
        root,
        "docs/specs/CARGO-ALLOW-SPEC-0011-three-product-convergence.md",
    )?;
    for requirement in [
        "identity-distinguishes-logical-path-alias-package-lib",
        "support-visibility-and-extraction-separate",
        "release-requires-evidence-backed-complete",
    ] {
        if !current_text.contains(requirement) {
            return Err(format!("current spec is missing requirement {requirement}"));
        }
    }
    Ok(())
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
