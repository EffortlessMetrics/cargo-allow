use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use allow_report::{
    CargoAllowReleaseExperienceV1, ReleaseExperienceBrownfieldPostureV1,
    ReleaseExperienceDocsIdentityV1, ReleaseExperienceFrictionDispositionV1,
    ReleaseExperienceFrictionV1, ReleaseExperienceInputV1, ReleaseExperienceResultV1,
    evaluate_release_experience_v1, render_release_experience_v1,
    RELEASE_EXPERIENCE_REQUIRED_DOCS, RELEASE_EXPERIENCE_SCHEMA_ID,
};
use ReleaseExperienceResultV1 as State;

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> Result<(), Box<dyn Error>> {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
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

fn input() -> ReleaseExperienceInputV1 {
    ReleaseExperienceInputV1 {
        schema_id: RELEASE_EXPERIENCE_SCHEMA_ID.to_string(),
        schema_version: 1,
        candidate_digest: digest(1),
        install_digest: digest(2),
        journey_digest: digest(3),
        binary_digest: digest(4),
        invocation_path: "/usr/local/bin/cargo-allow".to_string(),
        support_matrix_generation: "cargo-allow.support-matrix.v1".to_string(),
        command_registry_generation: "cargo-allow.command-registry.v1".to_string(),
        migration_denominator_digest: digest(5),
        migration_schema_id: "cargo-allow.migration.v1".to_string(),
        clean_pilot: None,
        brownfield_posture:
            ReleaseExperienceBrownfieldPostureV1::NotIncludedPendingPublishedPilot,
        brownfield_receipt_digest: None,
        docs_identities: RELEASE_EXPERIENCE_REQUIRED_DOCS
            .into_iter()
            .enumerate()
            .map(|(index, name)| ReleaseExperienceDocsIdentityV1 {
                name: name.to_string(),
                digest: digest(100 + index as u64),
            })
            .collect(),
        frictions: vec![ReleaseExperienceFrictionV1 {
            id: "friction-clean-pilot-absent".to_string(),
            disposition: ReleaseExperienceFrictionDispositionV1::AcceptedPostRelease,
            note: "Clean pilot awaits #3150 target selection.".to_string(),
        }],
        claimed_result: State::NotProven,
        not_proven_reason: "No clean external pilot ran for 0.2.0.".to_string(),
        narrowed_claims: vec![
            "installed truth holds for the exact candidate".to_string(),
            "no low-friction external adoption is claimed".to_string(),
        ],
        observed_at_unix_seconds: 100,
        evaluated_at_unix_seconds: 110,
        maximum_age_seconds: 30,
    }
}

fn receipt(input: &ReleaseExperienceInputV1) -> CargoAllowReleaseExperienceV1 {
    evaluate_release_experience_v1(input)
}

#[test]
fn release_experience_contract_notproven_without_pilot(
) -> Result<(), Box<dyn Error>> {
    let document = input();
    let compiled = receipt(&document);
    require(
        compiled.result == State::NotProven,
        format!("absent pilot must stay not-proven: {compiled:?}"),
    )?;
    require(
        compiled
            .retained_evidence
            .iter()
            .any(|evidence| evidence.contains("no low-friction external adoption")),
        "narrowed claims must travel visibly",
    )
}

#[test]
fn release_experience_contract_complete_claim_without_pilot_mismatches(
) -> Result<(), Box<dyn Error>> {
    let mut document = input();
    document.claimed_result = State::Complete;
    document.not_proven_reason.clear();
    document.narrowed_claims.clear();
    require(
        receipt(&document).result == State::Mismatch,
        "complete claim without pilot must mismatch",
    )
}

#[test]
fn release_experience_contract_rendered_receipt_matches_schema(
) -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("docs/schemas/cargo-allow.release-experience.v1.schema.json"),
    )?)?;
    require(
        schema
            .pointer("/properties/schema_id/const")
            .and_then(serde_json::Value::as_str)
            == Some(RELEASE_EXPERIENCE_SCHEMA_ID),
        "schema identity drifted from the production contract",
    )?;
    for field in [
        "schema_id",
        "schema_version",
        "result",
        "findings",
        "retained_evidence",
        "evaluated_at_unix_seconds",
    ] {
        let required = schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| io::Error::other("schema has no required fields"))?;
        require(
            required.iter().any(|entry| entry.as_str() == Some(field)),
            format!("schema dropped required field {field}"),
        )?;
    }
    let document = input();
    let rendered: serde_json::Value =
        serde_json::from_str(&render_release_experience_v1(&receipt(&document))?)?;
    require(
        rendered.get("result").and_then(serde_json::Value::as_str) == Some("not_proven"),
        "rendered receipt must carry the evaluated result",
    )
}
