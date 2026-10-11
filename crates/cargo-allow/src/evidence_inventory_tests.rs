use super::*;
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use allow_policy::{EvidenceReferenceCategory, EvidenceReferenceStatus};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::sync::atomic::{AtomicUsize, Ordering};

const FIXTURE_ALLOCATION_ATTEMPTS: usize = 128;
static FIXTURE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

fn fixture_dir() -> io::Result<PathBuf> {
    reserve_fixture_dir(&std::env::temp_dir(), &FIXTURE_SEQUENCE)
}

fn reserve_fixture_dir(parent: &Path, sequence: &AtomicUsize) -> io::Result<PathBuf> {
    for _ in 0..FIXTURE_ALLOCATION_ATTEMPTS {
        let unique = sequence.fetch_add(1, Ordering::Relaxed);
        let root = parent.join(format!(
            "cargo-allow-evidence-inventory-{}-{unique}",
            std::process::id()
        ));
        // The counter chooses candidates; only atomic creation grants
        // ownership. Never reuse or remove an occupied candidate.
        match fs::create_dir(&root) {
            Ok(()) => return Ok(root),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("exhausted {FIXTURE_ALLOCATION_ATTEMPTS} exclusive fixture directory candidates"),
    ))
}

fn require_fixture(condition: bool, message: &str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}

#[test]
fn fixture_allocator_reserves_distinct_roots_under_concurrent_collision() -> io::Result<()> {
    let parent = fixture_dir()?;
    let prior = reserve_fixture_dir(&parent, &AtomicUsize::new(0))?;
    let canary = prior.join("owner");
    fs::write(&canary, b"prior owner")?;
    let start = Barrier::new(4);
    let roots = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    // Each contender starts at the occupied name, forcing
                    // the actual allocator to handle collisions without clocks.
                    let sequence = AtomicUsize::new(0);
                    start.wait();
                    reserve_fixture_dir(&parent, &sequence)
                })
            })
            .collect();
        let mut roots = Vec::new();
        for handle in handles {
            roots.push(
                handle
                    .join()
                    .map_err(|_| io::Error::other("fixture allocator thread panicked"))??,
            );
        }
        Ok::<_, io::Error>(roots)
    })?;
    let distinct: BTreeSet<_> = roots.iter().collect();
    require_fixture(
        distinct.len() == 4 && !distinct.contains(&prior),
        "each concurrent allocator must exclusively own a different directory",
    )?;
    let mut owners = Vec::new();
    for (index, root) in roots.into_iter().enumerate() {
        let marker = format!("owner {index}");
        fs::write(root.join("owner"), &marker)?;
        owners.push((root, marker));
    }
    while let Some((root, _)) = owners.pop() {
        fs::remove_dir_all(root)?;
        for (peer, marker) in &owners {
            require_fixture(
                fs::read(peer.join("owner"))? == marker.as_bytes(),
                "removing one fixture must preserve every remaining concurrent owner",
            )?;
        }
        require_fixture(
            fs::read(&canary)? == b"prior owner",
            "allocating and removing competing fixtures must preserve the prior owner",
        )?;
    }
    fs::remove_dir_all(parent)
}

#[test]
fn fixture_allocator_preserves_file_collisions_and_stops_on_other_errors() -> io::Result<()> {
    let parent = fixture_dir()?;
    let occupied = reserve_fixture_dir(&parent, &AtomicUsize::new(0))?;
    fs::remove_dir(&occupied)?;
    fs::write(&occupied, b"file owner")?;

    let sequence = AtomicUsize::new(0);
    let root = reserve_fixture_dir(&parent, &sequence)?;
    require_fixture(
        root != occupied && root.is_dir() && sequence.load(Ordering::Relaxed) == 2,
        "an occupied file must be skipped without claiming it as a directory",
    )?;
    require_fixture(
        fs::read(&occupied)? == b"file owner",
        "a file collision must preserve the existing file bytes",
    )?;

    let blocked_sequence = AtomicUsize::new(0);
    let error = reserve_fixture_dir(&occupied, &blocked_sequence)
        .err()
        .ok_or_else(|| io::Error::other("a non-directory parent must fail allocation"))?;
    require_fixture(
        error.kind() != io::ErrorKind::AlreadyExists
            && blocked_sequence.load(Ordering::Relaxed) == 1,
        "an error other than AlreadyExists must return immediately without retries",
    )?;
    require_fixture(
        fs::read(&occupied)? == b"file owner",
        "a non-directory parent must remain unchanged after allocation fails",
    )?;
    fs::remove_dir_all(parent)
}

