//! Full-payload golden for the typed ReleaseManifestV2 final-release law
//! (#3761).
//!
//! The fixture is the final `0.2.0` identity in its production shape: ten
//! uniform cargo-allow candidate rows (the #3744 shared `0.1.0` prerequisites
//! live in the shared preflight, not in this manifest), every row reconciled
//! through the typed publication adapter so its `crate_digest` is the
//! proven-equal expected/observed checksum, and producer evidence retained per
//! consumed receipt. The golden pins the payload content, its canonical serde
//! byte rendering, and the fact that the envelope's volatile metadata never
//! enters those bytes.

use allow_report::{
    PackageRowClassV1, PublicationStateV1, RELEASE_MANIFEST_V2_SCHEMA_ID,
    RELEASE_MANIFEST_V2_SCHEMA_VERSION, ReconciledPackagePublicationV1, ReleaseManifestEnvelopeV2,
    ReleaseManifestResultV2, manifest_rows_from_reconciled,
    render_release_manifest_v2_payload_bytes, validate_release_manifest_v2,
};
use std::error::Error;

/// The final candidate closure in dependency order (#2277 candidate-crate-set,
/// cargo-allow packages only). Digests are deterministic fixture values.
const FINAL_ROWS: [(&str, u32, &str); 10] = [
    (
        "allow-core",
        10,
        "sha256:62b99437bc93528f3da331c6cafa654ab669faaf76a15ec590c680ffdf6e6275",
    ),
    (
        "allow-policy",
        20,
        "sha256:74f46a15622f26108c97923cd8b9fe46234412ab24a2da388c515dfa49583684",
    ),
    (
        "allow-inventory",
        30,
        "sha256:d3e668bf3e78734654aba4da919a7881c78b3465db46db0a06d86820293a2cfe",
    ),
    (
        "allow-files",
        40,
        "sha256:269d4a06282f9ef5ebc6d234999c785c3f0670cb6649193b1117dd9a503e8dc4",
    ),
    (
        "allow-rust",
        50,
        "sha256:6cfb93c0eafcda2b21234b15f18446d25360e7fd9d0ec568f10aef7657a89733",
    ),
    (
        "allow-match",
        60,
        "sha256:74a3acff8819928c7318e7a73b2c116810fe9dca75d5fa9dd4df25c6710c5997",
    ),
    (
        "allow-report",
        70,
        "sha256:d57fc76161b8dee498e10b09416f9f44f297b4831674f0c9c3e6a4c8f08629a7",
    ),
    (
        "allow-policy-legacy",
        80,
        "sha256:84f47a7267691a260f715523eec39c701fe5d985fc1abca175a33f2792896b57",
    ),
    (
        "allow-diff",
        90,
        "sha256:7c83ab9d7152b0bf030ffc4940794cf53c58db47c167dc74ea07e79a611bc396",
    ),
    (
        "cargo-allow",
        100,
        "sha256:e11aa3742785b2669f1311d89726fbf85bb395a464c048584e13198d16271e0f",
    ),
];

