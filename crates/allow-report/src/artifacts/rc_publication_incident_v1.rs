//! Typed reconciliation record for the moved `v0.2.0-rc.1` publication
//! incident (#3759).
//!
//! The record reconciles the entire rc.1 attempt sequence against retained
//! evidence and current registry state. Its law, from the incident ruling:
//! later green completions do not erase earlier partial attempts, name and
//! version visibility is not candidate identity, and rows published from
//! different source candidates stay visibly different. The channel posture
//! and final-candidate eligibility are fixed by the ruling and are not
//! free fields.

use serde::Deserialize;

pub const RC_PUBLICATION_INCIDENT_SCHEMA: &str = "cargo-allow.rc-publication-incident.v1";

/// How completely the incident sequence was observed. `PartialObservation`
/// must carry at least one explicit observation limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationCompletenessV1 {
    CompleteObservation,
    PartialObservation,
    ProviderUnavailable,
    InstrumentFailure,
}

/// Fixed by the incident ruling: rc.1 is publicly installable but carries
/// its publication-incident lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelPostureV1 {
    PublicPrereleaseWithIncident,
}

/// Fixed by the incident ruling: rc.1 evidence may not be reused as the
/// final 0.2.0 identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalCandidateEligibilityV1 {
    NotReusable,
}

/// One provider-observed job inside a release attempt. A failed job must
/// name its first failing step; a non-failed job must not.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptJobObservationV1 {
    pub name: String,
    pub conclusion: String,
    pub first_failing_step: Option<String>,
}

/// One provider-observed inner file of a retained workflow artifact.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactInnerFileV1 {
    pub name: String,
    pub size_in_bytes: u64,
    pub sha256: String,
}

/// One provider-observed workflow artifact of a release attempt. When the
/// inner file set was not observed in this reconciliation, the absence must
/// be explicit in `observation_note`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptArtifactObservationV1 {
    pub artifact_id: u64,
    pub name: String,
    pub size_in_bytes: u64,
    pub expired: bool,
    pub expires_at: String,
    pub inner_files: Vec<ArtifactInnerFileV1>,
    pub observation_note: Option<String>,
}

/// One observed release attempt over the incident tag, bound to the exact
/// provider observation (run attempt, job/step conclusions, retained
/// artifacts). `retained_publish_receipt_artifact_id` is `None` when the
/// run's publish job never produced a receipt; a run with a skipped publish
/// job must never be classified as a package publication.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAttemptV1 {
    pub run_id: u64,
    pub run_attempt: u32,
    pub commit: String,
    pub tree: String,
    pub conclusion: String,
    pub result_class: String,
    pub retained_publish_receipt_artifact_id: Option<u64>,
    pub jobs: Vec<AttemptJobObservationV1>,
    pub artifacts: Vec<AttemptArtifactObservationV1>,
    pub observation_limit: Option<String>,
}

/// One provider-observed GitHub Release asset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAssetObservationV1 {
    pub asset_id: u64,
    pub name: String,
    pub size_in_bytes: u64,
    pub digest: String,
    pub state: String,
}

/// One exact install observation bound to a retained install-smoke receipt.
/// The receipt carries identity fields only, so the observation must say so
/// rather than implying an independently verified installation result.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallObservationV1 {
    pub platform: String,
    pub run_id: u64,
    pub receipt_artifact_id: u64,
    pub receipt_inner_sha256: String,
    pub observed_note: String,
}

/// Observed state of the incident tag. Event-level create/delete/recreate
/// history requires retained audit evidence; when absent, the limit must
/// be explicit and the current target recorded.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagObservationV1 {
    pub current_tag_object: String,
    pub current_target_commit: String,
    pub events: Vec<String>,
    pub observation_limit: Option<String>,
}

/// Per-row reconciliation over the candidate set. Each state carries
/// exactly the evidence its classification requires; a row whose registry
/// bytes match no retained candidate stays visibly across-candidate.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RowReconciliationV1 {
    PublishedExactCandidate {
        package: String,
        version: String,
        candidate_commit: String,
        candidate_tree: String,
        candidate_receipt_artifact_id: u64,
        expected_checksum: String,
        observed_checksum: String,
    },
    PublishedAcrossCandidateHistory {
        package: String,
        version: String,
        registry_checksum: String,
        retained_candidate_checksums: Vec<String>,
        note: String,
    },
    PublishedButCandidateEvidenceUnavailable {
        package: String,
        version: String,
        registry_checksum: String,
        note: String,
    },
    Missing {
        package: String,
        version: String,
    },
    Yanked {
        package: String,
        version: String,
        registry_checksum: String,
    },
    ChecksumConflict {
        package: String,
        version: String,
        expected_checksum: String,
        observed_checksum: String,
    },
    ProviderUnavailable {
        package: String,
    },
    InstrumentFailure {
        package: String,
        note: String,
    },
}