#[test]
fn fixture_allocator_exhaustion_preserves_every_existing_owner() -> io::Result<()> {
    let parent = fixture_dir()?;
    let sequence = AtomicUsize::new(0);
    let mut owners = Vec::new();
    for _ in 0..FIXTURE_ALLOCATION_ATTEMPTS {
        let root = reserve_fixture_dir(&parent, &sequence)?;
        fs::write(root.join("owner"), b"existing owner")?;
        owners.push(root);
    }

    let exhausted_sequence = AtomicUsize::new(0);
    let error = reserve_fixture_dir(&parent, &exhausted_sequence)
        .err()
        .ok_or_else(|| io::Error::other("an exhausted collision budget must fail allocation"))?;
    require_fixture(
        error.kind() == io::ErrorKind::AlreadyExists
            && exhausted_sequence.load(Ordering::Relaxed) == FIXTURE_ALLOCATION_ATTEMPTS,
        "occupied candidates must exhaust exactly the bounded allocation budget",
    )?;
    require_fixture(
        fs::read_dir(&parent)?
            .collect::<io::Result<Vec<_>>>()?
            .len()
            == FIXTURE_ALLOCATION_ATTEMPTS,
        "exhaustion must not create any extra fixture directory",
    )?;
    for owner in owners {
        require_fixture(
            fs::read(owner.join("owner"))? == b"existing owner",
            "exhaustion must not replace or remove any existing owner",
        )?;
    }
    fs::remove_dir_all(parent)
}

