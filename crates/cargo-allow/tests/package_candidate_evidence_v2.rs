//! Contract tests for the topology-selected package candidate (#2924).
//!
//! The committed example binds the exact V2 topology derivation of the
//! current tree; the tests pin its law (mixed-version identity, dependency
//! order, exclusion of sibling products) and its schema sync. Rust tests
//! never spawn Cargo: `.crate` bytes are the producer's domain.

use allow_report::{
    PACKAGE_CANDIDATE_V2_SCHEMA_ID, PACKAGE_CANDIDATE_V2_SCHEMA_VERSION,
    PackageCandidateDependencyKindV2, PackageCandidateFamilyV2, PackageCandidatePayloadV2,
    PackageCandidateResultV2, render_package_candidate_v2_bytes, validate_package_candidate_v2,
};
use intent_model::parse_package_postures_v1;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example_payload() -> Result<PackageCandidatePayloadV2, String> {
    let text = std::fs::read_to_string(
        repo_root().join("docs/dogfood/receipts/package-candidate-v2.example.json"),
    )
    .map_err(|err| format!("read example candidate: {err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("parse example candidate: {err}"))
}

fn derived_candidate_names() -> Result<Vec<String>, String> {
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(repo_root().join(rel)).map_err(|err| format!("read {rel}: {err}"))
    };
    let postures = parse_package_postures_v1(&read("policy/product-package-topology-v2.toml")?)
        .map_err(|err| format!("parse topology: {err}"))?;
    let mut names: Vec<String> = postures
        .iter()
        .filter(|p| {
            p.membership.candidate_inclusion
                && (p.version_line == "cargo-allow-0.2" || p.version_line == "shared-0.1")
        })
        .map(|p| p.cargo_package_name.clone())
        .collect();
    names.sort();
    Ok(names)
}

#[test]
fn example_candidate_is_complete_and_mixed_version() -> Result<(), String> {
    let payload = example_payload()?;
    let validation = validate_package_candidate_v2(&payload);
    if validation.result != PackageCandidateResultV2::Complete {
        return Err(format!("committed example is not Complete: {validation:?}"));
    }
    let families: std::collections::BTreeSet<_> =
        payload.rows.iter().map(|row| row.product_family).collect();
    if families
        != std::collections::BTreeSet::from([
            PackageCandidateFamilyV2::CargoAllow02,
            PackageCandidateFamilyV2::Shared01,
        ])
    {
        return Err(format!("example families drifted: {families:?}"));
    }
    let cargo_allow_family = payload
        .rows
        .iter()
        .filter(|row| row.product_family == PackageCandidateFamilyV2::CargoAllow02)
        .map(|row| row.cargo_package_version.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let shared_family = payload
        .rows
        .iter()
        .filter(|row| row.product_family == PackageCandidateFamilyV2::Shared01)
        .map(|row| row.cargo_package_version.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if cargo_allow_family != std::collections::BTreeSet::from(["0.2.0-rc.1"])
        || shared_family != std::collections::BTreeSet::from(["0.1.0"])
    {
        return Err(format!(
            "mixed-version identity drifted: {cargo_allow_family:?} / {shared_family:?}"
        ));
    }
    Ok(())
}

#[test]
fn example_candidate_matches_the_live_topology_derivation() -> Result<(), String> {
    let payload = example_payload()?;
    let mut example_names: Vec<String> = payload
        .rows
        .iter()
        .map(|row| row.cargo_package_name.clone())
        .collect();
    example_names.sort();
    if example_names != derived_candidate_names()? {
        return Err(format!(
            "example rows {example_names:?} disagree with the topology derivation"
        ));
    }
    for row in &payload.rows {
        if row.expected_manifest_identity
            != format!("{}:{}", row.cargo_package_name, row.cargo_package_version)
        {
            return Err(format!("row {} identity drifted", row.logical_id));
        }
    }
    Ok(())
}

#[test]
fn candidate_missing_a_required_row_is_rejected() -> Result<(), String> {
    let mut payload = example_payload()?;
    let expected = derived_candidate_names()?;
    payload
        .rows
        .retain(|row| row.cargo_package_name != expected[0]);
    let validation = validate_package_candidate_v2(&payload);
    if validation.result == PackageCandidateResultV2::Complete {
        return Err("candidate missing a topology row validated".to_string());
    }
    if !derived_candidate_names()?.is_empty() {
        // The structural validator cannot know the count; the producer and
        // the live-derivation test own set equality. Assert the mutation is
        // detectable against the derivation instead.
        let mut example_names: Vec<String> = payload
            .rows
            .iter()
            .map(|row| row.cargo_package_name.clone())
            .collect();
        example_names.sort();
        if example_names == expected {
            return Err("row removal was not detectable against the derivation".to_string());
        }
    }
    Ok(())
}

#[test]
fn candidate_extra_sibling_row_is_rejected() -> Result<(), String> {
    let mut payload = example_payload()?;
    let mut sibling = payload.rows.first().cloned().ok_or("empty example")?;
    sibling.logical_id = "intent-model".to_string();
    sibling.cargo_package_name = "intent-model".to_string();
    sibling.cargo_package_version = "0.1.0".to_string();
    sibling.product_family = PackageCandidateFamilyV2::Shared01;
    sibling.expected_manifest_identity = "intent-model:0.1.0".to_string();
    payload.rows.push(sibling);
    let validation = validate_package_candidate_v2(&payload);
    if validation.result != PackageCandidateResultV2::DependencyConflict {
        return Err(format!(
            "sibling row insertion was not classified: {validation:?}"
        ));
    }
    Ok(())
}

#[test]
fn candidate_wrong_shared_version_is_rejected() -> Result<(), String> {
    let mut payload = example_payload()?;
    let shared = payload
        .rows
        .iter_mut()
        .find(|row| row.product_family == PackageCandidateFamilyV2::Shared01)
        .ok_or("example lost its shared row")?;
    shared.cargo_package_version = "0.2.0-rc.1".to_string();
    shared.expected_manifest_identity = format!(
        "{}:{}",
        shared.cargo_package_name, shared.cargo_package_version
    );
    let validation = validate_package_candidate_v2(&payload);
    if validation.result != PackageCandidateResultV2::IdentityConflict
        || !validation
            .gaps
            .iter()
            .any(|gap| gap.contains("mixed versions"))
    {
        return Err(format!(
            "shared-version substitution was not classified: {validation:?}"
        ));
    }
    Ok(())
}

#[test]
fn candidate_packaged_dependency_expectations_stay_internal_closed() -> Result<(), String> {
    let payload = example_payload()?;
    let names: std::collections::BTreeSet<_> = payload
        .rows
        .iter()
        .map(|row| row.cargo_package_name.as_str())
        .collect();
    for row in &payload.rows {
        for dependency in &row.expected_dependency_rows {
            if dependency.dependency_kind == PackageCandidateDependencyKindV2::Internal
                && !names.contains(dependency.package_name.as_str())
            {
                return Err(format!(
                    "internal dependency {} is not a candidate row",
                    dependency.package_name
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn candidate_rendering_is_deterministic_and_schema_synced() -> Result<(), String> {
    let payload = example_payload()?;
    let first = render_package_candidate_v2_bytes(&payload).map_err(|error| error.to_string())?;
    let second = render_package_candidate_v2_bytes(&payload).map_err(|error| error.to_string())?;
    if first != second {
        return Err("identical payloads rendered different bytes".to_string());
    }

    let schema: serde_json::Value = serde_json::from_str(::std::include_str!(
        "../../../docs/schemas/package-candidate-v2.schema.json"
    ))
    .map_err(|error| error.to_string())?;
    if schema.pointer("/$defs/product_family/enum")
        != Some(&serde_json::Value::Array(vec![
            serde_json::Value::String("cargo-allow-0.2".to_string()),
            serde_json::Value::String("shared-0.1".to_string()),
        ]))
    {
        return Err("schema family enum is out of sync with the Rust contract".to_string());
    }
    if schema.pointer("/properties/schema_id/const")
        != Some(&serde_json::Value::String(
            PACKAGE_CANDIDATE_V2_SCHEMA_ID.to_string(),
        ))
    {
        return Err("schema id is out of sync with the Rust contract".to_string());
    }
    if schema.pointer("/properties/schema_version/const")
        != Some(&serde_json::Value::from(
            PACKAGE_CANDIDATE_V2_SCHEMA_VERSION,
        ))
    {
        return Err("schema version is out of sync with the Rust contract".to_string());
    }
    Ok(())
}
