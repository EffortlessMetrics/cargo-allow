use allow_policy::product_crates::{
    ArchitectureDiagnosticKind, load_workspace_dependency_graph,
    validate_architecture_denominators_at, validate_architecture_manifest_at,
    validate_architecture_with_dependency_graph_at, workspace_members_from_manifest,
};
use std::path::PathBuf;

#[test]
fn product_crate_architecture_report_only_inventory() -> Result<(), String> {
    let root = repo_root();
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let (manifest, diagnostics, report) =
        validate_architecture_manifest_at(&root, &manifest_path, &members)
            .map_err(|err| format!("validate architecture manifest: {err}"))?;

    if diagnostics
        .iter()
        .any(|diag| diag.kind == ArchitectureDiagnosticKind::UnownedWorkspaceCrate)
    {
        return Err(format!("unowned workspace crates: {diagnostics:?}"));
    }
    assert_eq!(manifest.manifest_id, "CARGO-ALLOW-ARCH-0001");
    assert_eq!(manifest.controlling_issue, 2580);
    assert_eq!(report.planned_crate_count, 0);

    let law = root.join("docs/architecture/product-crate-law.md");
    let law_text = std::fs::read_to_string(&law)
        .map_err(|err| format!("product crate law readable: {err}"))?;
    if !law_text.contains("cargo-allow") {
        return Err("human projection missing cargo-allow ownership".to_string());
    }

    Ok(())
}

#[test]
fn product_crate_dependency_law_loads_workspace_graph() -> Result<(), String> {
    let root = repo_root();
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let graph = load_workspace_dependency_graph(&root)
        .map_err(|err| format!("load workspace dependency graph: {err}"))?;
    if graph.edges.is_empty() {
        return Err("workspace dependency graph should contain dependency edges".to_string());
    }

    let (_, diagnostics, _) =
        validate_architecture_with_dependency_graph_at(&root, &manifest_path, &members, &graph)
            .map_err(|err| format!("validate with dependency graph: {err}"))?;

    let dev_bypasses: Vec<_> = diagnostics
        .iter()
        .filter(|diag| diag.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency)
        .filter(|diag| diag.message.contains("cargo-allow") && diag.message.contains("dev"))
        .collect();
    if dev_bypasses.is_empty() {
        return Err(
            "expected cargo-allow dev dependency bypasses on intent crates to remain visible"
                .to_string(),
        );
    }

    let has_normal_forbidden = diagnostics.iter().any(|diag| {
        diag.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency
            && diag.message.contains("normal dependency")
    });
    if has_normal_forbidden {
        return Err(format!(
            "workspace should not have forbidden normal product dependencies yet: {diagnostics:?}"
        ));
    }

    Ok(())
}

#[test]
fn product_crate_architecture_denominators_align() -> Result<(), String> {
    let root = repo_root();
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("manifest readable: {err}"))?;
    let manifest = allow_policy::product_crates::parse_architecture_manifest(&text)
        .map_err(|err| format!("parse manifest: {err}"))?;
    let (diagnostics, report) = validate_architecture_denominators_at(&root, &manifest, &members)
        .map_err(|err| format!("validate denominators: {err}"))?;
    if diagnostics.iter().any(|diag| {
        matches!(
            diag.kind,
            ArchitectureDiagnosticKind::ManifestTopologyLinkMismatch
                | ArchitectureDiagnosticKind::ManifestMoveLedgerLinkMismatch
                | ArchitectureDiagnosticKind::PackageTopologyFamilyMismatch
                | ArchitectureDiagnosticKind::ArchitectureCrateMissingFromTopology
                | ArchitectureDiagnosticKind::PackageTopologyCrateMissingFromArchitecture
                | ArchitectureDiagnosticKind::PlannedCrateNowPresent
                | ArchitectureDiagnosticKind::MoveLedgerUnknownTargetCrate
        )
    }) {
        return Err(format!("architecture denominators drift: {diagnostics:?}"));
    }
    if report.architecture_crate_count != report.workspace_member_count {
        return Err(format!(
            "architecture inventory count {} should match workspace members {}",
            report.architecture_crate_count, report.workspace_member_count
        ));
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
