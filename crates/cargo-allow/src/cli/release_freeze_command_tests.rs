//! Verified snapshot derivation controls for the release-freeze command.

use crate::cli::release_freeze_command as subject;
use allow_core::CargoAllowErrorKind;

#[test]
fn verified_manifest_version_uses_retained_snapshot_bytes() -> Result<(), Box<dyn std::error::Error>>
{
    let mut later_manifest = b"# workspace\r\nversion = \"0.2.0\"\r\n".to_vec();
    let verified = std::collections::BTreeMap::from([(
        subject::WORKSPACE_MANIFEST_PATH,
        later_manifest.clone(),
    )]);
    // Model the admission boundary without a filesystem race: later input
    // changes cannot become a source for the root-free derivation helper.
    later_manifest.clear();
    later_manifest.extend_from_slice(b"version = \"9.9.9\"\n");
    let declared = subject::verified_workspace_version(&verified)?;
    if declared != "0.2.0" {
        return Err(format!("selected {declared:?} instead of the admitted version").into());
    }
    let later =
        std::collections::BTreeMap::from([(subject::WORKSPACE_MANIFEST_PATH, later_manifest)]);
    if subject::verified_workspace_version(&later)? != "9.9.9" {
        return Err("the control did not distinguish the later manifest bytes".into());
    }
    Ok(())
}

#[test]
fn verified_manifest_version_rejects_malformed_snapshot() -> Result<(), Box<dyn std::error::Error>>
{
    for (manifest, diagnostic) in [
        (vec![0xff], "Cargo.toml:"),
        (Vec::new(), "the workspace manifest has no version"),
    ] {
        let verified =
            std::collections::BTreeMap::from([(subject::WORKSPACE_MANIFEST_PATH, manifest)]);
        let error = subject::verified_workspace_version(&verified)
            .err()
            .ok_or("malformed admitted manifest unexpectedly produced a version")?;
        if error.kind() != CargoAllowErrorKind::InstrumentFailure
            || !error.to_string().contains(diagnostic)
        {
            return Err(format!("incorrect manifest failure: {error}").into());
        }
    }
    Ok(())
}

#[test]
fn verified_subject_input_rejects_missing_snapshot_entries()
-> Result<(), Box<dyn std::error::Error>> {
    let verified = std::collections::BTreeMap::new();
    for path in [
        subject::WORKSPACE_MANIFEST_PATH,
        subject::CARGO_LOCK_PATH,
        subject::TOPOLOGY_PATH,
    ] {
        let error = subject::verified_subject_input(&verified, path)
            .err()
            .ok_or("missing admitted input unexpectedly produced bytes")?;
        if error.kind() != CargoAllowErrorKind::InstrumentFailure
            || !error.to_string().contains(path)
        {
            return Err(format!("incorrect missing-input failure: {error}").into());
        }
    }
    if subject::verified_workspace_version(&verified).is_ok() {
        return Err("missing manifest unexpectedly produced a version".into());
    }
    Ok(())
}

#[test]
fn verified_subject_input_preserves_raw_digest_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let lock = b"lock\r\n";
    let topology = b"topology\n";
    let verified = std::collections::BTreeMap::from([
        (subject::CARGO_LOCK_PATH, lock.to_vec()),
        (subject::TOPOLOGY_PATH, topology.to_vec()),
    ]);
    for (path, expected) in [
        (subject::CARGO_LOCK_PATH, lock.as_slice()),
        (subject::TOPOLOGY_PATH, topology.as_slice()),
    ] {
        let retained = subject::verified_subject_input(&verified, path)?;
        if retained != expected {
            return Err(format!("admitted digest bytes changed for {path}").into());
        }
    }
    Ok(())
}
