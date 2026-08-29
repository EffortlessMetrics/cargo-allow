//! Root-bound persistent scan-cache coverage (#3915).

use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = "fn load(value: Result<(), ()>) { value.unwrap(); }\n";

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "allow-rust-cache-root-alias-{label}-{}-{stamp}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(Self(root))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn prepare_source(root: &Path) -> Result<Vec<PathBuf>, String> {
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), SOURCE).map_err(|error| error.to_string())?;
    Ok(vec![PathBuf::from("src/lib.rs")])
}

fn dirty_root_bound_store(
    root: &Path,
    files: &[PathBuf],
) -> Result<RootBoundScanCacheStore, String> {
    let mut store = RootBoundScanCacheStore::open(root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(root, files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;
    Ok(store)
}

fn assert_scan_parity(left: &RustScanResult, right: &RustScanResult) {
    assert_eq!(left.findings, right.findings);
    assert_eq!(left.file_statuses, right.file_statuses);
    assert_eq!(left.files_considered, right.files_considered);
    assert_eq!(left.files_skipped, right.files_skipped);
    assert_eq!(left.files_with_parse_errors, right.files_with_parse_errors);
}

#[test]
fn root_bound_store_preserves_cached_and_uncached_scan_semantics() -> Result<(), String> {
    let fixture = TempRoot::new("parity")?;
    let files = prepare_source(&fixture.0)?;

    let mut uncached_memory = ScanCache::new();
    let uncached = scan_rust_files_cached(&fixture.0, &files, &mut uncached_memory)
        .map_err(|error| error.to_string())?;

    let generation = scan_cache_generation();
    let mut store = RootBoundScanCacheStore::open(&fixture.0, &generation)
        .map_err(|disposition| disposition.as_str().to_string())?;
    assert!(matches!(
        store.root_disposition(),
        ScanCacheTargetDispositionV1::ExactRepositoryRoot
            | ScanCacheTargetDispositionV1::BenignExternalRootAlias
    ));
    assert_eq!(
        store.target_disposition(),
        ScanCacheTargetDispositionV1::SafeOwnedDescendant
    );
    let mut first_memory = ScanCache::new();
    let first = scan_rust_files_cached_with_root_bound_store(
        &fixture.0,
        &files,
        &mut first_memory,
        &mut store,
    )
    .map_err(|error| error.to_string())?;
    assert_scan_parity(&uncached, &first);
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;

    let mut reopened = RootBoundScanCacheStore::open(&fixture.0, &generation)
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut second_memory = ScanCache::new();
    let second = scan_rust_files_cached_with_root_bound_store(
        &fixture.0,
        &files,
        &mut second_memory,
        &mut reopened,
    )
    .map_err(|error| error.to_string())?;
    assert_scan_parity(&uncached, &second);
    Ok(())
}

#[test]
fn root_bound_store_replaces_an_existing_destination_on_second_dirty_flush() -> Result<(), String> {
    let fixture = TempRoot::new("second-dirty-flush")?;
    let files = prepare_source(&fixture.0)?;
    let generation = scan_cache_generation();
    let mut store = RootBoundScanCacheStore::open(&fixture.0, &generation)
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();

    scan_rust_files_cached_with_root_bound_store(&fixture.0, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;
    let store_path = ScanCacheStore::default_dir(&fixture.0).join("scan-cache.v2.bin");
    let first_bytes = fs::read(&store_path).map_err(|error| error.to_string())?;

    fs::write(
        fixture.0.join("src/lib.rs"),
        "fn load(value: Result<(), ()>) { value.expect(\"changed\"); }\n",
    )
    .map_err(|error| error.to_string())?;
    let mut expected_memory = ScanCache::new();
    let expected = scan_rust_files_cached(&fixture.0, &files, &mut expected_memory)
        .map_err(|error| error.to_string())?;
    let second =
        scan_rust_files_cached_with_root_bound_store(&fixture.0, &files, &mut memory, &mut store)
            .map_err(|error| error.to_string())?;
    assert_scan_parity(&expected, &second);
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;
    let second_bytes = fs::read(&store_path).map_err(|error| error.to_string())?;
    assert_ne!(
        first_bytes, second_bytes,
        "second dirty flush must replace bytes"
    );
    drop(store);

    let mut reopened = RootBoundScanCacheStore::open(&fixture.0, &generation)
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut reopened_memory = ScanCache::new();
    let persisted = scan_rust_files_cached_with_root_bound_store(
        &fixture.0,
        &files,
        &mut reopened_memory,
        &mut reopened,
    )
    .map_err(|error| error.to_string())?;
    assert_scan_parity(&expected, &persisted);
    Ok(())
}

#[test]
fn injected_temp_artifact_is_a_destination_change_not_an_instrument_failure() -> Result<(), String>
{
    let fixture = TempRoot::new("temp-injection")?;
    let files = prepare_source(&fixture.0)?;
    let mut store = dirty_root_bound_store(&fixture.0, &files)?;

    let result = store.flush_with_test_hook(&|store_dir| {
        let injected = store_dir.join("scan-cache.v2.bin.tmp-injected");
        fs::write(&injected, b"replacement temp").unwrap_or_else(|error| {
            std::panic::panic_any(format!("write {}: {error}", injected.display()))
        });
    });

    assert_eq!(
        result,
        Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange)
    );
    assert!(
        !ScanCacheStore::default_dir(&fixture.0)
            .join("scan-cache.v2.bin")
            .exists()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn replaced_lock_file_is_a_destination_change_not_an_instrument_failure() -> Result<(), String> {
    let fixture = TempRoot::new("lock-replacement")?;
    let files = prepare_source(&fixture.0)?;
    let mut store = dirty_root_bound_store(&fixture.0, &files)?;

    let result = store.flush_with_test_hook(&|store_dir| {
        let lock = store_dir.join("scan-cache.v2.lock");
        let original = store_dir.join("scan-cache.v2.lock.original");
        fs::rename(&lock, &original).unwrap_or_else(|error| {
            std::panic::panic_any(format!(
                "rename {} to {}: {error}",
                lock.display(),
                original.display()
            ))
        });
        fs::write(&lock, b"replacement lock").unwrap_or_else(|error| {
            std::panic::panic_any(format!("write {}: {error}", lock.display()))
        });
    });

    assert_eq!(
        result,
        Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange)
    );
    assert!(
        !ScanCacheStore::default_dir(&fixture.0)
            .join("scan-cache.v2.bin")
            .exists()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn lock_symlink_back_to_bound_inode_is_still_rejected() -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let fixture = TempRoot::new("lock-symlink")?;
    let files = prepare_source(&fixture.0)?;
    let mut store = dirty_root_bound_store(&fixture.0, &files)?;

    let result = store.flush_with_test_hook(&|store_dir| {
        let lock = store_dir.join("scan-cache.v2.lock");
        let original = store_dir.join("scan-cache.v2.lock.original");
        fs::rename(&lock, &original).unwrap_or_else(|error| {
            std::panic::panic_any(format!(
                "rename {} to {}: {error}",
                lock.display(),
                original.display()
            ))
        });
        symlink(&original, &lock).unwrap_or_else(|error| {
            std::panic::panic_any(format!(
                "symlink {} to {}: {error}",
                lock.display(),
                original.display()
            ))
        });
    });

    assert_eq!(
        result,
        Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange)
    );
    assert!(
        !ScanCacheStore::default_dir(&fixture.0)
            .join("scan-cache.v2.bin")
            .exists()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn replaced_destination_is_a_destination_change_not_an_instrument_failure() -> Result<(), String> {
    let fixture = TempRoot::new("destination-replacement")?;
    let files = prepare_source(&fixture.0)?;
    let mut store = dirty_root_bound_store(&fixture.0, &files)?;
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;

    fs::write(
        fixture.0.join("src/lib.rs"),
        "fn load(value: Result<(), ()>) { value.expect(\"changed\"); }\n",
    )
    .map_err(|error| error.to_string())?;
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(&fixture.0, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;

    let result = store.flush_with_test_hook(&|store_dir| {
        let destination = store_dir.join("scan-cache.v2.bin");
        let original = store_dir.join("scan-cache.v2.bin.original");
        fs::rename(&destination, &original).unwrap_or_else(|error| {
            std::panic::panic_any(format!(
                "rename {} to {}: {error}",
                destination.display(),
                original.display()
            ))
        });
        fs::write(&destination, b"replacement destination").unwrap_or_else(|error| {
            std::panic::panic_any(format!("write {}: {error}", destination.display()))
        });
    });

    assert_eq!(
        result,
        Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange)
    );
    let replacement = fs::read(ScanCacheStore::default_dir(&fixture.0).join("scan-cache.v2.bin"))
        .map_err(|error| error.to_string())?;
    assert_eq!(replacement, b"replacement destination");
    Ok(())
}

#[cfg(unix)]
#[test]
fn external_alias_is_admitted_without_authorizing_in_root_aliases() -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let fixture = TempRoot::new("explicit")?;
    let real_parent = fixture.0.join("real");
    let alias_parent = fixture.0.join("alias");
    let real_root = real_parent.join("repo");
    let files = prepare_source(&real_root)?;
    symlink(&real_parent, &alias_parent).map_err(|error| error.to_string())?;
    let requested_root = alias_parent.join("repo");

    let requested_dir = ScanCacheStore::default_dir(&requested_root);
    let mut strict_store = ScanCacheStore::open(&requested_dir, "generation");
    strict_store.put(
        Path::new("src/lib.rs"),
        "digest".to_string(),
        false,
        Vec::new(),
    );
    assert!(
        !strict_store.flush(),
        "the unbounded compatibility constructor remains strict"
    );
    assert!(!requested_dir.join("scan-cache.v2.bin").exists());

    let mut root_bound = RootBoundScanCacheStore::open(&requested_root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    assert_eq!(
        root_bound.root_disposition(),
        ScanCacheTargetDispositionV1::BenignExternalRootAlias
    );
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(
        &requested_root,
        &files,
        &mut memory,
        &mut root_bound,
    )
    .map_err(|error| error.to_string())?;
    root_bound
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;
    assert!(requested_dir.join("scan-cache.v2.bin").exists());
    drop(root_bound);

    fs::remove_dir_all(real_root.join("target")).map_err(|error| error.to_string())?;
    let outside = fixture.0.join("outside");
    fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
    symlink(&outside, real_root.join("target")).map_err(|error| error.to_string())?;
    let rejected = RootBoundScanCacheStore::open(&requested_root, "generation");
    assert!(matches!(
        rejected,
        Err(ScanCacheTargetDispositionV1::InRootSymlinkOrReparseEscape)
    ));
    assert!(!outside.join("cargo-allow/cache/scan-cache.v2.bin").exists());
    Ok(())
}

#[test]
fn changed_root_identity_fails_before_persistence() -> Result<(), String> {
    let fixture = TempRoot::new("root-change")?;
    let root = fixture.0.join("repo");
    let moved = fixture.0.join("repo-moved");
    let files = prepare_source(&root)?;
    let mut store = RootBoundScanCacheStore::open(&root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(&root, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;

    fs::rename(&root, &moved).map_err(|error| error.to_string())?;
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    assert_eq!(
        store.flush_with_disposition(),
        Err(ScanCacheTargetDispositionV1::RootIdentityChanged)
    );
    assert!(
        !root
            .join("target/cargo-allow/cache/scan-cache.v2.bin")
            .exists()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn changed_cache_directory_identity_fails_before_persistence() -> Result<(), String> {
    let fixture = TempRoot::new("cache-dir-change")?;
    let root = fixture.0.join("repo");
    let files = prepare_source(&root)?;
    let mut store = RootBoundScanCacheStore::open(&root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(&root, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;

    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Result<(), ()>) { value.expect(\"changed\"); }\n",
    )
    .map_err(|error| error.to_string())?;
    scan_rust_files_cached_with_root_bound_store(&root, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;

    let cache_dir = ScanCacheStore::default_dir(&root);
    let moved = root.join("moved-cache");
    fs::rename(&cache_dir, &moved).map_err(|error| error.to_string())?;
    fs::create_dir_all(&cache_dir).map_err(|error| error.to_string())?;
    assert_eq!(
        store.flush_with_disposition(),
        Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange)
    );
    assert!(!cache_dir.join("scan-cache.v2.bin").exists());
    assert!(moved.join("scan-cache.v2.bin").exists());
    Ok(())
}

#[test]
fn unsupported_or_missing_roots_never_admit_persistence() -> Result<(), String> {
    let fixture = TempRoot::new("invalid-root")?;
    let missing = fixture.0.join("missing");
    assert!(matches!(
        RootBoundScanCacheStore::open(&missing, "generation"),
        Err(ScanCacheTargetDispositionV1::InstrumentFailure)
    ));

    let file = fixture.0.join("not-a-directory");
    fs::write(&file, b"not a repository root").map_err(|error| error.to_string())?;
    assert!(matches!(
        RootBoundScanCacheStore::open(&file, "generation"),
        Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem)
    ));
    assert!(matches!(
        RootBoundScanCacheStore::open(Path::new("relative-root"), "generation"),
        Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem)
    ));
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_junction_inside_root_remains_rejected() -> Result<(), String> {
    use std::process::Command;

    let fixture = TempRoot::new("windows-junction")?;
    let root = fixture.0.join("repo");
    let outside = fixture.0.join("outside");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
    let junction = root.join("target");
    let output = Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&junction)
        .arg(&outside)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "mklink /J failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    assert!(matches!(
        RootBoundScanCacheStore::open(&root, "generation"),
        Err(ScanCacheTargetDispositionV1::InRootSymlinkOrReparseEscape)
    ));
    fs::remove_dir(&junction).map_err(|error| error.to_string())?;
    assert!(!outside.join("cargo-allow/cache/scan-cache.v2.bin").exists());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn macos_temp_root_accepts_hosted_alias_when_present() -> Result<(), String> {
    let fixture = TempRoot::new("macos-hosted")?;
    let files = prepare_source(&fixture.0)?;
    let canonical_root = fixture
        .0
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut store = RootBoundScanCacheStore::open(&fixture.0, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let expected = if fixture.0 == canonical_root {
        ScanCacheTargetDispositionV1::ExactRepositoryRoot
    } else {
        ScanCacheTargetDispositionV1::BenignExternalRootAlias
    };
    assert_eq!(store.root_disposition(), expected);
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(&fixture.0, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;
    assert!(
        ScanCacheStore::default_dir(&canonical_root)
            .join("scan-cache.v2.bin")
            .exists()
    );
    Ok(())
}

/// Prove the detect-after boundary the root-bound cache law documents: a
/// cache-path component swapped for indirection between the flush walk's
/// verification and the write must be refused, and no store bytes may move
/// through the swap. The injection hook lands after the walk and before the
/// write, exactly where prevention cannot reach and detection must.
#[cfg(unix)]
#[test]
fn unix_mid_walk_component_swap_is_detected_and_refused() -> Result<(), String> {
    let fixture = TempRoot::new("mid-walk-swap")?;
    let root = fixture.0.join("repo");
    let outside = fixture.0.join("outside");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
    let files = prepare_source(&root)?;

    // First lifecycle: admit, scan, and flush so the cache directory exists
    // and the second flush walks real components before the swap.
    let mut store = RootBoundScanCacheStore::open(&root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(&root, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;
    drop(store);

    let store_path = ScanCacheStore::default_dir(&root).join("scan-cache.v2.bin");
    let bytes_before = fs::read(&store_path).map_err(|error| error.to_string())?;

    // Second lifecycle with the injection: the hook renames the walked
    // `target` component aside and points the original spelling at outside
    // storage, so any write past this point would land outside the root.
    let mut store = RootBoundScanCacheStore::open(&root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Result<(), ()>) { value.expect(\"changed\"); }\n",
    )
    .map_err(|error| error.to_string())?;
    scan_rust_files_cached_with_root_bound_store(&root, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;

    let real_target = root.join("target.real");
    let alias = root.join("target");
    let result = store.flush_with_test_hook(&|_| {
        fs::rename(&alias, &real_target).unwrap_or_else(|error| {
            std::panic::panic_any(format!("rename {}: {error}", alias.display()))
        });
        std::os::unix::fs::symlink(&outside, &alias).unwrap_or_else(|error| {
            std::panic::panic_any(format!("symlink {}: {error}", alias.display()))
        });
    });

    // Detection: the flush is refused with a typed disposition rather than
    // silently succeeding through the swap.
    assert!(result.is_err(), "swapped component flush must be refused");
    // No content escape: the store behind the renamed component is
    // byte-identical to the last admitted flush, and the outside directory
    // never received a cache artifact.
    let bytes_after = fs::read(&real_target.join("cargo-allow/cache/scan-cache.v2.bin"))
        .map_err(|error| error.to_string())?;
    assert_eq!(bytes_before, bytes_after, "store bytes must not change");
    assert!(
        !outside.join("cargo-allow").exists(),
        "no cache artifact may be created through the swapped component"
    );
    Ok(())
}

/// Windows counterpart: the store's bound directory and lock handles pin
/// the walked tree, so the component rename an attacker needs is refused by
/// the filesystem itself before any indirection can be introduced. Assert
/// that structural refusal and that nothing is written outside the root.
#[cfg(windows)]
#[test]
fn windows_mid_walk_component_swap_is_structurally_refused() -> Result<(), String> {
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};

    static SWAP_DENIED: AtomicBool = AtomicBool::new(false);

    let fixture = TempRoot::new("mid-walk-swap")?;
    let root = fixture.0.join("repo");
    let outside = fixture.0.join("outside");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
    let files = prepare_source(&root)?;

    let mut store = RootBoundScanCacheStore::open(&root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();
    scan_rust_files_cached_with_root_bound_store(&root, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;
    store
        .flush_with_disposition()
        .map_err(|disposition| disposition.as_str().to_string())?;
    drop(store);

    let mut store = RootBoundScanCacheStore::open(&root, "generation")
        .map_err(|disposition| disposition.as_str().to_string())?;
    let mut memory = ScanCache::new();
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Result<(), ()>) { value.expect(\"changed\"); }\n",
    )
    .map_err(|error| error.to_string())?;
    scan_rust_files_cached_with_root_bound_store(&root, &files, &mut memory, &mut store)
        .map_err(|error| error.to_string())?;

    let real_target = root.join("target.real");
    let alias = root.join("target");
    let result = store.flush_with_test_hook(&|_| {
        if fs::rename(&alias, &real_target).is_err() {
            SWAP_DENIED.store(true, Ordering::SeqCst);
            return;
        }
        let output = Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(&alias)
            .arg(&outside)
            .output()
            .unwrap_or_else(|error| std::panic::panic_any(format!("mklink /J: {error}")));
        if !output.status.success() {
            std::panic::panic_any(format!(
                "mklink /J failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    });

    assert!(
        SWAP_DENIED.load(Ordering::SeqCst),
        "windows must refuse renaming the walked cache tree while the store \
         holds its bound handles"
    );
    if let Err(disposition) = result {
        // If a platform ever permits the swap, detection must still refuse.
        assert_ne!(
            disposition.as_str(),
            "",
            "refused swap must carry a typed disposition"
        );
    }
    assert!(
        !outside.join("cargo-allow").exists(),
        "no cache artifact may be created through the swapped component"
    );
    Ok(())
}
