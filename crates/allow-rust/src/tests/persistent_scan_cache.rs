//! Persistent content-addressed scan-cache coverage (#2571).

use super::*;
use allow_core::FindingKind;
use std::time::{Duration, SystemTime};

/// Monotonic-counter temp root: the system clock is coarse on Windows, so
/// timestamp-only names collide across tests running in the same tick.
fn temp_root(label: &str) -> PathBuf {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_parent = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    let root = temp_parent.join(format!(
        "allow-rust-scan-cache-{label}-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture: {err}")));
    root
}

fn set_mtime(path: &Path, mtime: SystemTime) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("open {}: {err}", path.display())));
    file.set_times(std::fs::FileTimes::new().set_modified(mtime))
        .unwrap_or_else(|err| std::panic::panic_any(format!("set mtime: {err}")));
}

const CLEAN_SOURCE: &str = "fn f(value: Result<(), ()>) { let _ = value;      }\n";
// Same byte length as CLEAN_SOURCE with `let _ = value;` swapped for an
// unwrap call, so a size-preserving content swap cannot hide behind mtime.
const UNWRAP_SOURCE: &str = "fn f(value: Result<(), ()>) {     value.unwrap(); }\n";

#[test]
fn persistent_store_round_trips_findings_across_instances() {
    let root = temp_root("round-trip");
    let source_path = root.join("src/lib.rs");
    fs::write(&source_path, UNWRAP_SOURCE)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write: {err}")));
    let dir = ScanCacheStore::default_dir(&root);
    let generation = scan_cache_generation();

    let mut cache = ScanCache::new();
    let mut store = ScanCacheStore::open(&dir, &generation);
    let files = vec![PathBuf::from("src/lib.rs")];
    let first = scan_rust_files_cached_with_store(&root, &files, &mut cache, &mut store)
        .unwrap_or_else(|err| std::panic::panic_any(format!("cold scan: {err}")));
    assert!(
        first
            .findings
            .iter()
            .any(|finding| finding.kind == FindingKind::Panic),
        "cold scan must find the unwrap"
    );
    assert!(store.flush(), "first flush should persist");

    // A brand-new process equivalent: fresh cache, freshly opened store.
    let mut cache2 = ScanCache::new();
    let mut store2 = ScanCacheStore::open(&dir, &generation);
    assert!(!store2.is_empty(), "reopened store must load entries");
    let second = scan_rust_files_cached_with_store(&root, &files, &mut cache2, &mut store2)
        .unwrap_or_else(|err| std::panic::panic_any(format!("warm scan: {err}")));
    assert_eq!(first.findings, second.findings);
    assert_eq!(first.file_statuses, second.file_statuses);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn preserved_mtime_cannot_mask_changed_content() {
    let root = temp_root("content-invalidation");
    let source_path = root.join("src/lib.rs");
    assert_eq!(
        CLEAN_SOURCE.len(),
        UNWRAP_SOURCE.len(),
        "fixture premise: equal-size swap"
    );
    fs::write(&source_path, CLEAN_SOURCE)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write: {err}")));
    let dir = ScanCacheStore::default_dir(&root);
    let generation = scan_cache_generation();
    let files = vec![PathBuf::from("src/lib.rs")];

    let mut cache = ScanCache::new();
    let mut store = ScanCacheStore::open(&dir, &generation);
    let cold = scan_rust_files_cached_with_store(&root, &files, &mut cache, &mut store)
        .unwrap_or_else(|err| std::panic::panic_any(format!("cold scan: {err}")));
    assert!(!cold.findings.iter().any(|f| f.kind == FindingKind::Panic));
    assert!(store.flush());

    // Swap content for an unwrap while restoring the ORIGINAL mtime and the
    // ORIGINAL size: only a content-digest key can invalidate this correctly.
    let original_mtime = fs::metadata(&source_path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    fs::write(&source_path, UNWRAP_SOURCE)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rewrite: {err}")));
    set_mtime(&source_path, original_mtime);

    let mut cache2 = ScanCache::new();
    let mut store2 = ScanCacheStore::open(&dir, &generation);
    let warm = scan_rust_files_cached_with_store(&root, &files, &mut cache2, &mut store2)
        .unwrap_or_else(|err| std::panic::panic_any(format!("warm scan: {err}")));
    assert!(
        warm.findings
            .iter()
            .any(|finding| finding.kind == FindingKind::Panic),
        "digest authority must surface changed content despite preserved mtime+size"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mtime_churn_without_content_change_keeps_facts_valid() {
    let root = temp_root("mtime-churn");
    let source_path = root.join("src/lib.rs");
    fs::write(&source_path, UNWRAP_SOURCE)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write: {err}")));
    let dir = ScanCacheStore::default_dir(&root);
    let generation = scan_cache_generation();
    let files = vec![PathBuf::from("src/lib.rs")];

    let mut cache = ScanCache::new();
    let mut store = ScanCacheStore::open(&dir, &generation);
    let cold = scan_rust_files_cached_with_store(&root, &files, &mut cache, &mut store)
        .unwrap_or_else(|err| std::panic::panic_any(format!("cold scan: {err}")));
    assert!(store.flush());
    let persisted_digest_hit = {
        allow_core::read_text_file_capped(&source_path)
            .ok()
            .and_then(|text| {
                store.get(
                    Path::new("src/lib.rs"),
                    &allow_core::sha256_v1_bytes(text.as_bytes()),
                )
            })
            .is_some()
    };
    assert!(persisted_digest_hit, "facts must be persisted by digest");

    // Churn the mtime far into the future; facts stay valid because the key
    // is content, not timestamps.
    let churned = SystemTime::UNIX_EPOCH + Duration::from_secs(4_102_444_800);
    set_mtime(&source_path, churned);

    let mut cache2 = ScanCache::new();
    let mut store2 = ScanCacheStore::open(&dir, &generation);
    let warm = scan_rust_files_cached_with_store(&root, &files, &mut cache2, &mut store2)
        .unwrap_or_else(|err| std::panic::panic_any(format!("warm scan: {err}")));
    assert_eq!(cold.findings, warm.findings);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn scanner_generation_change_invalidates_every_entry() {
    let root = temp_root("generation");
    fs::write(root.join("src/lib.rs"), UNWRAP_SOURCE)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write: {err}")));
    let dir = ScanCacheStore::default_dir(&root);
    let files = vec![PathBuf::from("src/lib.rs")];

    let mut cache = ScanCache::new();
    let mut store = ScanCacheStore::open(&dir, "allow-rust:test-gen-a");
    let _ = scan_rust_files_cached_with_store(&root, &files, &mut cache, &mut store)
        .unwrap_or_else(|err| std::panic::panic_any(format!("gen-a scan: {err}")));
    assert!(store.flush());

    let reopened = ScanCacheStore::open(&dir, "allow-rust:test-gen-b");
    assert!(
        reopened.is_empty(),
        "a different scanner generation must discard all durable entries"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn corrupt_store_degrades_to_cold_scan_with_identical_results() {
    let root = temp_root("corrupt-store");
    fs::write(root.join("src/lib.rs"), UNWRAP_SOURCE)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write: {err}")));
    let dir = ScanCacheStore::default_dir(&root);
    let generation = scan_cache_generation();
    let files = vec![PathBuf::from("src/lib.rs")];

    let mut cache = ScanCache::new();
    let mut store = ScanCacheStore::open(&dir, &generation);
    let cold = scan_rust_files_cached_with_store(&root, &files, &mut cache, &mut store)
        .unwrap_or_else(|err| std::panic::panic_any(format!("reference scan: {err}")));
    assert!(store.flush());
    let store_path = dir.join("scan-cache.v2.bin");

    for damaged in [
        Vec::new(),
        b"cargo-allow.scan-cache.v2\n".to_vec(),
        b"garbage-not-a-store".to_vec(),
    ] {
        fs::write(&store_path, &damaged)
            .unwrap_or_else(|err| std::panic::panic_any(format!("corrupt store: {err}")));
        let mut cache_damaged = ScanCache::new();
        let store_damaged = ScanCacheStore::open(&dir, &generation);
        assert!(
            store_damaged.is_empty(),
            "damaged store must fail open to empty"
        );
        let mut sink = ScanCacheStore::open(&dir, &generation);
        let rescanned =
            scan_rust_files_cached_with_store(&root, &files, &mut cache_damaged, &mut sink)
                .unwrap_or_else(|err| std::panic::panic_any(format!("post-corruption: {err}")));
        assert_eq!(cold.findings, rescanned.findings);
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn skipped_files_are_never_persisted() {
    let root = temp_root("skipped");
    // Invalid UTF-8 makes read_text_file_capped fail -> skipped status.
    let bad = root.join("src/bad.rs");
    fs::write(&bad, [b'f', b'n', 0xFF, b'(', b')', b'{', b'}'])
        .unwrap_or_else(|err| std::panic::panic_any(format!("write non-utf8: {err}")));
    let good = root.join("src/good.rs");
    fs::write(&good, UNWRAP_SOURCE)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write: {err}")));
    let dir = ScanCacheStore::default_dir(&root);
    let generation = scan_cache_generation();
    let files = vec![PathBuf::from("src/bad.rs"), PathBuf::from("src/good.rs")];

    let mut cache = ScanCache::new();
    let mut store = ScanCacheStore::open(&dir, &generation);
    let result = scan_rust_files_cached_with_store(&root, &files, &mut cache, &mut store)
        .unwrap_or_else(|err| std::panic::panic_any(format!("scan: {err}")));
    assert_eq!(
        result.files_skipped, 1,
        "the non-UTF-8 file must be skipped"
    );
    assert!(store.flush());

    let reopened = ScanCacheStore::open(&dir, &generation);
    assert_eq!(
        reopened.len(),
        1,
        "only the evaluated file may be persisted; skipped files carry no reusable facts"
    );

    fs::write(&good, [b'f', b'n', 0xFF, b'(', b')', b'{', b'}'])
        .unwrap_or_else(|err| std::panic::panic_any(format!("rewrite non-utf8: {err}")));
    let mut cache2 = ScanCache::new();
    let mut store2 = ScanCacheStore::open(&dir, &generation);
    let _ = scan_rust_files_cached_with_store(&root, &files, &mut cache2, &mut store2)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rescan skipped: {err}")));
    assert!(store2.flush());
    let reopened_after_skip = ScanCacheStore::open(&dir, &generation);
    assert!(
        reopened_after_skip.is_empty(),
        "a previously cached path must be pruned when it becomes unreadable"
    );

    let _ = fs::remove_dir_all(&root);
}