/// The golden envelope fixture: exactly the production generator's shape for a
/// published final release, with one consumed-evidence row per consumed
/// receipt naming its schema identity, canonical digest, producer, and result
/// class (the #3761 evidence-reference law).
const GOLDEN_ENVELOPE_JSON: &str = r#"{
  "payload": {
    "schema_id": "cargo-allow.release-manifest.v2",
    "schema_version": 2,
    "operation": "cargo_allow_release",
    "repository": "EffortlessMetrics/cargo-allow",
    "tag_or_authorization": "v0.2.0",
    "commit": "5f2e1a94c07db381e6a94c02f7d1b3a894e02c16",
    "tree": "9d41c7ea52b0f3861a4d7c95e08b26f13a7d0e45",
    "cargo_lock_digest": "sha256:1f933d252903251a500336021a5295b197e95d2b6f886584189b96b5827035c6",
    "architecture_digest": "sha256:11bba6717b6a4cb9d26e4fab6c4396b67a84af2f1c79265cc2ffaba78ced3425",
    "candidate_digest": "sha256:7e493dfbef9a9c92cfa42ad73f0de6ceed1c59b63ec48a765e768e429dd7e898",
    "package_rows": [
      {
        "logical_id": "allow-core",
        "package_name": "allow-core",
        "package_version": "0.2.0",
        "release_order": 10,
        "crate_digest": "sha256:62b99437bc93528f3da331c6cafa654ab669faaf76a15ec590c680ffdf6e6275",
        "registry_checksum": "sha256:62b99437bc93528f3da331c6cafa654ab669faaf76a15ec590c680ffdf6e6275"
      },
      {
        "logical_id": "allow-policy",
        "package_name": "allow-policy",
        "package_version": "0.2.0",
        "release_order": 20,
        "crate_digest": "sha256:74f46a15622f26108c97923cd8b9fe46234412ab24a2da388c515dfa49583684",
        "registry_checksum": "sha256:74f46a15622f26108c97923cd8b9fe46234412ab24a2da388c515dfa49583684"
      },
      {
        "logical_id": "allow-inventory",
        "package_name": "allow-inventory",
        "package_version": "0.2.0",
        "release_order": 30,
        "crate_digest": "sha256:d3e668bf3e78734654aba4da919a7881c78b3465db46db0a06d86820293a2cfe",
        "registry_checksum": "sha256:d3e668bf3e78734654aba4da919a7881c78b3465db46db0a06d86820293a2cfe"
      },
      {
        "logical_id": "allow-files",
        "package_name": "allow-files",
        "package_version": "0.2.0",
        "release_order": 40,
        "crate_digest": "sha256:269d4a06282f9ef5ebc6d234999c785c3f0670cb6649193b1117dd9a503e8dc4",
        "registry_checksum": "sha256:269d4a06282f9ef5ebc6d234999c785c3f0670cb6649193b1117dd9a503e8dc4"
      },
      {
        "logical_id": "allow-rust",
        "package_name": "allow-rust",
        "package_version": "0.2.0",
        "release_order": 50,
        "crate_digest": "sha256:6cfb93c0eafcda2b21234b15f18446d25360e7fd9d0ec568f10aef7657a89733",
        "registry_checksum": "sha256:6cfb93c0eafcda2b21234b15f18446d25360e7fd9d0ec568f10aef7657a89733"
      },
      {
        "logical_id": "allow-match",
        "package_name": "allow-match",
        "package_version": "0.2.0",
        "release_order": 60,
        "crate_digest": "sha256:74a3acff8819928c7318e7a73b2c116810fe9dca75d5fa9dd4df25c6710c5997",
        "registry_checksum": "sha256:74a3acff8819928c7318e7a73b2c116810fe9dca75d5fa9dd4df25c6710c5997"
      },
      {
        "logical_id": "allow-report",
        "package_name": "allow-report",
        "package_version": "0.2.0",
        "release_order": 70,
        "crate_digest": "sha256:d57fc76161b8dee498e10b09416f9f44f297b4831674f0c9c3e6a4c8f08629a7",
        "registry_checksum": "sha256:d57fc76161b8dee498e10b09416f9f44f297b4831674f0c9c3e6a4c8f08629a7"
      },
      {
        "logical_id": "allow-policy-legacy",
        "package_name": "allow-policy-legacy",
        "package_version": "0.2.0",
        "release_order": 80,
        "crate_digest": "sha256:84f47a7267691a260f715523eec39c701fe5d985fc1abca175a33f2792896b57",
        "registry_checksum": "sha256:84f47a7267691a260f715523eec39c701fe5d985fc1abca175a33f2792896b57"
      },
      {
        "logical_id": "allow-diff",
        "package_name": "allow-diff",
        "package_version": "0.2.0",
        "release_order": 90,
        "crate_digest": "sha256:7c83ab9d7152b0bf030ffc4940794cf53c58db47c167dc74ea07e79a611bc396",
        "registry_checksum": "sha256:7c83ab9d7152b0bf030ffc4940794cf53c58db47c167dc74ea07e79a611bc396"
      },
      {
        "logical_id": "cargo-allow",
        "package_name": "cargo-allow",
        "package_version": "0.2.0",
        "release_order": 100,
        "crate_digest": "sha256:e11aa3742785b2669f1311d89726fbf85bb395a464c048584e13198d16271e0f",
        "registry_checksum": "sha256:e11aa3742785b2669f1311d89726fbf85bb395a464c048584e13198d16271e0f"
      }
    ],
    "authentication": "crates_io_api_token",
    "publication_posture": "published",
    "support_posture": "experimental",
    "limitations": [
      "candidate rows come from the exact topology publisher receipt",
      "binary evidence is limited to the referenced receipt and verified target"
    ],
    "claim_boundary": "topology-derived package identity and release evidence only; this manifest does not authorize publication or claim source, runtime, platform, or support completeness",
    "consumed_evidence": [
      {
        "schema_id": "cargo-allow.topology-publish-receipt.v1",
        "path": "target/cargo-allow/topology-publish.receipt.json",
        "sha256": "sha256:7e493dfbef9a9c92cfa42ad73f0de6ceed1c59b63ec48a765e768e429dd7e898",
        "producer": "scripts/release-topology-publisher.py",
        "result_class": "complete"
      },
      {
        "schema_id": "cargo-allow.release-binary-package.v1",
        "path": "artifact/release-binary.receipt.json",
        "sha256": "sha256:d4aa8d15166ebd4c3d818f5055787b7c7cc311a6bc3dc09486a97053d31da0d0",
        "producer": "scripts/package-release-binary.sh",
        "result_class": "verified"
      },
      {
        "schema_id": "cargo-allow.release-binary-install.v1",
        "path": "artifact/release-binary-install.receipt.json",
        "sha256": "sha256:81ca3807829d5e02927066227476e6213c4734540a682aea4f1f849cc183dc81",
        "producer": "scripts/verify-release-binary.sh",
        "result_class": "verified"
      }
    ]
  },
  "generated_at": "2026-11-03T00:00:00Z",
  "workflow_path": ".github/workflows/release.yml",
  "workflow_run_id": 32770000001,
  "workflow_attempt": 1,
  "event": "release",
  "github_ref": "refs/tags/v0.2.0",
  "artifact_references": [
    "target/cargo-allow/topology-publish.receipt.json",
    "artifact/release-binary.receipt.json",
    "artifact/release-binary-install.receipt.json"
  ],
  "authorization_reference": "issue:3760",
  "instrument_diagnostics": []
}"#;

