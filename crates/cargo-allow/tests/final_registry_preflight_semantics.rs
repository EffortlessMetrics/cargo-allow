use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[test]
fn final_registry_preflight_current_topology_uses_production_evaluator_and_schema()
-> Result<(), Box<dyn Error>> {
    use allow_report::{
        FinalRegistryContextV1, FinalRegistryObservationOriginV1, FinalRegistryObservationV1,
        FinalRegistryOwnerStateV1, FinalRegistryPreflightInputV1, FinalRegistryPreflightResultV1,
        FinalRegistryProvenanceV1, FinalRegistryPublishAuthorityV1, FinalRegistrySharedAuthorityV1,
        FinalRegistryVersionResponseV1, PackageCandidatePayloadV2,
        evaluate_final_registry_preflight_v1, final_registry_bindings_v1,
        render_final_registry_preflight_v1,
    };
    let root = repository_root()?;
    let topology: toml::Value = toml::from_str(&fs::read_to_string(
        root.join("policy/product-package-topology-v2.toml"),
    )?)?;
    let topology = serde_json::to_value(topology)?;
    let mut selected: Vec<_> = topology
        .get("package")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| io::Error::other("topology package table missing"))?
        .iter()
        .filter(|row| {
            row.get("candidate_inclusion")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        })
        .collect();
    selected.sort_by_key(|row| row.get("release_order").and_then(serde_json::Value::as_u64));
    let digest = format!("sha256:{:064x}", 1);
    let mut candidate_rows = Vec::new();
    let mut shared_authorities = Vec::new();
    let mut observations = Vec::new();
    let provenance = FinalRegistryProvenanceV1 {
        origin: FinalRegistryObservationOriginV1::TestFixture,
        provider: "current-topology-fixture".to_string(),
        source: "fixture://topology".to_string(),
        evidence_digest: digest.clone(),
        observed_at_unix_seconds: 100,
    };
    for row in selected {
        let field = |name: &str| -> Result<&str, io::Error> {
            row.get(name)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| io::Error::other(format!("topology field {name} missing")))
        };
        let logical = field("logical_id")?;
        let package = field("cargo_package_name")?;
        let version = field("package_version")?;
        let shared = field("product_family")? == "shared";
        candidate_rows.push(serde_json::json!({
            "logical_id": logical, "cargo_package_name": package, "cargo_package_version": version,
            "rust_library_name": logical.replace('-', "_"), "workspace_source_path": format!("crates/{logical}"),
            "product_family": if shared { "shared-0.1" } else { "cargo-allow-0.2" },
            "publication_state": field("publication_state")?, "publish": row.get("publish"),
            "support_tier": field("support_tier")?, "release_order": row.get("release_order"),
            "selected_features": [], "expected_manifest_identity": format!("{package}:{version}"),
            "expected_dependency_rows": [], "required_assets": [], "crate_digest": digest, "crate_size_bytes": 100,
        }));
        let version_response = if shared {
            let expected_checksum = field("expected_registry_checksum")?.to_string();
            shared_authorities.push(FinalRegistrySharedAuthorityV1 {
                package_name: package.to_string(),
                package_version: version.to_string(),
                expected_checksum: expected_checksum.clone(),
                authority_digest: digest.clone(),
            });
            // This is deliberately fixture provenance, never a registry observation.
            FinalRegistryVersionResponseV1::Found {
                checksum: expected_checksum,
                yanked: false,
            }
        } else {
            FinalRegistryVersionResponseV1::Missing {}
        };
        observations.push(FinalRegistryObservationV1 {
            package_name: package.to_string(),
            package_version: version.to_string(),
            version: version_response,
            version_provenance: Some(provenance.clone()),
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            owner_provenance: Some(provenance.clone()),
            publish_authority: FinalRegistryPublishAuthorityV1::NotProven,
            authority_provenance: Some(provenance.clone()),
        });
    }
    let candidate: PackageCandidatePayloadV2 = serde_json::from_value(serde_json::json!({
        "schema_id": "cargo-allow.package-candidate.v2", "schema_version": 2,
        "topology_id": topology.get("topology_id"), "repository_commit": "fixture", "repository_tree": "fixture",
        "cargo_lock_digest": digest, "candidate_product_id": "cargo-allow-0.2", "root_logical_id": "cargo-allow",
        "root_package_name": "cargo-allow", "root_package_version": "0.2.0", "target_class": "fixture", "feature_set_id": "default",
        "rows": candidate_rows, "known_exclusions": [], "limitations": ["fixture bytes and provider"], "claim_boundary": "topology integration only",
    }))?;
    let (candidate_digest, denominator_digest) =
        final_registry_bindings_v1(&candidate, &shared_authorities)?;
    let context = FinalRegistryContextV1 {
        candidate_digest,
        denominator_digest,
        workflow_digest: digest.clone(),
        principal: "fixture".to_string(),
        environment: "fixture".to_string(),
        owner_team_digest: digest.clone(),
        release_controls_digest: digest.clone(),
        provider_state_digest: digest,
    };
    let mut input = FinalRegistryPreflightInputV1 {
        schema_id: "cargo-allow.final-registry-preflight.v1".to_string(),
        schema_version: 1,
        candidate,
        shared_authorities,
        observations,
        observed_context: context.clone(),
        current_context: context,
        evaluated_at_unix_seconds: 105,
        maximum_age_seconds: 10,
    };
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("docs/schemas/cargo-allow.final-registry-preflight.v1.schema.json"),
    )?)?;
    let validator = jsonschema::validator_for(&schema)?;
    for expected in [
        FinalRegistryPreflightResultV1::CompleteWithResidualAuthorityRisk,
        FinalRegistryPreflightResultV1::Malformed,
    ] {
        let receipt = evaluate_final_registry_preflight_v1(&input);
        if receipt.result != expected {
            return Err(io::Error::other(format!("expected {expected:?}: {receipt:?}")).into());
        }
        let rendered: serde_json::Value =
            serde_json::from_str(&render_final_registry_preflight_v1(&receipt)?)?;
        if !validator.is_valid(&rendered) {
            return Err(io::Error::other(format!(
                "rendered result violates schema: {:?}",
                validator
                    .iter_errors(&rendered)
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
            ))
            .into());
        }
        input.observations.clear();
    }
    Ok(())
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}