impl RowReconciliationV1 {
    pub fn package(&self) -> &str {
        match self {
            Self::PublishedExactCandidate { package, .. }
            | Self::PublishedAcrossCandidateHistory { package, .. }
            | Self::PublishedButCandidateEvidenceUnavailable { package, .. }
            | Self::Missing { package, .. }
            | Self::Yanked { package, .. }
            | Self::ChecksumConflict { package, .. }
            | Self::ProviderUnavailable { package }
            | Self::InstrumentFailure { package, .. } => package,
        }
    }
}

/// Observed GitHub Release state for the incident tag. Deviations (for
/// example an RC published with a stable release posture) are recorded as
/// observations, never normalized away. Assets are observed by provider
/// asset ID with their provider digests.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubReleaseObservationV1 {
    pub release_id: u64,
    pub name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub target_commitish: String,
    pub asset_count: u32,
    pub assets: Vec<ReleaseAssetObservationV1>,
    pub observed_note: String,
}

/// Current registry observation covering the candidate set.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryObservationV1 {
    pub observed_at: String,
    pub rows_present: u32,
    pub yanked_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RcPublicationIncidentV1 {
    pub schema: String,
    pub incident_id: String,
    pub channel_version: String,
    pub channel_tag: String,
    pub attempts: Vec<ReleaseAttemptV1>,
    pub tag_observation: TagObservationV1,
    pub row_reconciliations: Vec<RowReconciliationV1>,
    pub github_release: GitHubReleaseObservationV1,
    pub install_observations: Vec<InstallObservationV1>,
    pub registry_observation: RegistryObservationV1,
    pub observation_completeness: ObservationCompletenessV1,
    pub observation_limits: Vec<String>,
    pub channel_posture: ChannelPostureV1,
    pub final_candidate_eligibility: FinalCandidateEligibilityV1,
    pub claim_boundary: String,
}

