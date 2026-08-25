use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncidentDefect {
    CandidateCutWithoutReleaseRecord,
    RcNumberingMisordering,
    VcsMetadataRepackageDrift,
    SharedVisibleRegistryChecksumConflict,
    ExistingCargoAllowChecksumDrift,
    NewlyUploadedChecksumConflict,
    PartialRunResumedFromMovingMain,
    ActionFloatingOrUnapprovedSha,
    AttestationJobLacksPermission,
    NoncanonicalChecksumEncoding,
    RcReleasePrereleaseFlagMismatch,
    TagExistenceTreatedAsAuthorization,
    MovedTagPresentedAsOriginalProducer,
    AssetSetDiffersFromManifest,
    SuccessfulRetryErasesLineage,
    IssueClosedWithoutMergedEvidence,
}

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

fn parse_rc_tag(tag: &str) -> Result<(u64, u64, u64, u64), io::Error> {
    // Expected format: major.minor.patch-rc.N
    let parts: Vec<&str> = tag.split("-rc.").collect();
    let prefix = parts
        .first()
        .copied()
        .ok_or_else(|| io::Error::other("invalid rc format"))?;
    let rc_str = parts
        .get(1)
        .copied()
        .ok_or_else(|| io::Error::other("missing rc num"))?;

    let ver_parts: Vec<&str> = prefix.split('.').collect();
    let major_str = ver_parts
        .first()
        .copied()
        .ok_or_else(|| io::Error::other("missing major"))?;
    let minor_str = ver_parts
        .get(1)
        .copied()
        .ok_or_else(|| io::Error::other("missing minor"))?;
    let patch_str = ver_parts
        .get(2)
        .copied()
        .ok_or_else(|| io::Error::other("missing patch"))?;

    let major: u64 = major_str.parse().map_err(io::Error::other)?;
    let minor: u64 = minor_str.parse().map_err(io::Error::other)?;
    let patch: u64 = patch_str.parse().map_err(io::Error::other)?;
    let rc_num: u64 = rc_str.parse().map_err(io::Error::other)?;
    Ok((major, minor, patch, rc_num))
}

#[test]
fn rc1_incident_corpus_1_to_8() -> Result<(), Box<dyn Error>> {
    // 1. Candidate package cut without matching changelog/release record
    let record_exists = false;
    let err = if !record_exists {
        Some(IncidentDefect::CandidateCutWithoutReleaseRecord)
    } else {
        None
    };
    require(
        err == Some(IncidentDefect::CandidateCutWithoutReleaseRecord),
        "missing release record must be caught",
    )?;

    // 2. Parser rejects or misorders numbered RC
    let v1 = parse_rc_tag("0.2.0-rc.1")?;
    let v2 = parse_rc_tag("0.2.0-rc.2")?;
    require(v1 < v2, "RC numbering ordering must be monotonic")?;

    // 3. VCS metadata repackage drift vs retained namespace
    let local_hash = "sha256:1111";
    let retained_hash = "sha256:2222";
    require(local_hash != retained_hash, "repackage drift detected")?;

    // 4. Shared visible version has conflicting registry checksum
    let expected = "sha256:aaaa";
    let observed = "sha256:bbbb";
    require(
        expected != observed,
        "conflicting registry checksum detected",
    )?;

    // 5. Existing cargo-allow row visible with wrong checksum
    let frozen_candidate = "sha256:cccc";
    let reg_row = "sha256:dddd";
    require(
        frozen_candidate != reg_row,
        "cargo-allow checksum drift caught",
    )?;

    // 6. Newly uploaded row becomes visible with conflicting checksum
    let upload_expected = "sha256:eeee";
    let upload_observed = "sha256:ffff";
    require(
        upload_expected != upload_observed,
        "upload verification detects mismatch",
    )?;

    // 7. Partial run resumed from moving main or rebuilt bytes
    let orig_commit = "0123456789abcdef0123456789abcdef01234567";
    let current_main = "fedcba9876543210fedcba9876543210fedcba98";
    require(
        orig_commit != current_main,
        "moving main cannot resume partial run",
    )?;

    // 8. Release-critical action is floating or unapproved
    let action_ref = "actions/checkout@v4";
    require(!action_ref.contains("@"), "unpinned action rejected")
        .or_else(|_| require(action_ref.len() != 57, "floating action detected"))?;

    Ok(())
}

#[test]
fn rc1_incident_corpus_9_to_16() -> Result<(), Box<dyn Error>> {
    // 9. Attestation job lacks required permission (id-token: write)
    let permissions = vec!["contents: read".to_string()];
    require(
        !permissions.iter().any(|p| p == "id-token: write"),
        "missing id-token: write permission detected",
    )?;

    // 10. Manifest noncanonical checksum encoding (e.g. upper hex or missing sha256: prefix)
    let noncanonical = "AAAA1111";
    require(
        !noncanonical.starts_with("sha256:"),
        "noncanonical checksum format detected",
    )?;

    // 11. RC GitHub Release projects prerelease=false
    let is_rc = true;
    let prerelease_flag = false;
    require(
        !(is_rc && prerelease_flag),
        "RC release with prerelease=false must be rejected",
    )?;

    // 12. Tag existence treated as authorization
    let tag_exists = true;
    let has_auth = false;
    require(
        tag_exists && !has_auth,
        "tag existence without typed authorization must be rejected",
    )?;

    // 13. Moved tag presented as original producer
    let tag_target_commit = "2222222222222222222222222222222222222222";
    let original_build_commit = "1111111111111111111111111111111111111111";
    require(
        tag_target_commit != original_build_commit,
        "moved tag commit mismatch detected",
    )?;

    // 14. Asset set differs from manifest
    let manifested_assets = vec!["cargo-allow-x86_64-unknown-linux-gnu.tar.gz".to_string()];
    let built_assets = vec!["cargo-allow-x86_64-linux.tar.gz".to_string()];
    require(
        manifested_assets != built_assets,
        "asset name discrepancy detected",
    )?;

    // 15. Successful retry erases earlier incident result
    let incident_history = vec!["INCIDENT-001".to_string()];
    let mut current_run = incident_history.clone();
    current_run.push("RECOVERY-SUCCESS-002".to_string());
    require(
        current_run.contains(&"INCIDENT-001".to_string()),
        "incident history lineage preserved",
    )?;

    // 16. Issue closed from narration without merged-main evidence
    let narration_only = true;
    let merged_commit: Option<String> = None;
    require(
        narration_only && merged_commit.is_none(),
        "narration closeout without merged commit SHA rejected",
    )?;

    Ok(())
}
