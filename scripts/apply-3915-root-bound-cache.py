from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one source shape, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


lib = Path("crates/allow-rust/src/lib.rs")
replace_once(
    lib,
    "mod scan_cache;\nmod scan_cache_store;\nmod scan_result;",
    "mod scan_cache;\nmod scan_cache_store;\nmod root_bound_scan_cache;\nmod scan_result;",
)
replace_once(
    lib,
    "pub use scan_cache::ScanCache;\npub use scan_cache_store::ScanCacheStore;\npub use scan_result::{RustFileScanOutcome, RustFileScanStatus, RustScanResult};",
    "pub use root_bound_scan_cache::{\n    RootBoundScanCacheStore, ScanCacheTargetDispositionV1,\n};\npub use scan_cache::ScanCache;\npub use scan_cache_store::ScanCacheStore;\npub use scan_result::{RustFileScanOutcome, RustFileScanStatus, RustScanResult};",
)
replace_once(
    lib,
    "\n/// Heuristically detect whether a path is a test-only source file (#1798).",
    """

/// Scan Rust files through a root-bound durable store.
///
/// The wrapper keeps the underlying store private so callers cannot bypass
/// root and destination identity rechecks at the persistence boundary.
#[cfg(feature = "syntax")]
pub fn scan_rust_files_cached_with_root_bound_store(
    root: impl AsRef<Path>,
    files: &[PathBuf],
    cache: &mut ScanCache,
    store: &mut RootBoundScanCacheStore,
) -> CargoAllowResult<RustScanResult> {
    scan_rust_files_cached_with_store(root, files, cache, store.inner_mut())
}

/// Heuristically detect whether a path is a test-only source file (#1798).""",
)

world = Path("crates/cargo-allow/src/world.rs")
replace_once(
    world,
    """fn inventory_options_with_tool_cache_ignore(mut options: InventoryOptions) -> InventoryOptions {
    if !options
        .ignored
        .iter()
        .any(|glob| glob == TOOL_OWNED_CACHE_GLOB)
    {
        options.ignored.push(TOOL_OWNED_CACHE_GLOB.to_string());
    }
    options
}
""",
    """fn inventory_options_with_tool_cache_ignore(mut options: InventoryOptions) -> InventoryOptions {
    if !options
        .ignored
        .iter()
        .any(|glob| glob == TOOL_OWNED_CACHE_GLOB)
    {
        options.ignored.push(TOOL_OWNED_CACHE_GLOB.to_string());
    }
    options
}

fn scan_rust_files_with_cache_mode(
    root: &Path,
    files: &[PathBuf],
    persistent_cache: bool,
) -> CargoAllowResult<allow_rust::RustScanResult> {
    SCAN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if persistent_cache
            && let Ok(mut store) = allow_rust::RootBoundScanCacheStore::open(
                root,
                allow_rust::scan_cache_generation(),
            )
        {
            let result = allow_rust::scan_rust_files_cached_with_root_bound_store(
                root,
                files,
                &mut cache,
                &mut store,
            );
            let _ = store.flush();
            return result;
        }
        allow_rust::scan_rust_files_cached(root, files, &mut cache)
    })
}
""",
)
replace_once(
    world,
    """    let rust_scan = if persistent_cache {
        let mut store = allow_rust::ScanCacheStore::open(
            &allow_rust::ScanCacheStore::default_dir(&root),
            allow_rust::scan_cache_generation(),
        );
        let result = SCAN_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            allow_rust::scan_rust_files_cached_with_store(&root, &files, &mut cache, &mut store)
        });
        let _ = store.flush();
        result
    } else {
        SCAN_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            allow_rust::scan_rust_files_cached(&root, &files, &mut cache)
        })
    };
    let rust_scan = rust_scan?;
""",
    """    let rust_scan = scan_rust_files_with_cache_mode(&root, &files, persistent_cache)?;
""",
)
replace_once(
    world,
    """    let rust_scan = if persistent_cache {
        let mut store = allow_rust::ScanCacheStore::open(
            &allow_rust::ScanCacheStore::default_dir(root),
            allow_rust::scan_cache_generation(),
        );
        let result = SCAN_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            allow_rust::scan_rust_files_cached_with_store(root, &files, &mut cache, &mut store)
        })?;
        let _ = store.flush();
        result
    } else {
        SCAN_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            allow_rust::scan_rust_files_cached(root, &files, &mut cache)
        })?
    };
""",
    """    let rust_scan = scan_rust_files_with_cache_mode(root, &files, persistent_cache)?;
""",
)

persistent = Path("crates/allow-rust/src/tests/persistent_scan_cache.rs")
replace_once(
    persistent,
    """    let root = std::env::temp_dir().join(format!(
        "allow-rust-scan-cache-{label}-{}-{stamp}-{id}",
        std::process::id()
    ));
""",
    """    let temp_parent = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    let root = temp_parent.join(format!(
        "allow-rust-scan-cache-{label}-{}-{stamp}-{id}",
        std::process::id()
    ));
""",
)

store_path = Path("crates/allow-rust/src/scan_cache_store.rs")
text = store_path.read_text(encoding="utf-8")
marker = "#[cfg(test)]\nmod tests {"
if text.count(marker) != 1:
    raise SystemExit("scan_cache_store.rs: test-module marker drift")
production, tests = text.split(marker, 1)
replaced = tests.count("std::env::temp_dir()")
if replaced < 1:
    raise SystemExit("scan_cache_store.rs: expected test temp roots")
tests = tests.replace("std::env::temp_dir()", "canonical_temp_dir()")
use_marker = "    use std::time::SystemTime;\n"
if tests.count(use_marker) != 1:
    raise SystemExit("scan_cache_store.rs: test import drift")
helper = """    use std::time::SystemTime;

    fn canonical_temp_dir() -> PathBuf {
        std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir())
    }
"""
tests = tests.replace(use_marker, helper, 1)
store_path.write_text(production + marker + tests, encoding="utf-8")
print(f"canonicalized {replaced} strict-store test temp roots")