impl RcPublicationIncidentV1 {
    /// Fail-honest validation of the reconciliation law.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != RC_PUBLICATION_INCIDENT_SCHEMA {
            return Err(format!(
                "schema must be {RC_PUBLICATION_INCIDENT_SCHEMA}, got {}",
                self.schema
            ));
        }
        if self.incident_id.is_empty() {
            return Err("incident_id must be non-empty".to_string());
        }
        if self.attempts.is_empty() {
            return Err("at least one release attempt must be recorded".to_string());
        }
        let mut seen_runs = std::collections::BTreeSet::new();
        for attempt in &self.attempts {
            if !seen_runs.insert(attempt.run_id) {
                return Err(format!("duplicate attempt run id {}", attempt.run_id));
            }
            validate_hex_identity(&attempt.commit, "attempt commit")?;
            validate_hex_identity(&attempt.tree, "attempt tree")?;
            if attempt.conclusion.is_empty() || attempt.result_class.is_empty() {
                return Err(format!(
                    "attempt {} must carry conclusion and result class",
                    attempt.run_id
                ));
            }
            if attempt.run_attempt == 0 {
                return Err(format!(
                    "attempt {} must carry a nonzero provider run attempt",
                    attempt.run_id
                ));
            }
            if attempt.jobs.is_empty() {
                return Err(format!(
                    "attempt {} must carry its provider job observations",
                    attempt.run_id
                ));
            }
            let mut seen_jobs = std::collections::BTreeSet::new();
            for job in &attempt.jobs {
                if job.name.is_empty() || job.conclusion.is_empty() {
                    return Err(format!(
                        "attempt {} carries a job without name or conclusion",
                        attempt.run_id
                    ));
                }
                if !seen_jobs.insert(job.name.clone()) {
                    return Err(format!(
                        "attempt {} lists job {} more than once",
                        attempt.run_id, job.name
                    ));
                }
                match (&job.first_failing_step, job.conclusion.as_str()) {
                    (Some(step), "failure") => {
                        if step.is_empty() {
                            return Err(format!(
                                "attempt {} job {} failed without naming its first failing step",
                                attempt.run_id, job.name
                            ));
                        }
                    }
                    (Some(_), other) => {
                        return Err(format!(
                            "attempt {} job {} has conclusion {other} but names a failing step",
                            attempt.run_id, job.name
                        ));
                    }
                    (None, "failure") => {
                        return Err(format!(
                            "attempt {} job {} failed without naming its first failing step",
                            attempt.run_id, job.name
                        ));
                    }
                    (None, _) => {}
                }
            }
            let mut seen_artifacts = std::collections::BTreeSet::new();
            for artifact in &attempt.artifacts {
                if artifact.name.is_empty() {
                    return Err(format!(
                        "attempt {} carries an artifact without a provider name",
                        attempt.run_id
                    ));
                }
                if !seen_artifacts.insert(artifact.artifact_id) {
                    return Err(format!(
                        "attempt {} lists artifact {} more than once",
                        attempt.run_id, artifact.artifact_id
                    ));
                }
                if artifact.expires_at.is_empty() {
                    return Err(format!(
                        "attempt {} artifact {} must carry its provider expiry",
                        attempt.run_id, artifact.artifact_id
                    ));
                }
                if artifact.inner_files.is_empty() && artifact.observation_note.is_none() {
                    return Err(format!(
                        "attempt {} artifact {} was listed without inner observation and must name that limit",
                        attempt.run_id, artifact.artifact_id
                    ));
                }
                for inner in &artifact.inner_files {
                    if inner.name.is_empty() {
                        return Err(format!(
                            "attempt {} artifact {} carries an inner file without a name",
                            attempt.run_id, artifact.artifact_id
                        ));
                    }
                    validate_checksum(&inner.sha256, &format!("{} inner file", artifact.name))?;
                }
            }
            match attempt.retained_publish_receipt_artifact_id {
                Some(receipt_id) => {
                    if !attempt
                        .artifacts
                        .iter()
                        .any(|artifact| artifact.artifact_id == receipt_id)
                    {
                        return Err(format!(
                            "attempt {} pins publish receipt {} outside its observed artifact set",
                            attempt.run_id, receipt_id
                        ));
                    }
                }
                None => {
                    if attempt.observation_limit.is_none() {
                        return Err(format!(
                            "attempt {} without a publish receipt must carry an explicit observation limit",
                            attempt.run_id
                        ));
                    }
                }
            }
            if attempt.artifacts.is_empty() && attempt.observation_limit.is_none() {
                return Err(format!(
                    "attempt {} retained no artifacts and must carry an explicit observation limit",
                    attempt.run_id
                ));
            }
        }

        if self.row_reconciliations.len() != 10 {
            return Err(format!(
                "the candidate denominator is 10 rows, got {}",
                self.row_reconciliations.len()
            ));
        }
        let mut seen_packages = std::collections::BTreeSet::new();
        for row in &self.row_reconciliations {
            if !seen_packages.insert(row.package()) {
                return Err(format!("duplicate row for package {}", row.package()));
            }
            match row {
                RowReconciliationV1::PublishedExactCandidate {
                    candidate_commit,
                    candidate_tree,
                    expected_checksum,
                    observed_checksum,
                    ..
                } => {
                    validate_hex_identity(candidate_commit, "candidate commit")?;
                    validate_hex_identity(candidate_tree, "candidate tree")?;
                    validate_checksum(expected_checksum, row.package())?;
                    validate_checksum(observed_checksum, row.package())?;
                    if expected_checksum != observed_checksum {
                        return Err(format!(
                            "{}: exact-candidate rows must have equal expected and observed checksums; use checksum_conflict instead",
                            row.package()
                        ));
                    }
                }
                RowReconciliationV1::PublishedAcrossCandidateHistory {
                    registry_checksum,
                    retained_candidate_checksums,
                    note,
                    ..
                } => {
                    validate_checksum(registry_checksum, row.package())?;
                    if note.is_empty() {
                        return Err(format!(
                            "{}: across-candidate rows must name the provenance gap",
                            row.package()
                        ));
                    }
                    if retained_candidate_checksums.is_empty() {
                        return Err(format!(
                            "{}: across-candidate rows must list the retained candidate checksums that do not match",
                            row.package()
                        ));
                    }
                    if retained_candidate_checksums
                        .iter()
                        .any(|candidate| candidate == registry_checksum)
                    {
                        return Err(format!(
                            "{}: registry bytes match a retained candidate; classify as published_exact_candidate",
                            row.package()
                        ));
                    }
                }
                RowReconciliationV1::PublishedButCandidateEvidenceUnavailable {
                    registry_checksum,
                    note,
                    ..
                } => {
                    validate_checksum(registry_checksum, row.package())?;
                    if note.is_empty() {
                        return Err(format!(
                            "{}: unavailable-evidence rows must describe the gap",
                            row.package()
                        ));
                    }
                }
                RowReconciliationV1::Yanked {
                    registry_checksum, ..
                } => {
                    validate_checksum(registry_checksum, row.package())?;
                }
                RowReconciliationV1::ChecksumConflict {
                    expected_checksum,
                    observed_checksum,
                    ..
                } => {
                    validate_checksum(expected_checksum, row.package())?;
                    validate_checksum(observed_checksum, row.package())?;
                }
                RowReconciliationV1::InstrumentFailure { note, .. } => {
                    if note.is_empty() {
                        return Err("instrument-failure rows must describe the failure".to_string());
                    }
                }
                RowReconciliationV1::Missing { .. }
                | RowReconciliationV1::ProviderUnavailable { .. } => {}
            }
        }

        if self.github_release.assets.len() != self.github_release.asset_count as usize {
            return Err(format!(
                "the release observation pins {} assets but declares asset_count {}",
                self.github_release.assets.len(),
                self.github_release.asset_count
            ));
        }
        let mut seen_assets = std::collections::BTreeSet::new();
        for asset in &self.github_release.assets {
            if !seen_assets.insert(asset.asset_id) {
                return Err(format!("duplicate release asset id {}", asset.asset_id));
            }
            if asset.name.is_empty() || asset.state.is_empty() {
                return Err(format!(
                    "release asset {} must carry a provider name and state",
                    asset.asset_id
                ));
            }
            validate_checksum(&asset.digest, &format!("asset {}", asset.name))?;
        }

        let mut seen_install_rows = std::collections::BTreeSet::new();
        for install in &self.install_observations {
            if install.platform.is_empty() || install.observed_note.is_empty() {
                return Err(format!(
                    "install observation for artifact {} must carry platform and note",
                    install.receipt_artifact_id
                ));
            }
            if !seen_install_rows.insert((install.run_id, install.receipt_artifact_id)) {
                return Err(format!(
                    "duplicate install observation for run {} artifact {}",
                    install.run_id, install.receipt_artifact_id
                ));
            }
            validate_checksum(
                &install.receipt_inner_sha256,
                &format!("install receipt {}", install.receipt_artifact_id),
            )?;
            let attempt = self
                .attempts
                .iter()
                .find(|attempt| attempt.run_id == install.run_id)
                .ok_or_else(|| {
                    format!(
                        "install observation references unobserved run {}",
                        install.run_id
                    )
                })?;
            let artifact = attempt
                .artifacts
                .iter()
                .find(|artifact| artifact.artifact_id == install.receipt_artifact_id)
                .ok_or_else(|| {
                    format!(
                        "install observation references artifact {} outside run {}'s observed set",
                        install.receipt_artifact_id, install.run_id
                    )
                })?;
            if !artifact
                .inner_files
                .iter()
                .any(|inner| inner.sha256 == install.receipt_inner_sha256)
            {
                return Err(format!(
                    "install observation digest for artifact {} does not match any observed inner file",
                    install.receipt_artifact_id
                ));
            }
        }

        validate_hex_identity(
            &self.tag_observation.current_target_commit,
            "tag target commit",
        )?;
        if self.tag_observation.events.is_empty()
            && self.tag_observation.observation_limit.is_none()
        {
            return Err(
                "tag history needs either observed events or an explicit observation limit"
                    .to_string(),
            );
        }

        if self.observation_completeness == ObservationCompletenessV1::PartialObservation
            && self.observation_limits.is_empty()
        {
            return Err(
                "partial observation must carry at least one explicit observation limit"
                    .to_string(),
            );
        }
        if self.observation_completeness == ObservationCompletenessV1::CompleteObservation
            && !self.observation_limits.is_empty()
        {
            return Err("complete observation must not carry observation limits".to_string());
        }
        if self.observation_completeness == ObservationCompletenessV1::CompleteObservation
            && self.row_reconciliations.iter().any(|row| {
                matches!(
                    row,
                    RowReconciliationV1::PublishedAcrossCandidateHistory { .. }
                        | RowReconciliationV1::PublishedButCandidateEvidenceUnavailable { .. }
                )
            })
        {
            return Err(
                "complete observation cannot carry rows without retained candidate provenance"
                    .to_string(),
            );
        }

        if self.channel_posture != ChannelPostureV1::PublicPrereleaseWithIncident {
            return Err(
                "the incident ruling fixes the channel posture; it is not a free field".to_string(),
            );
        }
        if self.final_candidate_eligibility != FinalCandidateEligibilityV1::NotReusable {
            return Err(
                "the incident ruling fixes final-candidate eligibility; it is not a free field"
                    .to_string(),
            );
        }
        if self.claim_boundary.is_empty() {
            return Err("claim_boundary must be non-empty".to_string());
        }
        Ok(())
    }
}

