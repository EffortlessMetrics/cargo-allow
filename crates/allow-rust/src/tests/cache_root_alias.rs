//! Characterize pre-root aliases separately from cache-owned path movement (#3915).

#[cfg(unix)]
use super::*;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
struct TempRoot(PathBuf);

#[cfg(unix)]
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

#[cfg(unix)]
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn add_entry(store: &mut ScanCacheStore) {
    store.put(
        Path::new("src/lib.rs"),
        "digest".to_string(),
        false,
        Vec::new(),
    );
}

#[cfg(unix)]
#[test]
fn pre_root_alias_and_in_root_alias_have_distinct_safety_outcomes() -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let fixture = TempRoot::new("explicit")?;
    let real_parent = fixture.0.join("real");
    let alias_parent = fixture.0.join("alias");
    let real_root = real_parent.join("repo");
    fs::create_dir_all(real_root.join("src")).map_err(|error| error.to_string())?;
    symlink(&real_parent, &alias_parent).map_err(|error| error.to_string())?;

    let requested_root = alias_parent.join("repo");
    let canonical_root = requested_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    assert_eq!(
        canonical_root,
        real_root
            .canonicalize()
            .map_err(|error| error.to_string())?,
        "the external alias must resolve to the same selected root"
    );

    let requested_dir = ScanCacheStore::default_dir(&requested_root);
    let mut requested_store = ScanCacheStore::open(&requested_dir, "generation");
    add_entry(&mut requested_store);
    assert!(
        !requested_store.flush(),
        "the current unbounded component walk must characterize the external alias as non-writable"
    );
    assert!(
        !requested_dir.join("scan-cache.v2.bin").exists(),
        "characterization must not write through the aliased spelling"
    );

    let canonical_dir = ScanCacheStore::default_dir(&canonical_root);
    let mut canonical_store = ScanCacheStore::open(&canonical_dir, "generation");
    add_entry(&mut canonical_store);
    assert!(
        canonical_store.flush(),
        "the exact canonical root should retain the existing cache behavior"
    );
    assert!(canonical_dir.join("scan-cache.v2.bin").exists());
    drop(canonical_store);

    fs::remove_dir_all(canonical_root.join("target")).map_err(|error| error.to_string())?;
    let outside = fixture.0.join("outside");
    fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
    symlink(&outside, canonical_root.join("target")).map_err(|error| error.to_string())?;

    let in_root_dir = ScanCacheStore::default_dir(&canonical_root);
    let mut in_root_store = ScanCacheStore::open(&in_root_dir, "generation");
    add_entry(&mut in_root_store);
    assert!(
        !in_root_store.flush(),
        "an alias at or below the selected root must remain fail-closed"
    );
    assert!(
        !outside.join("cargo-allow/cache/scan-cache.v2.bin").exists(),
        "the in-root alias must not redirect cache persistence"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn macos_temp_root_characterizes_hosted_alias_when_present() -> Result<(), String> {
    let fixture = TempRoot::new("macos-hosted")?;
    fs::create_dir_all(fixture.0.join("src")).map_err(|error| error.to_string())?;
    let canonical_root = fixture
        .0
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if fixture.0 == canonical_root {
        // Some local macOS shells expose an already-canonical TMPDIR. The
        // explicit Unix fixture above still covers the root-alias semantics;
        // hosted rows characterize the platform observation when present.
        return Ok(());
    }

    let requested_dir = ScanCacheStore::default_dir(&fixture.0);
    let mut requested_store = ScanCacheStore::open(&requested_dir, "generation");
    add_entry(&mut requested_store);
    assert!(
        !requested_store.flush(),
        "the current guard should reproduce the macOS cache exclusion premise"
    );
    assert!(
        !requested_dir.join("scan-cache.v2.bin").exists(),
        "the aliased macOS spelling must not persist after a failed flush"
    );

    let canonical_dir = ScanCacheStore::default_dir(&canonical_root);
    let mut canonical_store = ScanCacheStore::open(&canonical_dir, "generation");
    add_entry(&mut canonical_store);
    assert!(
        canonical_store.flush(),
        "the same selected root is writable after its external alias is resolved"
    );
    Ok(())
}
