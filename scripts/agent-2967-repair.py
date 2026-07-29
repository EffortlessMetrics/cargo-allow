from __future__ import annotations

from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_tail(path: Path, marker: str, replacement: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    start = text.find(marker)
    if start < 0:
        raise SystemExit(f"{label}: marker not found")
    if text.find(marker, start + len(marker)) >= 0:
        raise SystemExit(f"{label}: marker is not unique")
    path.write_text(text[:start] + replacement, encoding="utf-8")


minimal = Path("crates/allow-policy/src/spec_system/design_package_tests.rs")
text = minimal.read_text(encoding="utf-8")
for name in ["ProductRow", "CollapseRow", "ReconstructionFixture"]:
    old = f"#[derive(Debug, serde::Deserialize)]\n#[serde(deny_unknown_fields)]\nstruct {name}"
    new = f"#[derive(Debug, serde::Deserialize)]\nstruct {name}"
    if text.count(old) != 1:
        raise SystemExit(f"minimal fixture DTO {name}: expected one strict annotation")
    text = text.replace(old, new, 1)
minimal.write_text(text, encoding="utf-8")

path = Path("crates/allow-policy/src/spec_system/tests.rs")
replace_once(
    path,
    '''    assert_eq!(rows.len(), 8);
    assert!(rows.iter().any(|row| {
        row.surface == "Spec-system profile" && row.tier == SupportTierLevel::Advisory
    }));
    assert!(rows.iter().any(|row| {
        row.surface == "cargo-intent (planned)" && row.tier == SupportTierLevel::Advisory
    }));
    assert!(rows.iter().any(|row| {
        row.surface == "cargo-proof (planned)" && row.tier == SupportTierLevel::Advisory
    }));
    assert!(rows.iter().any(|row| {
        row.surface == "Migration compat lanes" && row.tier == SupportTierLevel::Advisory
    }));
    assert!(rows.iter().any(|row| {
        row.surface == "Self-hosting readiness" && row.tier == SupportTierLevel::Advisory
    }));
''',
    '''    assert_eq!(rows.len(), 14);
    for (surface, tier) in [
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
    ] {
        assert!(
            rows.iter()
                .any(|row| row.surface == surface && row.tier == tier),
            "missing support-tier row {surface} = {tier:?}"
        );
    }
''',
    "current support-tier row expectations",
)

text = path.read_text(encoding="utf-8")
text = text.replace(
    "| Worklist routing | Experimental | Worklists exist. | cargo-allow worklist --format json | Unknown tier. |",
    "| Worklist routing | Unsupported | Worklists exist. | cargo-allow worklist --format json | Unknown tier. |",
)
text = text.replace(
    'CargoAllowError::new("unknown support-tier level experimental")',
    'CargoAllowError::new("unknown support-tier level unsupported")',
)
text = text.replace(
    '"support-tier claims table with Surface, Tier, Claim, Proof command, and Notes columns not found"',
    '"support-tier claims table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes columns not found"',
)
path.write_text(text, encoding="utf-8")

replacement = r'''#[derive(serde::Deserialize)]
struct ThreeProductCollapseRow {
    target_module: String,
    disposition: String,
}

#[derive(serde::Deserialize)]
struct ThreeProductDispositionMap {
    schema_version: String,
    authority_generation: u32,
    design_package_proposal: String,
    ownership_adr: String,
    package_identity_adr: String,
    historical_spec: String,
    current_spec: String,
    design_package_plan: String,
    observed_package_count: usize,
    target_package_count: usize,
    repository_extraction_authorized: bool,
    release_authorized: bool,
    collapse: Vec<ThreeProductCollapseRow>,
}

#[test]
fn spec_system_design_package() -> Result<(), String> {
    let root = repo_root();
    let disposition_map = root.join("tests/fixtures/three-product-design/disposition-map.toml");
    if !disposition_map.is_file() {
        return Err(format!(
            "three-product reconstruction fixture missing: {}",
            disposition_map.display()
        ));
    }
    let disposition_text = std::fs::read_to_string(&disposition_map)
        .map_err(|error| format!("disposition map should be readable: {error}"))?;
    let disposition = toml::from_str::<ThreeProductDispositionMap>(&disposition_text)
        .map_err(|error| format!("disposition map should parse as TOML: {error}"))?;

    if disposition.schema_version != "2.0"
        || disposition.authority_generation != 2
        || disposition.design_package_proposal != "CARGO-ALLOW-PROP-0010"
        || disposition.ownership_adr != "CARGO-ALLOW-ADR-0002"
        || disposition.package_identity_adr != "CARGO-ALLOW-ADR-0003"
        || disposition.historical_spec != "CARGO-ALLOW-SPEC-0010"
        || disposition.current_spec != "CARGO-ALLOW-SPEC-0011"
        || disposition.design_package_plan != "CARGO-ALLOW-PLAN-0010"
        || disposition.observed_package_count != 27
        || disposition.target_package_count != 22
        || disposition.repository_extraction_authorized
        || disposition.release_authorized
    {
        return Err("generation-2 disposition authority fields do not match".to_string());
    }
    if disposition.collapse.len() != 5
        || disposition.collapse.iter().any(|row| {
            row.target_module.trim().is_empty() || row.disposition != "CollapseIntoPackage"
        })
    {
        return Err("generation-2 collapse projection is incomplete".to_string());
    }

    let ledger = parse_doc_artifact_ledger(include_str!(
        "../../../../.allow/artifacts/doc-artifacts.toml"
    ))
    .map_err(|error| error.to_string())?;
    for id in [
        "CARGO-ALLOW-PROP-0010",
        "CARGO-ALLOW-ADR-0002",
        "CARGO-ALLOW-ADR-0003",
        "CARGO-ALLOW-SPEC-0010",
        "CARGO-ALLOW-SPEC-0011",
        "CARGO-ALLOW-PLAN-0010",
    ] {
        if !ledger.artifact.iter().any(|artifact| artifact.id == id) {
            return Err(format!("artifact ledger missing {id}"));
        }
    }
    validate_doc_artifact_links(&ledger).map_err(|error| error.to_string())?;
    validate_doc_artifact_files(repo_root(), &ledger, &test_roots())
        .map_err(|error| error.to_string())?;
    Ok(())
}
'''
replace_tail(
    path,
    "#[derive(serde::Deserialize)]\nstruct ThreeProductDispositionEntry",
    replacement,
    "generation-1 phrase-based design-package tail",
)