fn validate_hex_identity(value: &str, label: &str) -> Result<(), String> {
    let is_hex = value.len() >= 40
        && value.len() <= 64
        && value.chars().all(|character| character.is_ascii_hexdigit());
    if !is_hex {
        return Err(format!(
            "{label} `{value}` is not a 40-64 character hex identity"
        ));
    }
    Ok(())
}

fn validate_checksum(value: &str, package: &str) -> Result<(), String> {
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    let valid = digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(format!(
            "{package}: checksum `{value}` is not a sha256 digest"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_candidate_row(package: &str) -> RowReconciliationV1 {
        RowReconciliationV1::PublishedExactCandidate {
            package: package.to_string(),
            version: "0.2.0-rc.1".to_string(),
            candidate_commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
            candidate_tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
            candidate_receipt_artifact_id: 9506518765,
            expected_checksum:
                "sha256:a3f6c99da45120b96968349bc51c83ddf81e6d7a2c49e06e8727d3ab4195a3b2"
                    .to_string(),
            observed_checksum:
                "sha256:a3f6c99da45120b96968349bc51c83ddf81e6d7a2c49e06e8727d3ab4195a3b2"
                    .to_string(),
        }
    }

    fn across_candidate_row(package: &str) -> RowReconciliationV1 {
        RowReconciliationV1::PublishedAcrossCandidateHistory {
            package: package.to_string(),
            version: "0.2.0-rc.1".to_string(),
            registry_checksum:
                "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488"
                    .to_string(),
            retained_candidate_checksums: vec![
                "sha256:86fe2512fda53f78468273d5829876cf16c9c55e2f39d41652644fdd2fb22afd"
                    .to_string(),
            ],
            note: "registry bytes match no retained candidate".to_string(),
        }
    }

    fn incident(rows: Vec<RowReconciliationV1>) -> RcPublicationIncidentV1 {
        RcPublicationIncidentV1 {
            schema: RC_PUBLICATION_INCIDENT_SCHEMA.to_string(),
            incident_id: "rc1-moved-tag".to_string(),
            channel_version: "0.2.0-rc.1".to_string(),
            channel_tag: "v0.2.0-rc.1".to_string(),
            attempts: vec![ReleaseAttemptV1 {
                run_id: 32688128233,
                run_attempt: 1,
                commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
                tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
                conclusion: "failure".to_string(),
                result_class: "partial".to_string(),
                retained_publish_receipt_artifact_id: Some(9506518765),
                jobs: vec![AttemptJobObservationV1 {
                    name: "publish".to_string(),
                    conclusion: "failure".to_string(),
                    first_failing_step: Some(
                        "Publish cargo-allow topology rows in dependency order".to_string(),
                    ),
                }],
                artifacts: vec![AttemptArtifactObservationV1 {
                    artifact_id: 9506518765,
                    name: "release-publish-receipt".to_string(),
                    size_in_bytes: 2982,
                    expired: false,
                    expires_at: "2026-11-22T03:55:23Z".to_string(),
                    inner_files: vec![ArtifactInnerFileV1 {
                        name: "release-publish.receipt.json".to_string(),
                        size_in_bytes: 455,
                        sha256: "sha256:7330c08b2cf2c3735dddefd967f37e8bfadd68891a23df923ad3675b98ce6f59".to_string(),
                    }],
                    observation_note: None,
                }],
                observation_limit: None,
            }],
            tag_observation: TagObservationV1 {
                current_tag_object: "994343d6c5357f6bd4530f9ed1fd95424624e48f".to_string(),
                current_target_commit: "8bdabcd18723e36bbcfada423fa44779eefa599c".to_string(),
                events: Vec::new(),
                observation_limit: Some("tag event history unretained".to_string()),
            },
            row_reconciliations: rows,
            github_release: GitHubReleaseObservationV1 {
                release_id: 375500943,
                name: "v0.2.0-rc.1".to_string(),
                draft: false,
                prerelease: false,
                target_commitish: "main".to_string(),
                asset_count: 1,
                assets: vec![ReleaseAssetObservationV1 {
                    asset_id: 527214043,
                    name: "release-manifest-v2.json".to_string(),
                    size_in_bytes: 4315,
                    digest: "sha256:a0458fdf012c11cb56f4fd771e788c8cdc8e4676f8551e301aa89a3d5608ff33".to_string(),
                    state: "uploaded".to_string(),
                }],
                observed_note: "RC published with a stable release posture".to_string(),
            },
            install_observations: Vec::new(),
            registry_observation: RegistryObservationV1 {
                observed_at: "2026-08-30".to_string(),
                rows_present: 10,
                yanked_count: 0,
            },
            observation_completeness: ObservationCompletenessV1::PartialObservation,
            observation_limits: vec!["tag event history unretained".to_string()],
            channel_posture: ChannelPostureV1::PublicPrereleaseWithIncident,
            final_candidate_eligibility: FinalCandidateEligibilityV1::NotReusable,
            claim_boundary: "retained evidence only".to_string(),
        }
    }

    fn fixture_attempt(jobs: Vec<AttemptJobObservationV1>) -> ReleaseAttemptV1 {
        ReleaseAttemptV1 {
            run_id: 32688128233,
            run_attempt: 1,
            commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
            tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
            conclusion: "failure".to_string(),
            result_class: "partial".to_string(),
            retained_publish_receipt_artifact_id: Some(9506518765),
            jobs,
            artifacts: vec![AttemptArtifactObservationV1 {
                artifact_id: 9506518765,
                name: "release-publish-receipt".to_string(),
                size_in_bytes: 2982,
                expired: false,
                expires_at: "2026-11-22T03:55:23Z".to_string(),
                inner_files: vec![ArtifactInnerFileV1 {
                    name: "release-publish.receipt.json".to_string(),
                    size_in_bytes: 455,
                    sha256:
                        "sha256:7330c08b2cf2c3735dddefd967f37e8bfadd68891a23df923ad3675b98ce6f59"
                            .to_string(),
                }],
                observation_note: None,
            }],
            observation_limit: None,
        }
    }

    #[test]
    fn valid_reconciliation_passes() {
        let rows = (0..8)
            .map(|index| exact_candidate_row(&format!("allow-{index}")))
            .chain([
                across_candidate_row("allow-diff"),
                across_candidate_row("cargo-allow"),
            ])
            .collect();
        let record = incident(rows);
        assert_eq!(record.validate(), Ok(()));
    }

    #[test]
    fn wrong_row_count_or_mismatched_exact_checksums_fail() {
        let short = incident(vec![exact_candidate_row("allow-core")]);
        assert!(
            short.validate().is_err(),
            "a non-10-row record must be rejected"
        );
        let mismatch = RowReconciliationV1::PublishedExactCandidate {
            package: "allow-core".to_string(),
            version: "0.2.0-rc.1".to_string(),
            candidate_commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
            candidate_tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
            candidate_receipt_artifact_id: 9506518765,
            expected_checksum:
                "sha256:86fe2512fda53f78468273d5829876cf16c9c55e2f39d41652644fdd2fb22afd"
                    .to_string(),
            observed_checksum:
                "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488"
                    .to_string(),
        };
        let mut rows: Vec<RowReconciliationV1> = (0..8)
            .map(|index| exact_candidate_row(&format!("allow-{index}")))
            .collect();
        rows.push(mismatch);
        rows.push(across_candidate_row("cargo-allow"));
        let record = incident(rows);
        assert!(
            record.validate().is_err(),
            "a mismatched exact-candidate row must be rejected"
        );
    }

    #[test]
    fn every_row_state_carries_its_required_evidence() {
        let digest = |hex: &str| format!("sha256:{hex}");
        let checksum_a = digest(&"ab".repeat(32));
        let rows = vec![
            RowReconciliationV1::PublishedExactCandidate {
                package: "allow-0".to_string(),
                version: "0.2.0-rc.1".to_string(),
                candidate_commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
                candidate_tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
                candidate_receipt_artifact_id: 1,
                expected_checksum: checksum_a.clone(),
                observed_checksum: checksum_a.clone(),
            },
            RowReconciliationV1::PublishedButCandidateEvidenceUnavailable {
                package: "allow-1".to_string(),
                version: "0.2.0-rc.1".to_string(),
                registry_checksum: checksum_a.clone(),
                note: "receipt unretained".to_string(),
            },
            RowReconciliationV1::Missing {
                package: "allow-2".to_string(),
                version: "0.2.0-rc.1".to_string(),
            },
            RowReconciliationV1::Yanked {
                package: "allow-3".to_string(),
                version: "0.2.0-rc.1".to_string(),
                registry_checksum: checksum_a.clone(),
            },
            RowReconciliationV1::ChecksumConflict {
                package: "allow-4".to_string(),
                version: "0.2.0-rc.1".to_string(),
                expected_checksum: checksum_a.clone(),
                observed_checksum: digest(&"cd".repeat(32)),
            },
            RowReconciliationV1::ProviderUnavailable {
                package: "allow-5".to_string(),
            },
            RowReconciliationV1::InstrumentFailure {
                package: "allow-6".to_string(),
                note: "scanner failed mid-observation".to_string(),
            },
            RowReconciliationV1::PublishedExactCandidate {
                package: "allow-7".to_string(),
                version: "0.2.0-rc.1".to_string(),
                candidate_commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
                candidate_tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
                candidate_receipt_artifact_id: 2,
                expected_checksum: checksum_a.clone(),
                observed_checksum: checksum_a.clone(),
            },
            RowReconciliationV1::PublishedExactCandidate {
                package: "allow-8".to_string(),
                version: "0.2.0-rc.1".to_string(),
                candidate_commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
                candidate_tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
                candidate_receipt_artifact_id: 3,
                expected_checksum: checksum_a.clone(),
                observed_checksum: checksum_a.clone(),
            },
            RowReconciliationV1::Missing {
                package: "allow-9".to_string(),
                version: "0.2.0-rc.1".to_string(),
            },
        ];
        let mut record = incident(rows);
        record.observation_limits = vec!["mixed-state fixture".to_string()];
        assert_eq!(record.validate(), Ok(()));

        record.row_reconciliations[3] = RowReconciliationV1::Yanked {
            package: "allow-3".to_string(),
            version: "0.2.0-rc.1".to_string(),
            registry_checksum: "not-a-digest".to_string(),
        };
        assert!(
            record.validate().is_err(),
            "yanked rows with malformed checksums must be rejected"
        );
        record.row_reconciliations[3] = RowReconciliationV1::ChecksumConflict {
            package: "allow-3".to_string(),
            version: "0.2.0-rc.1".to_string(),
            expected_checksum: "short".to_string(),
            observed_checksum: checksum_a.clone(),
        };
        assert!(
            record.validate().is_err(),
            "checksum-conflict rows with malformed checksums must be rejected"
        );
        record.row_reconciliations[3] = RowReconciliationV1::InstrumentFailure {
            package: "allow-3".to_string(),
            note: String::new(),
        };
        assert!(
            record.validate().is_err(),
            "instrument-failure rows without a note must be rejected"
        );
        record.row_reconciliations[1] =
            RowReconciliationV1::PublishedButCandidateEvidenceUnavailable {
                package: "allow-1".to_string(),
                version: "0.2.0-rc.1".to_string(),
                registry_checksum: checksum_a.clone(),
                note: String::new(),
            };
        assert!(
            record.validate().is_err(),
            "unavailable-evidence rows without a note must be rejected"
        );
    }

    #[test]
    fn validation_law_rejects_each_violation() {
        let rows: Vec<RowReconciliationV1> = (0..8)
            .map(|index| exact_candidate_row(&format!("allow-{index}")))
            .chain([
                across_candidate_row("allow-diff"),
                across_candidate_row("cargo-allow"),
            ])
            .collect();
        let base = incident(rows);

        let mut record = base.clone();
        record.schema = "wrong.schema".to_string();
        assert!(record.validate().is_err(), "wrong schema must be rejected");

        let mut record = base.clone();
        record.attempts.clear();
        assert!(record.validate().is_err(), "no attempts must be rejected");

        let mut record = base.clone();
        record.attempts.push(record.attempts[0].clone());
        assert!(
            record.validate().is_err(),
            "duplicate run ids must be rejected"
        );

        let mut record = base.clone();
        record.attempts[0].commit = "not-hex".to_string();
        assert!(
            record.validate().is_err(),
            "malformed attempt commits must be rejected"
        );

        let mut record = base.clone();
        record.tag_observation.observation_limit = None;
        assert!(
            record.validate().is_err(),
            "tag history without events or a limit must be rejected"
        );

        let mut record = base.clone();
        record.tag_observation.current_target_commit = "xyz".to_string();
        assert!(
            record.validate().is_err(),
            "malformed tag target commits must be rejected"
        );

        let mut record = base.clone();
        record.row_reconciliations[8] = across_candidate_row("allow-0");
        assert!(
            record.validate().is_err(),
            "duplicate package rows must be rejected"
        );

        let mut record = base.clone();
        record.row_reconciliations[0] = RowReconciliationV1::PublishedExactCandidate {
            package: "allow-0".to_string(),
            version: "0.2.0-rc.1".to_string(),
            candidate_commit: "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace".to_string(),
            candidate_tree: "73e98378d9053445cde2adfbc806c11a6cad6a67".to_string(),
            candidate_receipt_artifact_id: 9506518765,
            expected_checksum: "sha256:short".to_string(),
            observed_checksum: "sha256:short".to_string(),
        };
        assert!(
            record.validate().is_err(),
            "malformed exact-candidate checksums must be rejected"
        );

        let bad_across = RowReconciliationV1::PublishedAcrossCandidateHistory {
            package: "allow-diff".to_string(),
            version: "0.2.0-rc.1".to_string(),
            registry_checksum:
                "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488"
                    .to_string(),
            retained_candidate_checksums: vec![
                "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488"
                    .to_string(),
            ],
            note: "registry matches a retained candidate".to_string(),
        };
        let mut record = base.clone();
        record.row_reconciliations[8] = bad_across.clone();
        assert!(
            record.validate().is_err(),
            "across-candidate rows whose registry bytes match a retained candidate must be rejected"
        );

        let mut record = base.clone();
        record.row_reconciliations[8] = RowReconciliationV1::PublishedAcrossCandidateHistory {
            package: "allow-diff".to_string(),
            version: "0.2.0-rc.1".to_string(),
            registry_checksum:
                "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488"
                    .to_string(),
            retained_candidate_checksums: Vec::new(),
            note: "registry matches a retained candidate".to_string(),
        };
        assert!(
            record.validate().is_err(),
            "across-candidate rows without retained comparisons must be rejected"
        );

        let mut record = base.clone();
        record.row_reconciliations[8] = RowReconciliationV1::PublishedAcrossCandidateHistory {
            package: "allow-diff".to_string(),
            version: "0.2.0-rc.1".to_string(),
            registry_checksum:
                "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488"
                    .to_string(),
            retained_candidate_checksums: vec![
                "sha256:86fe2512fda53f78468273d5829876cf16c9c55e2f39d41652644fdd2fb22afd"
                    .to_string(),
            ],
            note: String::new(),
        };
        assert!(
            record.validate().is_err(),
            "across-candidate rows without a provenance note must be rejected"
        );
    }

    #[test]
    fn posture_eligibility_and_limits_are_lawful() {
        let rows: Vec<RowReconciliationV1> = (0..8)
            .map(|index| exact_candidate_row(&format!("allow-{index}")))
            .chain([
                across_candidate_row("allow-diff"),
                across_candidate_row("cargo-allow"),
            ])
            .collect();
        let mut record = incident(rows);
        assert_eq!(record.validate(), Ok(()));
        record.observation_limits.clear();
        assert!(
            record.validate().is_err(),
            "partial observation without limits must be rejected"
        );
        record.observation_limits = vec!["unobserved facet".to_string()];
        record.observation_completeness = ObservationCompletenessV1::CompleteObservation;
        assert!(
            record.validate().is_err(),
            "complete observation carrying limits must be rejected"
        );
        record.observation_limits.clear();
        assert!(
            record.validate().is_err(),
            "complete observation is false while across-candidate rows lack retained provenance"
        );
    }

    #[test]
    fn attempts_carry_provider_job_and_artifact_law() {
        let rows: Vec<RowReconciliationV1> = (0..8)
            .map(|index| exact_candidate_row(&format!("allow-{index}")))
            .chain([
                across_candidate_row("allow-diff"),
                across_candidate_row("cargo-allow"),
            ])
            .collect();
        let base = incident(rows);
        assert_eq!(base.validate(), Ok(()));

        let mut record = base.clone();
        record.attempts = vec![fixture_attempt(vec![AttemptJobObservationV1 {
            name: "publish".to_string(),
            conclusion: "failure".to_string(),
            first_failing_step: None,
        }])];
        assert!(
            record.validate().is_err(),
            "a failed job must name its first failing step"
        );

        let mut record = base.clone();
        record.attempts = vec![fixture_attempt(vec![AttemptJobObservationV1 {
            name: "publish".to_string(),
            conclusion: "success".to_string(),
            first_failing_step: Some("Set up job".to_string()),
        }])];
        assert!(
            record.validate().is_err(),
            "a non-failed job must not name a failing step"
        );

        let mut record = base.clone();
        let mut attempt = fixture_attempt(Vec::new());
        attempt.jobs = Vec::new();
        record.attempts = vec![attempt];
        assert!(
            record.validate().is_err(),
            "attempts must carry their provider job observations"
        );

        let mut record = base.clone();
        let mut attempt = fixture_attempt(vec![AttemptJobObservationV1 {
            name: "publish".to_string(),
            conclusion: "failure".to_string(),
            first_failing_step: Some("Publish cargo-allow topology rows".to_string()),
        }]);
        attempt.artifacts = Vec::new();
        attempt.observation_limit = Some("provider retains no artifacts for this run".to_string());
        assert!(
            attempt.retained_publish_receipt_artifact_id.is_some(),
            "fixture pins a receipt outside an empty artifact set"
        );
        record.attempts = vec![attempt];
        assert!(
            record.validate().is_err(),
            "a pinned publish receipt must sit inside the observed artifact set"
        );

        let mut record = base.clone();
        let mut attempt = fixture_attempt(vec![AttemptJobObservationV1 {
            name: "publish".to_string(),
            conclusion: "failure".to_string(),
            first_failing_step: Some("Publish cargo-allow topology rows".to_string()),
        }]);
        attempt.retained_publish_receipt_artifact_id = None;
        record.attempts = vec![attempt];
        assert!(
            record.validate().is_err(),
            "an attempt without a publish receipt must carry an explicit observation limit"
        );

        let mut record = base.clone();
        let mut attempt = fixture_attempt(vec![AttemptJobObservationV1 {
            name: "publish".to_string(),
            conclusion: "failure".to_string(),
            first_failing_step: Some("Publish cargo-allow topology rows".to_string()),
        }]);
        if let Some(artifact) = attempt.artifacts.first_mut() {
            artifact.inner_files = Vec::new();
            artifact.observation_note = None;
        }
        record.attempts = vec![attempt];
        assert!(
            record.validate().is_err(),
            "a listed-only artifact must name why its inner bytes were not observed"
        );
    }

    #[test]
    fn release_assets_and_install_observations_are_bound() {
        let rows: Vec<RowReconciliationV1> = (0..8)
            .map(|index| exact_candidate_row(&format!("allow-{index}")))
            .chain([
                across_candidate_row("allow-diff"),
                across_candidate_row("cargo-allow"),
            ])
            .collect();
        let base = incident(rows);
        assert_eq!(base.validate(), Ok(()));

        let mut record = base.clone();
        record.github_release.asset_count = 2;
        assert!(
            record.validate().is_err(),
            "asset_count must equal the pinned asset observation set"
        );

        let mut record = base.clone();
        record.install_observations = vec![InstallObservationV1 {
            platform: "ubuntu-latest".to_string(),
            run_id: 999,
            receipt_artifact_id: 1,
            receipt_inner_sha256:
                "sha256:7330c08b2cf2c3735dddefd967f37e8bfadd68891a23df923ad3675b98ce6f59"
                    .to_string(),
            observed_note: "identity fields only".to_string(),
        }];
        assert!(
            record.validate().is_err(),
            "install observations must reference an observed run"
        );

        let mut record = base.clone();
        record.install_observations = vec![InstallObservationV1 {
            platform: "ubuntu-latest".to_string(),
            run_id: 32688128233,
            receipt_artifact_id: 424242,
            receipt_inner_sha256:
                "sha256:7330c08b2cf2c3735dddefd967f37e8bfadd68891a23df923ad3675b98ce6f59"
                    .to_string(),
            observed_note: "identity fields only".to_string(),
        }];
        assert!(
            record.validate().is_err(),
            "install observations must reference an artifact inside the run's observed set"
        );

        let mut record = base.clone();
        record.install_observations = vec![InstallObservationV1 {
            platform: "ubuntu-latest".to_string(),
            run_id: 32688128233,
            receipt_artifact_id: 9506518765,
            receipt_inner_sha256:
                "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            observed_note: "identity fields only".to_string(),
        }];
        assert!(
            record.validate().is_err(),
            "install observation digests must match an observed inner file of the referenced artifact"
        );

        let mut record = base.clone();
        record.install_observations = vec![InstallObservationV1 {
            platform: "ubuntu-latest".to_string(),
            run_id: 32688128233,
            receipt_artifact_id: 9506518765,
            receipt_inner_sha256:
                "sha256:7330c08b2cf2c3735dddefd967f37e8bfadd68891a23df923ad3675b98ce6f59"
                    .to_string(),
            observed_note: "receipt carries identity fields only; no installation-result field"
                .to_string(),
        }];
        assert_eq!(record.validate(), Ok(()));
    }
}
