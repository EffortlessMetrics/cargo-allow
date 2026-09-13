//! Verified snapshot derivation controls for the release-freeze command.

use crate::cli::release_freeze_command as subject;
use allow_core::CargoAllowErrorKind;

#[test]
fn verified_manifest_version_uses_retained_snapshot_bytes() -> Result<(), Box<dyn std::error::Error>>
{
    let mut later_manifest = b"[workspace.package]\r\nversion = \"0.2.0\"\r\n".to_vec();
    let verified = std::collections::BTreeMap::from([(
        subject::WORKSPACE_MANIFEST_PATH,
        later_manifest.clone(),
    )]);
    // Model the admission boundary without a filesystem race: later input
    // changes cannot become a source for the root-free derivation helper.
    later_manifest.clear();
    later_manifest.extend_from_slice(b"[workspace.package]\nversion = \"9.9.9\"\n");
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
        (Vec::new(), "workspace.package.version"),
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
fn verified_manifest_version_selects_only_workspace_package()
-> Result<(), Box<dyn std::error::Error>> {
    let manifest = br#"version = "8.0.0"
[package]
name = "fixture"
version = "9.0.0"
[workspace.package]
version = "0.2.0"
[dependencies]
fixture = { version = "7.0.0" }
"#;
    let verified =
        std::collections::BTreeMap::from([(subject::WORKSPACE_MANIFEST_PATH, manifest.to_vec())]);
    let version = subject::verified_workspace_version(&verified)?;
    if version != "0.2.0" {
        return Err(format!("selected unrelated version {version:?}").into());
    }
    Ok(())
}

#[test]
fn verified_manifest_version_accepts_toml_field_forms() -> Result<(), Box<dyn std::error::Error>> {
    for manifest in [
        "[workspace.package]\nversion='0.2.0' # release\n",
        "workspace.package.version = \"0.2.0\"\n",
        "[workspace]\npackage = { version = \"0.2.0\" }\n",
    ] {
        let verified = std::collections::BTreeMap::from([(
            subject::WORKSPACE_MANIFEST_PATH,
            manifest.as_bytes().to_vec(),
        )]);
        if subject::verified_workspace_version(&verified)? != "0.2.0" {
            return Err(format!("workspace version not selected for {manifest:?}").into());
        }
    }
    Ok(())
}

#[test]
fn verified_manifest_version_rejects_missing_or_invalid_field()
-> Result<(), Box<dyn std::error::Error>> {
    for manifest in [
        "version = \"0.2.0\"\n",
        "[package]\nversion = \"0.2.0\"\n",
        "workspace = 1\n",
        "[workspace]\n",
        "[workspace]\npackage = false\n",
        "[workspace.package]\n",
        "[workspace.package]\nversion = 2\n",
        "[workspace.package]\nversion = { value = \"0.2.0\" }\n",
    ] {
        let verified = std::collections::BTreeMap::from([(
            subject::WORKSPACE_MANIFEST_PATH,
            manifest.as_bytes().to_vec(),
        )]);
        let error = subject::verified_workspace_version(&verified)
            .err()
            .ok_or("missing or non-string workspace version was accepted")?;
        if error.kind() != CargoAllowErrorKind::InstrumentFailure
            || !error.to_string().contains("workspace.package.version")
        {
            return Err(format!("incorrect field failure for {manifest:?}: {error}").into());
        }
    }
    Ok(())
}

#[test]
fn verified_manifest_version_rejects_invalid_toml() -> Result<(), Box<dyn std::error::Error>> {
    for manifest in [
        "[workspace.package]\nversion = \"0.2.0\"\nbroken = [\n",
        "[workspace.package]\nversion = \"0.2.0\"\nversion = \"0.3.0\"\n",
    ] {
        let verified = std::collections::BTreeMap::from([(
            subject::WORKSPACE_MANIFEST_PATH,
            manifest.as_bytes().to_vec(),
        )]);
        let error = subject::verified_workspace_version(&verified)
            .err()
            .ok_or("invalid TOML unexpectedly produced a version")?;
        if error.kind() != CargoAllowErrorKind::InstrumentFailure
            || !error.to_string().contains("Cargo.toml is not valid TOML")
        {
            return Err(format!("incorrect TOML failure: {error}").into());
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

/// Later reads deliberately supply different bytes so the oracle observes both
/// redundant acquisition and accidental derivation from unadmitted inputs.
struct CountingSubjectInputs {
    working: std::collections::BTreeMap<&'static str, Vec<u8>>,
    reads: Vec<String>,
    dirty: bool,
    hidden: bool,
    read_error: Option<&'static str>,
}

impl CountingSubjectInputs {
    fn committed() -> std::collections::BTreeMap<&'static str, &'static str> {
        std::collections::BTreeMap::from([
            (
                subject::WORKSPACE_MANIFEST_PATH,
                "[workspace.package]\nversion = \"0.2.0\"\n",
            ),
            (subject::CARGO_LOCK_PATH, "version = 4\n"),
            (subject::TOPOLOGY_PATH, "schema_version = 2\n"),
        ])
    }

    fn clean() -> Self {
        Self {
            working: Self::committed()
                .into_iter()
                .map(|(path, text)| (path, text.replace('\n', "\r\n").into_bytes()))
                .collect(),
            reads: Vec::new(),
            dirty: false,
            hidden: false,
            read_error: None,
        }
    }

    fn read_count(&self, path: &str) -> usize {
        self.reads
            .iter()
            .filter(|read| read.as_str() == path)
            .count()
    }
}

fn collector_input_error(message: impl Into<String>) -> allow_core::CargoAllowError {
    allow_core::CargoAllowError::with_kind(CargoAllowErrorKind::InstrumentFailure, message)
}

impl subject::SubjectInputs for CountingSubjectInputs {
    fn git(&mut self, args: &[&str]) -> allow_core::CargoAllowResult<String> {
        let text = match args {
            ["status", "--porcelain"] => {
                if self.dirty {
                    " M Cargo.toml"
                } else {
                    ""
                }
            }
            ["ls-files", "-v", "-z"] => {
                if self.hidden {
                    "S Cargo.toml\0"
                } else {
                    "H Cargo.toml\0"
                }
            }
            ["rev-parse", "HEAD"] => "committed-subject",
            ["rev-parse", "HEAD^{tree}"] => "committed-tree",
            ["log", "-1", "--format=%cI"] => "2026-09-13T00:00:00Z",
            ["show", blob] => {
                let path = blob
                    .strip_prefix("HEAD:")
                    .ok_or_else(|| collector_input_error("unexpected blob query"))?;
                return Self::committed()
                    .get(path)
                    .map(|text| (*text).to_owned())
                    .ok_or_else(|| {
                        collector_input_error(format!("unexpected committed path {path}"))
                    });
            }
            _ => {
                return Err(collector_input_error(format!(
                    "unexpected git arguments {args:?}"
                )));
            }
        };
        Ok(text.to_owned())
    }

    fn read_working_bytes(&mut self, path: &str) -> allow_core::CargoAllowResult<Vec<u8>> {
        self.reads.push(path.to_owned());
        if self.read_error == Some(path) {
            return Err(collector_input_error(format!(
                "read {path}: controlled failure"
            )));
        }
        if self.read_count(path) > 1 {
            return Ok(b"[workspace.package]\nversion = \"9.9.9\"\n".to_vec());
        }
        self.working
            .get(path)
            .cloned()
            .ok_or_else(|| collector_input_error(format!("unexpected working path {path}")))
    }
}

#[test]
fn collector_reads_each_input_once_and_derives_from_admitted_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let mut inputs = CountingSubjectInputs::clean();
    let lock = inputs
        .working
        .get(subject::CARGO_LOCK_PATH)
        .ok_or("missing lock fixture")?
        .clone();
    let topology = inputs
        .working
        .get(subject::TOPOLOGY_PATH)
        .ok_or("missing topology fixture")?
        .clone();
    let identity = subject::SubjectIdentity::collect(&mut inputs, "0.2.0")?;
    for path in CountingSubjectInputs::committed().keys() {
        let count = inputs.read_count(path);
        if count != 1 {
            return Err(format!("collector read {path} {count} times; expected once").into());
        }
    }
    if inputs.reads.len() != 3 {
        return Err(format!("unexpected collector reads: {:?}", inputs.reads).into());
    }
    if identity.version != "0.2.0"
        || identity.cargo_lock_digest != allow_core::sha256_v1_bytes(&lock)
        || identity.topology_digest != allow_core::sha256_v1_bytes(&topology)
    {
        return Err(
            format!("identity was not derived from admitted raw bytes: {identity:?}").into(),
        );
    }
    Ok(())
}

#[test]
fn collector_rejects_each_mismatched_input_without_reading_it_again()
-> Result<(), Box<dyn std::error::Error>> {
    for path in [
        subject::WORKSPACE_MANIFEST_PATH,
        subject::CARGO_LOCK_PATH,
        subject::TOPOLOGY_PATH,
    ] {
        let mut inputs = CountingSubjectInputs::clean();
        inputs
            .working
            .insert(path, b"uncommitted change\n".to_vec());
        let error = subject::SubjectIdentity::collect(&mut inputs, "0.2.0")
            .err()
            .ok_or("collector accepted mismatched working bytes")?;
        if error.kind() != CargoAllowErrorKind::InstrumentFailure
            || !error
                .to_string()
                .contains(&format!("working bytes for {path} differ"))
            || inputs.read_count(path) != 1
            || inputs.reads.last().map(String::as_str) != Some(path)
            || inputs.reads.iter().any(|read| inputs.read_count(read) != 1)
        {
            return Err(format!(
                "incorrect mismatch rejection: {error}; reads {:?}",
                inputs.reads
            )
            .into());
        }
    }
    Ok(())
}

#[test]
fn collector_propagates_invalid_bytes_and_read_failures() -> Result<(), Box<dyn std::error::Error>>
{
    for path in [
        subject::WORKSPACE_MANIFEST_PATH,
        subject::CARGO_LOCK_PATH,
        subject::TOPOLOGY_PATH,
    ] {
        for fail_read in [false, true] {
            let mut inputs = CountingSubjectInputs::clean();
            if fail_read {
                inputs.read_error = Some(path);
            } else {
                inputs.working.insert(path, vec![0xff]);
            }
            let error = subject::SubjectIdentity::collect(&mut inputs, "0.2.0")
                .err()
                .ok_or("collector accepted invalid input")?;
            if error.kind() != CargoAllowErrorKind::InstrumentFailure
                || !error.to_string().contains(path)
                || inputs.read_count(path) != 1
                || inputs.reads.last().map(String::as_str) != Some(path)
            {
                return Err(
                    format!("incorrect input failure: {error}; reads {:?}", inputs.reads).into(),
                );
            }
            if fail_read && !error.to_string().contains("controlled failure") {
                return Err(format!("read failure lost context: {error}").into());
            }
        }
    }
    Ok(())
}

#[test]
fn collector_rejects_dirty_or_hidden_subject_before_working_reads()
-> Result<(), Box<dyn std::error::Error>> {
    for hidden in [false, true] {
        let mut inputs = CountingSubjectInputs::clean();
        inputs.hidden = hidden;
        inputs.dirty = !hidden;
        let error = subject::SubjectIdentity::collect(&mut inputs, "0.2.0")
            .err()
            .ok_or("collector accepted a dirty or hidden subject")?;
        let diagnostic = if hidden {
            "hidden state"
        } else {
            "worktree is dirty"
        };
        if error.kind() != CargoAllowErrorKind::InstrumentFailure
            || !error.to_string().contains(diagnostic)
            || !inputs.reads.is_empty()
        {
            return Err(format!(
                "incorrect pre-admission rejection: {error}; reads {:?}",
                inputs.reads
            )
            .into());
        }
    }
    Ok(())
}
