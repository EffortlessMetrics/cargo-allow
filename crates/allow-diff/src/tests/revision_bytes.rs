//! Exact revision bytes must earn the same scanner disposition as current files.

use super::*;
use allow_core::{
    CappedReadError, CargoAllowError, CargoAllowErrorKind, SOURCE_FILE_READ_MAX_BYTES,
};
use allow_rust::RustFileScanOutcome;
use std::cell::Cell;
use std::io::{self, Cursor, Read};
use std::rc::Rc;

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Result<Self, String> {
        let fixture = Self(temp_root(label));
        fs::create_dir_all(fixture.0.join("src")).map_err(|error| error.to_string())?;
        fixture_git(&fixture.0, &["init"]);
        fixture_git(
            &fixture.0,
            &["config", "user.email", "cargo-allow@example.invalid"],
        );
        fixture_git(&fixture.0, &["config", "user.name", "cargo-allow test"]);
        fixture_git(&fixture.0, &["config", "core.autocrlf", "false"]);
        Ok(fixture)
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        fs::write(self.0.join(path), bytes).map_err(|error| error.to_string())
    }

    fn commit(&self) {
        fixture_git(&self.0, &["add", "."]);
        fixture_git(
            &self.0,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "revision byte fixture",
            ],
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn isolate_fixture_repository(command: &mut std::process::Command) {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
}

fn fixture_git_command(root: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(root);
    isolate_fixture_repository(&mut command);
    command
}

fn fixture_git(root: &Path, args: &[&str]) {
    let output = fixture_git_command(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("fixture git {args:?}: {error}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "fixture git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

#[test]
fn fixture_commits_own_bytes() -> Result<(), String> {
    let fixture = Fixture::new("isolated-revision-fixture")?;
    let source: &[u8] = b"pub fn isolated_fixture() -> u8 { 7 }\n";
    fixture.write("src/lib.rs", source)?;
    fixture.commit();
    let output = fixture_git_command(&fixture.0)
        .args(["show", "HEAD:src/lib.rs"])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() || output.stdout != source {
        return Err(format!(
            "the fixture must commit its own source bytes: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[test]
fn revision_fixtures_isolate_repository_environment() -> Result<(), String> {
    let canary = Fixture::new("revision-environment-canary")?;
    let source: &[u8] = b"// unrelated repository canary\n";
    canary.write("src/lib.rs", source)?;
    canary.commit();
    let git_dir = canary.0.join(".git");
    let index = git_dir.join("index");
    let index_before = fs::read(&index).map_err(|error| error.to_string())?;
    let head_before = fixture_git_command(&canary.0)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .map_err(|error| error.to_string())?;
    if !head_before.status.success() || head_before.stdout.is_empty() {
        return Err("the canary must start with a committed HEAD".to_string());
    }

    let test_name = "tests::revision_bytes::fixture_commits_own_bytes";
    for index_only in [true, false] {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut command = std::process::Command::new(executable);
        isolate_fixture_repository(&mut command);
        command
            .args(["--exact", test_name, "--nocapture", "--color", "never"])
            .env("GIT_INDEX_FILE", &index);
        if !index_only {
            command
                .env("GIT_DIR", &git_dir)
                .env("GIT_WORK_TREE", &canary.0)
                .env("GIT_COMMON_DIR", &git_dir)
                .env("GIT_OBJECT_DIRECTORY", git_dir.join("objects"))
                .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", git_dir.join("objects"));
        }
        let output = command.output().map_err(|error| error.to_string())?;
        let head_after = fixture_git_command(&canary.0)
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .map_err(|error| error.to_string())?;
        if fs::read(&index).map_err(|error| error.to_string())? != index_before
            || !head_after.status.success()
            || head_after.stdout != head_before.stdout
            || fs::read(canary.0.join("src/lib.rs")).map_err(|error| error.to_string())? != source
        {
            return Err(format!(
                "fixture child changed the canary index, commit or source (index_only={index_only})"
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || !stdout.contains(&format!("test {test_name} ... ok")) {
            return Err(format!(
                "the named fixture child did not pass (index_only={index_only}): {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    Ok(())
}

fn require_optional_package_context(
    fixture: &Fixture,
    paths: &[PathBuf],
    expected: &[(&str, Option<&str>)],
) -> Result<(), String> {
    let revision = scan_at_revision(&fixture.0, "HEAD", &AllowConfig::empty())
        .map_err(|error| error.to_string())?;
    let current =
        allow_rust::scan_rust_files(&fixture.0, paths).map_err(|error| error.to_string())?;
    let rust_findings = revision
        .findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::Panic)
        .cloned()
        .collect::<Vec<_>>();
    if revision.source_files_considered != paths.len()
        || revision.rust_files_considered != expected.len()
        || revision.rust_files_scanned != expected.len()
        || revision.rust_files_skipped != 0
        || revision.rust_files_with_parse_errors != 0
        || revision.scanner_completeness != "complete"
        || revision.inventory_completeness != "complete"
        || revision.rust_file_statuses != current.file_statuses
        || rust_findings != current.findings
        || rust_findings.len() != expected.len()
    {
        return Err(format!(
            "optional package context changed valid source scanning: {revision:?} / {current:?}"
        ));
    }
    for (path, package) in expected {
        let matches = rust_findings
            .iter()
            .filter(|finding| finding.path == Path::new(path))
            .collect::<Vec<_>>();
        if matches.len() != 1
            || matches
                .first()
                .is_none_or(|finding| finding.identity.crate_name.as_deref() != *package)
        {
            return Err(format!(
                "optional package context for {path} must be {package:?}: {matches:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn unreadable_optional_manifests_do_not_abort_revision_rust_scans() -> Result<(), String> {
    for (label, manifest) in [
        (
            "non-utf8",
            Some(b"[package]\nname = \"ignored\"\n#\xff\n".as_slice()),
        ),
        (
            "invalid-toml",
            Some(b"[package\nname = \"ignored\"\n".as_slice()),
        ),
        ("absent", None),
    ] {
        let fixture = Fixture::new(&format!("optional-manifest-{label}"))?;
        let mut paths = vec![PathBuf::from("src/lib.rs")];
        if let Some(manifest) = manifest {
            fixture.write("Cargo.toml", manifest)?;
            paths.insert(0, PathBuf::from("Cargo.toml"));
        }
        fixture.write(
            "src/lib.rs",
            b"fn retained(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )?;
        fixture.commit();
        require_optional_package_context(&fixture, &paths, &[("src/lib.rs", None)])?;
    }
    Ok(())
}

#[test]
fn optional_manifest_context_retains_the_cap_boundary() -> Result<(), String> {
    let fixture = Fixture::new("optional-manifest-cap")?;
    let cap = usize::try_from(SOURCE_FILE_READ_MAX_BYTES).map_err(|error| error.to_string())?;
    let mut manifest = b"[package]\nname = \"at-cap\"\nversion = \"0.1.0\"\n#".to_vec();
    manifest.resize(cap, b'x');
    fixture.write("Cargo.toml", &manifest)?;
    fixture.write(
        "src/lib.rs",
        b"fn retained(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )?;
    fixture.commit();
    let paths = vec!["Cargo.toml".into(), "src/lib.rs".into()];
    require_optional_package_context(&fixture, &paths, &[("src/lib.rs", Some("at-cap"))])?;
    manifest.push(b'x');
    fixture.write("Cargo.toml", &manifest)?;
    fixture.commit();
    require_optional_package_context(&fixture, &paths, &[("src/lib.rs", None)])?;
    let error = read_file_at_revision(&fixture.0, "HEAD", "Cargo.toml")
        .err()
        .ok_or_else(|| "the strict single-file reader accepted an over-cap manifest".to_string())?;
    if error.kind() != CargoAllowErrorKind::Scan || !error.to_string().contains("8388609 bytes") {
        return Err(format!("the source cap was weakened for manifests: {error}"));
    }
    Ok(())
}

#[test]
fn unreadable_nested_manifest_keeps_nearest_readable_package_context() -> Result<(), String> {
    let fixture = Fixture::new("optional-nested-package-context")?;
    for directory in ["src/omitted", "src/named"] {
        fs::create_dir_all(fixture.0.join(directory)).map_err(|error| error.to_string())?;
    }
    fixture.write("Cargo.toml", b"[package]\nname = \"outer\"\n")?;
    fixture.write(
        "src/omitted/Cargo.toml",
        b"[package]\nname = \"omitted\"\n#\xff\n",
    )?;
    fixture.write("src/named/Cargo.toml", b"[package]\nname = \"inner\"\n")?;
    for path in ["src/lib.rs", "src/omitted/lib.rs", "src/named/lib.rs"] {
        fixture.write(
            path,
            b"fn retained(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )?;
    }
    fixture.commit();
    let paths = vec![
        "Cargo.toml".into(),
        "src/lib.rs".into(),
        "src/named/Cargo.toml".into(),
        "src/named/lib.rs".into(),
        "src/omitted/Cargo.toml".into(),
        "src/omitted/lib.rs".into(),
    ];
    require_optional_package_context(
        &fixture,
        &paths,
        &[
            ("src/lib.rs", Some("outer")),
            ("src/named/lib.rs", Some("inner")),
            ("src/omitted/lib.rs", Some("outer")),
        ],
    )
}

#[test]
fn required_revision_companions_still_reject_unreadable_bytes() -> Result<(), String> {
    let cap = usize::try_from(SOURCE_FILE_READ_MAX_BYTES).map_err(|error| error.to_string())?;
    for (label, path, receipt, valid) in [
        (
            "attributes",
            ".gitattributes",
            generated_code_entry("generated/schema.json"),
            b"generated/schema.json linguist-generated=true\n".as_slice(),
        ),
        (
            "workflow",
            ".github/workflows/ci.yml",
            workflow_entry(
                "workflow-ci",
                "github_workflow",
                "github_workflow",
                ".github/workflows/ci.yml",
                None,
            ),
            b"steps:\n  - uses: actions/checkout@v4\n".as_slice(),
        ),
    ] {
        let fixture = Fixture::new(&format!("required-revision-{label}"))?;
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(fixture.0.join(parent)).map_err(|error| error.to_string())?;
        }
        fixture.write("src/lib.rs", b"fn retained() {}\n")?;
        fixture.write(path, valid)?;
        fixture.commit();
        let cfg = config_with(receipt);
        scan_at_revision(&fixture.0, "HEAD", &cfg).map_err(|error| error.to_string())?;
        let mut oversized = valid.to_vec();
        oversized.extend_from_slice(b"#");
        oversized.resize(cap + 1, b'x');
        for (bytes, reason) in [
            (b"#\xff\n".as_slice(), "not valid UTF-8"),
            (oversized.as_slice(), "8388609 bytes"),
        ] {
            fixture.write(path, bytes)?;
            fixture.commit();
            let error = scan_at_revision(&fixture.0, "HEAD", &cfg)
                .err()
                .ok_or_else(|| format!("required companion {path} was treated as optional"))?;
            if error.kind() != CargoAllowErrorKind::Scan
                || !error.to_string().contains(path)
                || !error.to_string().contains(reason)
            {
                return Err(format!(
                    "required companion lost its strict path error: {error}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn invalid_comment_bytes_preserve_valid_findings_and_order() -> Result<(), String> {
    let fixture = Fixture::new("revision-strict-utf8")?;
    fixture.write(
        "Cargo.toml",
        b"[package]\nname = \"exact-bytes\"\nversion = \"0.1.0\"\n",
    )?;
    let valid = b"fn retained(value: Option<u8>) -> u8 { value.unwrap() }\n";
    let invalid =
        b"// invalid byte: \xff\nfn omitted(value: Option<u8>) -> u8 { value.unwrap() }\n";
    // The two valid paths share an object identity. Both still need their own
    // path-bound findings, even though the batch reader requests the blob once.
    fixture.write("src/a.rs", valid)?;
    fixture.write("src/bad.rs", invalid)?;
    fixture.write("src/z.rs", valid)?;
    fixture.commit();

    let revision = scan_at_revision(&fixture.0, "HEAD", &AllowConfig::empty())
        .map_err(|error| error.to_string())?;
    let paths = vec![
        "Cargo.toml".into(),
        "src/a.rs".into(),
        "src/bad.rs".into(),
        "src/z.rs".into(),
    ];
    let current =
        allow_rust::scan_rust_files(&fixture.0, &paths).map_err(|error| error.to_string())?;
    if revision.rust_files_considered != 3
        || revision.rust_files_scanned != 2
        || revision.rust_files_skipped != 1
        || revision.rust_files_with_parse_errors != 0
        || revision.scanner_completeness != "partial"
        || revision.inventory_completeness != "complete"
        || revision.rust_file_statuses != current.file_statuses
    {
        return Err(format!(
            "revision/current strict-read disposition diverged: {revision:?} / {current:?}"
        ));
    }
    let rust_findings = revision
        .findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::Panic)
        .cloned()
        .collect::<Vec<_>>();
    if rust_findings != current.findings || rust_findings.len() != 2 {
        return Err(format!(
            "valid findings, identities or order changed: {rust_findings:?} / {:?}",
            current.findings,
        ));
    }
    if rust_findings
        .iter()
        .any(|finding| finding.identity.crate_name.as_deref() != Some("exact-bytes"))
    {
        return Err("streaming lost the source package context".to_string());
    }
    let rejected = revision
        .rust_file_statuses
        .iter()
        .find(|status| status.path == Path::new("src/bad.rs"))
        .ok_or_else(|| "selected invalid source has no disposition".to_string())?;
    if !matches!(
        &rejected.outcome,
        RustFileScanOutcome::Skipped { reason } if reason.contains("not valid UTF-8")
    ) || rejected.path.is_absolute()
    {
        return Err(format!(
            "invalid bytes were not a repository-relative rejected read: {rejected:?}"
        ));
    }
    // Exact single-file revision reads must not offer a second lossy route.
    let rejected_single = read_file_at_revision(&fixture.0, "HEAD", "src/bad.rs")
        .err()
        .ok_or_else(|| "single-file revision read accepted invalid UTF-8".to_string())?;
    if rejected_single.kind() != CargoAllowErrorKind::Scan
        || !rejected_single.to_string().contains("src/bad.rs")
    {
        return Err(format!(
            "single-file read lost its typed path diagnostic: {rejected_single}"
        ));
    }
    Ok(())
}

#[test]
fn revision_cap_and_cap_plus_one_match_current_tree_dispositions() -> Result<(), String> {
    let fixture = Fixture::new("revision-byte-cap")?;
    let cap = usize::try_from(SOURCE_FILE_READ_MAX_BYTES).map_err(|error| error.to_string())?;
    let mut source = b"fn bounded(value: Option<u8>) -> u8 { value.unwrap() }\n//".to_vec();
    source.resize(cap, b'x');
    fixture.write("src/at_cap.rs", &source)?;
    source.push(b'x');
    fixture.write("src/over_cap.rs", &source)?;
    drop(source);
    fixture.commit();

    let revision = scan_at_revision(&fixture.0, "HEAD", &AllowConfig::empty())
        .map_err(|error| error.to_string())?;
    let paths = vec!["src/at_cap.rs".into(), "src/over_cap.rs".into()];
    let current =
        allow_rust::scan_rust_files(&fixture.0, &paths).map_err(|error| error.to_string())?;
    if revision.rust_files_considered != 2
        || revision.rust_files_scanned != 1
        || revision.rust_files_skipped != 1
        || revision.rust_files_with_parse_errors != 0
        || revision.scanner_completeness != "partial"
        || revision.rust_file_statuses != current.file_statuses
        || revision.findings != current.findings
    {
        return Err(format!(
            "cap boundary did not match current-tree reads: {revision:?} / {current:?}"
        ));
    }
    let at_cap = revision
        .rust_file_statuses
        .first()
        .ok_or_else(|| "missing at-cap disposition".to_string())?;
    let over_cap = revision
        .rust_file_statuses
        .get(1)
        .ok_or_else(|| "missing over-cap disposition".to_string())?;
    if at_cap.path != Path::new("src/at_cap.rs")
        || at_cap.outcome != RustFileScanOutcome::Scanned
        || !matches!(
            &over_cap.outcome,
            RustFileScanOutcome::Skipped { reason }
                if reason.contains("8388609 bytes") && reason.contains("8388608-byte")
        )
        || revision.findings.len() != 1
    {
        return Err(format!(
            "at-cap and cap+1 dispositions were not distinct: {:?}",
            revision.rust_file_statuses,
        ));
    }
    Ok(())
}

#[test]
fn missing_git_object_is_an_inventory_failure() -> Result<(), String> {
    let fixture = Fixture::new("revision-missing-object")?;
    fixture.write("src/lib.rs", b"fn valid() {}\n")?;
    fixture.commit();
    let path = PathBuf::from("src/lib.rs");
    let tree = vec![revision_git::GitTreeFile {
        mode: "100644".to_string(),
        object_oid: "f".repeat(40),
        path: path.clone(),
        raw_path: b"src/lib.rs".to_vec(),
    }];
    let mut visited = 0;
    let result = revision_git::read_files_at_revision(&fixture.0, &tree, &[path], |_, _| {
        visited += 1;
        Ok(())
    });
    let error = result
        .err()
        .ok_or_else(|| "missing blob produced clean empty source".to_string())?;
    if error.kind() != CargoAllowErrorKind::Inventory || visited != 0 {
        return Err(format!(
            "missing object was not an inventory failure: {error}, visits={visited}"
        ));
    }
    Ok(())
}

#[test]
fn batch_rejections_preserve_bytes_and_require_exact_identities() -> Result<(), String> {
    let first = "a".repeat(40);
    let second = "b".repeat(40);
    let mut bytes = format!("{first} blob 3\n").into_bytes();
    bytes.extend_from_slice(b"/\xff/\n");
    bytes.extend_from_slice(format!("{second} blob 2\nok\n").as_bytes());
    let mut seen = Vec::new();
    revision_git::visit_git_cat_file_batch_for_test(
        bytes.as_slice(),
        &[first.clone(), second.clone()],
        |oid, source| {
            seen.push((
                oid.to_string(),
                source.map(str::to_string).map_err(ToString::to_string),
            ));
            Ok(())
        },
    )
    .map_err(|error| error.to_string())?;
    if seen.len() != 2
        || !matches!(
            seen.first(),
            Some((oid, Err(reason))) if oid == &first && reason.contains("not valid UTF-8")
        )
        || !matches!(seen.get(1), Some((oid, Ok(text))) if oid == &second && text == "ok")
    {
        return Err(format!(
            "invalid source desynchronized following exact bytes: {seen:?}"
        ));
    }
    for (bytes, requested) in [
        (
            format!("{second} blob 0\n\n").into_bytes(),
            vec![first.clone()],
        ),
        (
            format!("{first} blob 0\n\n{second} blob 0\n\n").into_bytes(),
            vec![first.clone()],
        ),
        (
            format!("{first} blob 0\n\n{first} blob 0\n\n").into_bytes(),
            vec![first.clone(), second.clone()],
        ),
        (
            format!("{first} blob 3\nab").into_bytes(),
            vec![first.clone()],
        ),
        (
            format!("{first} blob 2\nab!").into_bytes(),
            vec![first.clone()],
        ),
        (
            format!("{first} blob 0\n").into_bytes(),
            vec![first.clone()],
        ),
        (vec![b'a'; 129], vec![first.clone()]),
    ] {
        let error = revision_git::visit_git_cat_file_batch_for_test(
            bytes.as_slice(),
            &requested,
            |_, _| Ok(()),
        )
        .err()
        .ok_or_else(|| "malformed batch response was accepted".to_string())?;
        if error.kind() != CargoAllowErrorKind::Inventory {
            return Err(format!(
                "malformed response lost inventory classification: {error}"
            ));
        }
    }
    Ok(())
}

#[test]
fn early_batch_consumer_failure_reaps_git_without_reading_the_aggregate() -> Result<(), String> {
    let fixture = Fixture::new("batch-consumer-failure")?;
    for index in 0..64 {
        let mut source = format!("// source {index}\n//").into_bytes();
        source.resize(32 * 1024, b'x');
        fixture.write(&format!("src/file_{index:02}.rs"), &source)?;
    }
    fixture.commit();
    let tree = revision_git::git_tree_files_at_revision(&fixture.0, "HEAD")
        .map_err(|error| error.to_string())?;
    let paths = tree
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let mut visited = 0;
    let result = revision_git::read_files_at_revision(&fixture.0, &tree, &paths, |_, _| {
        visited += 1;
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Scan,
            "consumer rejected the first selected source",
        ))
    });
    let error = result
        .err()
        .ok_or_else(|| "consumer failure was discarded".to_string())?;
    if visited != 1
        || error.kind() != CargoAllowErrorKind::Scan
        || !error.to_string().contains("consumer rejected")
    {
        return Err(format!(
            "batch did not preserve the early consumer failure: {error}"
        ));
    }
    Ok(())
}

struct MeteredReader<R> {
    inner: R,
    bytes_read: Rc<Cell<u64>>,
    largest_read: Rc<Cell<usize>>,
}

impl<R: Read> Read for MeteredReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.largest_read
            .set(self.largest_read.get().max(buffer.len()));
        let count = self.inner.read(buffer)?;
        self.bytes_read.set(self.bytes_read.get() + count as u64);
        Ok(count)
    }
}

#[test]
fn oversized_batch_body_is_drained_in_bounded_reads_before_next_valid_blob() -> Result<(), String> {
    let first = "a".repeat(40);
    let second = "b".repeat(40);
    let size = SOURCE_FILE_READ_MAX_BYTES + 1;
    let bytes_read = Rc::new(Cell::new(0));
    let largest_read = Rc::new(Cell::new(0));
    let reader = MeteredReader {
        inner: Cursor::new(format!("{first} blob {size}\n").into_bytes())
            .chain(io::repeat(b'x').take(size))
            .chain(Cursor::new(format!("\n{second} blob 2\nok\n").into_bytes())),
        bytes_read: Rc::clone(&bytes_read),
        largest_read: Rc::clone(&largest_read),
    };
    let mut visited = 0;
    revision_git::visit_git_cat_file_batch_for_test(reader, &[first, second], |_, source| {
        visited += 1;
        let expected = if visited == 1 {
            matches!(
                source,
                Err(CappedReadError::Oversized { len: Some(actual), limit })
                    if *actual == size && *limit == SOURCE_FILE_READ_MAX_BYTES
            )
        } else {
            source.is_ok_and(|text| text == "ok")
        };
        if !expected {
            return Err(CargoAllowError::new(
                "oversized body changed the following response",
            ));
        }
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    if visited != 2 || bytes_read.get() < size || largest_read.get() > 64 * 1024 {
        return Err(format!(
            "oversized body was not drained with bounded buffers: visits={visited}, bytes={}, largest={}",
            bytes_read.get(),
            largest_read.get(),
        ));
    }
    Ok(())
}

#[test]
fn aggregate_batch_bytes_are_streamed_before_later_source_is_read() -> Result<(), String> {
    let oids = (1..=16)
        .map(|index| format!("{index:040x}"))
        .collect::<Vec<_>>();
    let blob_size = 1024 * 1024u64;
    let mut stream: Box<dyn Read> = Box::new(io::empty());
    for oid in &oids {
        stream = Box::new(
            stream
                .chain(Cursor::new(
                    format!("{oid} blob {blob_size}\n").into_bytes(),
                ))
                .chain(io::repeat(b'x').take(blob_size))
                .chain(Cursor::new(b"\n")),
        );
    }
    let bytes_read = Rc::new(Cell::new(0));
    let reader = MeteredReader {
        inner: stream,
        bytes_read: Rc::clone(&bytes_read),
        largest_read: Rc::new(Cell::new(0)),
    };
    let mut visited = 0u64;
    revision_git::visit_git_cat_file_batch_for_test(reader, &oids, |_, source| {
        visited += 1;
        if !source.is_ok_and(|text| text.len() as u64 == blob_size)
            || bytes_read.get() > visited * (blob_size + 128) + 64 * 1024
        {
            return Err(CargoAllowError::new(
                "batch retained later source before visiting the current blob",
            ));
        }
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    if visited != 16 || bytes_read.get() <= SOURCE_FILE_READ_MAX_BYTES {
        return Err("aggregate fixture did not exceed the per-file cap".to_string());
    }
    Ok(())
}

#[test]
fn revision_scan_retains_one_git_batch_for_many_selected_sources() -> Result<(), String> {
    let trace_root = Fixture::new("batch-trace")?;
    let trace_path = trace_root.0.join("batch-trace.json");
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let output = std::process::Command::new(executable)
        .args([
            "--exact",
            "tests::revision_bytes::trace_child_scans_many_selected_sources",
            "--ignored",
            "--nocapture",
        ])
        .env("GIT_TRACE2_EVENT", &trace_path)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "batched scan child failed: {}",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let trace = fs::read_to_string(&trace_path).map_err(|error| error.to_string())?;
    let mut batches = 0;
    for line in trace.lines() {
        let event: serde_json::Value =
            serde_json::from_str(line).map_err(|error| error.to_string())?;
        if event.get("event").and_then(serde_json::Value::as_str) != Some("start") {
            continue;
        }
        let Some(args) = event.get("argv").and_then(serde_json::Value::as_array) else {
            continue;
        };
        if args.iter().any(|arg| arg.as_str() == Some("cat-file"))
            && args.iter().any(|arg| arg.as_str() == Some("--batch"))
        {
            batches += 1;
        }
    }
    if batches != 1 {
        return Err(format!(
            "one revision scan should use one Git source batch, observed {batches}"
        ));
    }
    Ok(())
}

#[test]
#[ignore = "invoked by the parent Trace2 isolation test"]
fn trace_child_scans_many_selected_sources() -> Result<(), String> {
    let fixture = Fixture::new("many-sources")?;
    for index in 0..64 {
        fixture.write(
            &format!("src/file_{index:02}.rs"),
            format!("fn value_{index}(value: Option<u8>) -> u8 {{ value.unwrap() }}\n").as_bytes(),
        )?;
    }
    fixture.commit();
    let scan = scan_at_revision(&fixture.0, "HEAD", &AllowConfig::empty())
        .map_err(|error| error.to_string())?;
    if scan.rust_files_scanned != 64 || scan.rust_files_skipped != 0 || scan.findings.len() != 64 {
        return Err(format!(
            "batched scan did not retain all valid source findings: {scan:?}"
        ));
    }
    Ok(())
}

#[test]
fn batch_stderr_is_capped_but_excess_diagnostics_are_still_drained() -> Result<(), String> {
    let bytes_read = Rc::new(Cell::new(0));
    let reader = MeteredReader {
        inner: io::repeat(b'x').take(256 * 1024),
        bytes_read: Rc::clone(&bytes_read),
        largest_read: Rc::new(Cell::new(0)),
    };
    let stderr =
        revision_git::read_batch_stderr_for_test(reader).map_err(|error| error.to_string())?;
    if bytes_read.get() != 256 * 1024
        || stderr.len() > 64 * 1024 + 64
        || !stderr.ends_with(b"[additional Git diagnostics omitted]\n")
    {
        return Err(format!(
            "stderr either retained excess bytes or stopped draining: read={}, retained={}",
            bytes_read.get(),
            stderr.len(),
        ));
    }
    Ok(())
}
