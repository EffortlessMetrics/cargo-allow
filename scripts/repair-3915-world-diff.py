from pathlib import Path
import subprocess

BASE = "664bbbb6310ac52df817c91939b5703ef08d05a8"
PATH = Path("crates/cargo-allow/src/world.rs")

raw = subprocess.check_output(["git", "show", f"{BASE}:{PATH.as_posix()}"])
text = raw.decode("utf-8")
newline = "\r\n" if "\r\n" in text else "\n"
text = text.replace("\r\n", "\n")


def replace_once(old: str, new: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one world.rs source shape, found {count}")
    text = text.replace(old, new, 1)


replace_once(
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
            && let Ok(mut store) =
                allow_rust::RootBoundScanCacheStore::open(root, allow_rust::scan_cache_generation())
        {
            let result = allow_rust::scan_rust_files_cached_with_root_bound_store(
                root, files, &mut cache, &mut store,
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

if newline == "\r\n":
    text = text.replace("\n", "\r\n")
PATH.write_bytes(text.encode("utf-8"))

numstat = subprocess.check_output(
    ["git", "diff", "--numstat", "--", PATH.as_posix()], text=True
).strip()
print(f"world.rs numstat: {numstat}")
parts = numstat.split()
if len(parts) < 2 or int(parts[0]) > 80 or int(parts[1]) > 80:
    raise SystemExit("world.rs repair did not restore a bounded diff")
