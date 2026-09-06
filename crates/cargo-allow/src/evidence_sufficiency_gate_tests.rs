//! Evidence-sufficiency-gate tests for the #3846 campaign issue
//! closeout guard: a Complete closeout must name evidence-surface
//! inventory ids that exist, and at least one must carry a sufficient
//! evidence class; lexical and projection-only classes never satisfy a
//! production, external, or live-control acceptance row (#3810 / #3842).

use serde::Deserialize;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel))
        .expect("the evidence-surface inventory is retained in the tree")
}

const INVENTORY_SCHEMA: &str = "cargo-allow.evidence-surface-inventory.v1";

/// Evidence classes that name real evidence strength; anything else —
/// including classes unknown to the gate — is insufficient.
const SUFFICIENT_EVIDENCE_CLASSES: [&str; 5] = [
    "StructuredShapeValidation",
    "TypedModelValidation",
    "ProductionBehaviorValidation",
    "ExternalObservationValidation",
    "LiveControlReadback",
];

#[derive(Debug, Deserialize)]
struct EvidenceSurface {
    id: String,
    evidence_class: String,
}

#[derive(Debug, Deserialize)]
struct EvidenceSurfaceInventory {
    schema: String,
    #[serde(default)]
    surfaces: Vec<EvidenceSurface>,
}

fn validate_inventory(inventory: &EvidenceSurfaceInventory) -> Vec<String> {
    let mut failures = Vec::new();
    if inventory.schema != INVENTORY_SCHEMA {
        failures.push("inventory_schema_mismatch".to_string());
    }
    let mut seen: Vec<&str> = Vec::new();
    for surface in &inventory.surfaces {
        if seen.contains(&surface.id.as_str()) {
            failures.push(format!("duplicate_surface: {}", surface.id));
        } else {
            seen.push(&surface.id);
        }
        if surface.evidence_class.trim().is_empty() {
            failures.push(format!("invalid_class: {}", surface.id));
        }
    }
    failures
}

/// The gate: `declared` are the surface ids a closeout claims as
/// acceptance backing. Bounded rejection codes match the runtime guard.
fn gate_acceptance_evidence(
    declared: &[String],
    inventory: &EvidenceSurfaceInventory,
) -> Vec<String> {
    let mut codes = Vec::new();
    if declared.is_empty() {
        return vec!["evidence_surfaces_missing".to_string()];
    }
    if declared.iter().any(|item| item.trim().is_empty())
        || declared.len()
            != declared
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
    {
        codes.push("evidence_surfaces_invalid".to_string());
        return codes;
    }
    if declared
        .iter()
        .any(|item| !inventory.surfaces.iter().any(|surface| &surface.id == item))
    {
        return vec!["evidence_surface_unknown".to_string()];
    }
    let classes: std::collections::BTreeSet<&str> = declared
        .iter()
        .map(|item| {
            inventory
                .surfaces
                .iter()
                .find(|surface| &surface.id == item)
                .map(|surface| surface.evidence_class.as_str())
                .unwrap_or_default()
        })
        .collect();
    if !classes
        .iter()
        .any(|class| SUFFICIENT_EVIDENCE_CLASSES.contains(class))
    {
        codes.push("insufficient_evidence_class".to_string());
    }
    codes
}

fn inventory() -> EvidenceSurfaceInventory {
    let root = workspace_root();
    let text = read_workspace_file(&root, "policy/evidence-surface-inventory.toml");
    toml::from_str(&text).expect("the checked inventory parses")
}

#[test]
fn evidence_sufficiency_gate_loads_the_checked_inventory() {
    let inventory = inventory();
    assert_eq!(inventory.schema, INVENTORY_SCHEMA);
    assert!(
        inventory.surfaces.len() >= 20,
        "the evidence-surface inventory is explicit and non-empty"
    );
    let validation = validate_inventory(&inventory);
    assert!(
        validation.is_empty(),
        "the checked inventory is coherent: {validation:?}"
    );
}

#[test]
fn evidence_sufficiency_gate_rejects_missing_or_malformed_declarations() {
    let inventory = inventory();
    assert_eq!(
        gate_acceptance_evidence(&[], &inventory),
        vec!["evidence_surfaces_missing".to_string()]
    );
    let duplicated = vec!["a".to_string(), "a".to_string()];
    assert_eq!(
        gate_acceptance_evidence(&duplicated, &inventory),
        vec!["evidence_surfaces_invalid".to_string()]
    );
    let unknown = vec!["no-such-surface".to_string()];
    assert_eq!(
        gate_acceptance_evidence(&unknown, &inventory),
        vec!["evidence_surface_unknown".to_string()]
    );
}

#[test]
fn evidence_sufficiency_gate_rejects_projection_only_backing() {
    let inventory = inventory();
    // Negative control 4: a lexical/projection surface alone cannot
    // back a Complete closeout. The campaign-skill surface is the
    // checked LexicalProjectionOnly row.
    let lexical_only = vec!["campaign-skill-contract".to_string()];
    assert_eq!(
        gate_acceptance_evidence(&lexical_only, &inventory),
        vec!["insufficient_evidence_class".to_string()]
    );
    // Unknown classes cannot be assumed to prove the named authority.
    let unknown_class = EvidenceSurfaceInventory {
        schema: INVENTORY_SCHEMA.to_string(),
        surfaces: vec![EvidenceSurface {
            id: "hostile-surface".to_string(),
            evidence_class: "AuthorAssertionValidation".to_string(),
        }],
    };
    let declared = vec!["hostile-surface".to_string()];
    assert_eq!(
        gate_acceptance_evidence(&declared, &unknown_class),
        vec!["insufficient_evidence_class".to_string()]
    );
}

#[test]
fn evidence_sufficiency_gate_accepts_structured_or_stronger_backing() {
    let inventory = inventory();
    let sufficient: Vec<String> = inventory
        .surfaces
        .iter()
        .find(|surface| SUFFICIENT_EVIDENCE_CLASSES.contains(&surface.evidence_class.as_str()))
        .map(|surface| vec![surface.id.clone()])
        .expect("the checked inventory retains at least one sufficient surface");
    assert!(gate_acceptance_evidence(&sufficient, &inventory).is_empty());
}

#[test]
fn evidence_sufficiency_gate_binds_the_runtime_guard() {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/verify-campaign-issue-closeout.py");
    for code in [
        "evidence_surfaces_missing",
        "evidence_surfaces_invalid",
        "evidence_surface_unknown",
        "insufficient_evidence_class",
        "LiveControlReadback",
        "ProductionBehaviorValidation",
    ] {
        assert!(
            script.contains(code),
            "the runtime guard carries the gate code/class '{code}'"
        );
    }
    assert!(
        script.contains("policy/evidence-surface-inventory.toml"),
        "the runtime guard consumes the checked inventory"
    );
}