/// The reconciled source rows behind the golden payload: one cargo-allow
/// candidate row per package whose expected authority is the candidate's own
/// frozen `.crate` bytes and whose observed registry checksum proved equal.
fn reconciled_final_rows() -> Vec<ReconciledPackagePublicationV1> {
    FINAL_ROWS
        .iter()
        .map(|(name, order, digest)| ReconciledPackagePublicationV1 {
            logical_id: (*name).to_string(),
            package_name: (*name).to_string(),
            package_version: "0.2.0".to_string(),
            release_order: *order,
            row_class: PackageRowClassV1::CargoAllowCandidate,
            state: PublicationStateV1::PublishedVerified,
            expected_checksum: (*digest).to_string(),
            observed_registry_checksum: Some((*digest).to_string()),
        })
        .collect()
}

#[test]
fn final_golden_parses_validates_and_round_trips() -> Result<(), Box<dyn Error>> {
    let envelope: ReleaseManifestEnvelopeV2 =
        serde_json::from_str(GOLDEN_ENVELOPE_JSON).map_err(|error| {
            format!("golden envelope must parse against the typed contract: {error}")
        })?;

    let validation = validate_release_manifest_v2(&envelope);
    if validation.result != ReleaseManifestResultV2::Complete || !validation.gaps.is_empty() {
        return Err(format!(
            "the final golden envelope must validate Complete with no gaps: {validation:?}"
        )
        .into());
    }
    if envelope.payload.schema_id != RELEASE_MANIFEST_V2_SCHEMA_ID
        || envelope.payload.schema_version != RELEASE_MANIFEST_V2_SCHEMA_VERSION
    {
        return Err("golden payload must carry the current manifest generation".into());
    }

    // The golden rows are exactly the typed reconciliation projection: the
    // manifest row exists only after the equality proof, and it binds
    // crate_digest to the proven-equal checksum.
    let projected = manifest_rows_from_reconciled(&reconciled_final_rows())
        .map_err(|error| format!("every final golden row must reconcile exact: {error}"))?;
    if projected != envelope.payload.package_rows {
        return Err(format!(
            "typed projection diverged from the golden package rows: {projected:?} vs {:?}",
            envelope.payload.package_rows
        )
        .into());
    }
    for row in &projected {
        let expected = FINAL_ROWS
            .iter()
            .find(|(name, _, _)| *name == row.package_name)
            .map(|(_, _, digest)| *digest)
            .ok_or_else(|| format!("unexpected golden row {}", row.package_name))?;
        if row.crate_digest.as_deref() != Some(expected)
            || row.registry_checksum.as_deref() != Some(expected)
        {
            return Err(format!(
                "{}: crate_digest and registry_checksum must both be the proven-equal checksum",
                row.package_name
            )
            .into());
        }
    }

    // Canonical payload bytes: stable across re-renders and immune to the
    // envelope's volatile metadata.
    let first = render_release_manifest_v2_payload_bytes(&envelope.payload)?;
    let mut volatile = envelope.clone();
    volatile.generated_at = "2027-01-01T00:00:00Z".to_string();
    volatile.workflow_run_id = Some(99);
    let second = render_release_manifest_v2_payload_bytes(&volatile.payload)?;
    if first != second {
        return Err("volatile envelope metadata changed the canonical payload bytes".into());
    }
    if first.is_empty() {
        return Err("canonical payload rendering was empty".into());
    }
    Ok(())
}

#[test]
fn golden_cannot_be_rebuilt_from_unproven_bytes() -> Result<(), Box<dyn Error>> {
    // Flipping any row's observed registry checksum to different valid bytes
    // must refuse the whole batch: a golden payload is only reproducible from
    // equality-proven publications.
    let mut tampered = reconciled_final_rows();
    let last = tampered
        .last_mut()
        .ok_or_else(|| "fixture lost its final row".to_string())?;
    last.observed_registry_checksum =
        Some("sha256:212dfcc89d7ce2170e2cd7507231cbbf75e2a43e987f6ef0691f3ad222168143".to_string());
    let refused = match manifest_rows_from_reconciled(&tampered) {
        Ok(rows) => {
            return Err(format!(
                "tampered batch must refuse manifest construction, got {} rows",
                rows.len()
            )
            .into());
        }
        Err(error) => error,
    };
    if !refused.contains("cargo-allow") || !refused.contains("checksum_conflict") {
        return Err(format!(
            "tampered final row must be refused as a checksum conflict, naming the row: {refused}"
        )
        .into());
    }
    Ok(())
}