fn test_entry(id: &str, evidence: Vec<&str>, links: Vec<&str>) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::PolicyException,
        family: Some("network_destination".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "security".to_string(),
        classification: "reviewed".to_string(),
        reason: "Network exception is reviewed.".to_string(),
        evidence: evidence.into_iter().map(str::to_string).collect(),
        links: links.into_iter().map(str::to_string).collect(),
        occurrence_limit: Some(1),
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("function".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

#[test]
fn evidence_reference_diagnostics_for_source_tree_downgrades_present_file_outside_inventory()
-> io::Result<()> {
    let root = fixture_dir()?;
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let entry = test_entry("allow-network", vec!["doc:docs/untracked.md"], vec![]);
    let source_tree_files = BTreeSet::new();

    let diagnostics =
        evidence_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(diagnostic.raw, "doc:docs/untracked.md");
    assert_eq!(diagnostic.status, EvidenceReferenceStatus::LocalFileMissing);
    assert_eq!(diagnostic.category, EvidenceReferenceCategory::Missing);
    assert_eq!(
        diagnostic.message,
        DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE
    );
    assert_eq!(
        diagnostic.target.as_deref(),
        Some(Path::new("docs/untracked.md"))
    );
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}

#[test]
fn evidence_reference_diagnostics_for_source_tree_preserves_inventory_members() -> io::Result<()> {
    let root = fixture_dir()?;
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/tracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let entry = test_entry("allow-network", vec!["doc:docs/tracked.md"], vec![]);
    let mut source_tree_files = BTreeSet::new();
    source_tree_files.insert("docs/tracked.md".to_string());

    let diagnostics =
        evidence_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(diagnostic.status, EvidenceReferenceStatus::LocalFilePresent);
    assert_eq!(diagnostic.category, EvidenceReferenceCategory::Present);
    assert_eq!(diagnostic.message, "local evidence file exists");
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}

#[test]
fn evidence_reference_diagnostics_for_source_tree_skips_inventory_when_unavailable()
-> io::Result<()> {
    let root = fixture_dir()?;
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/local.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let entry = test_entry("allow-network", vec!["doc:docs/local.md"], vec![]);

    let diagnostics = evidence_reference_diagnostics_for_source_tree(&root, &entry, None);

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(diagnostic.status, EvidenceReferenceStatus::LocalFilePresent);
    assert_eq!(diagnostic.category, EvidenceReferenceCategory::Present);
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}

#[test]
fn evidence_reference_rejects_directory_target_as_invalid_local_path() -> io::Result<()> {
    // #1949: a directory path used as evidence (e.g. `doc:docs/`) must be
    // flagged as InvalidLocalPath, not silently treated as valid. The base
    // evidence_reference_diagnostic function catches this at the metadata
    // level (Ok(_) => InvalidLocalPath "exists but is not a file").
    let root = fixture_dir()?;
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));

    let entry = test_entry("allow-dir-evidence", vec!["doc:docs"], vec![]);
    let source_tree_files = BTreeSet::new();

    let diagnostics =
        evidence_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one evidence diagnostic"));
    assert_eq!(
        diagnostic.status,
        EvidenceReferenceStatus::InvalidLocalPath,
        "directory evidence target should be InvalidLocalPath: {:?}",
        diagnostic
    );
    assert_eq!(
        diagnostic.message,
        "local evidence path exists but is not a file"
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}

#[test]
fn policy_reference_diagnostics_for_source_tree_applies_inventory_to_links() -> io::Result<()> {
    let root = fixture_dir()?;
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/trace.md"), "traceability")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture link file: {err}")));
    let entry = test_entry("allow-network", vec![], vec!["doc:docs/trace.md"]);
    let source_tree_files = BTreeSet::new();

    let references =
        policy_reference_diagnostics_for_source_tree(&root, &entry, Some(&source_tree_files));

    assert_eq!(references.len(), 1);
    let reference = references
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one policy reference"));
    assert_eq!(reference.source, ReferenceSource::Link);
    assert_eq!(
        reference.diagnostic.status,
        EvidenceReferenceStatus::LocalFileMissing
    );
    assert_eq!(
        reference.diagnostic.message,
        DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE
    );
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}

#[test]
fn validate_evidence_references_for_source_tree_returns_ok_when_inventory_clean() -> io::Result<()>
{
    let root = fixture_dir()?;
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/tracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(test_entry(
        "allow-network",
        vec!["doc:docs/tracked.md"],
        vec![],
    ));
    let mut source_tree_files = BTreeSet::new();
    source_tree_files.insert("docs/tracked.md".to_string());

    let result =
        validate_evidence_references_for_source_tree(&root, &cfg, Some(&source_tree_files));

    assert!(result.is_ok());
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}

#[test]
fn validate_evidence_references_for_source_tree_returns_err_for_missing_local_files()
-> io::Result<()> {
    let root = fixture_dir()?;
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture root dir: {err}")));
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(test_entry(
        "allow-network",
        vec!["doc:docs/missing.md"],
        vec![],
    ));
    let source_tree_files = BTreeSet::new();

    let err = validate_evidence_references_for_source_tree(&root, &cfg, Some(&source_tree_files))
        .expect_err("missing local evidence should fail validation");

    let message = err.to_string();
    assert!(message.contains("allow-network"));
    assert!(message.contains("doc:docs/missing.md"));
    assert!(message.contains("local evidence file is missing"));
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}

#[test]
fn validate_evidence_references_for_source_tree_returns_err_for_outside_inventory_evidence()
-> io::Result<()> {
    let root = fixture_dir()?;
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(test_entry(
        "allow-network",
        vec!["doc:docs/untracked.md"],
        vec![],
    ));
    let source_tree_files = BTreeSet::new();

    let err = validate_evidence_references_for_source_tree(&root, &cfg, Some(&source_tree_files))
        .expect_err("untracked local evidence should fail default inventory validation");

    let message = err.to_string();
    assert!(message.contains("allow-network"));
    assert!(message.contains("doc:docs/untracked.md"));
    assert!(message.contains(DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE));
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    Ok(())
}
