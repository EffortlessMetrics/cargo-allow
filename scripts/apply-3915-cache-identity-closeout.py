from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    raw = path.read_bytes()
    text = raw.decode("utf-8")
    newline = "\r\n" if "\r\n" in text else "\n"
    normalized = text.replace("\r\n", "\n")
    count = normalized.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one source shape, found {count}")
    normalized = normalized.replace(old, new, 1)
    if newline == "\r\n":
        normalized = normalized.replace("\n", "\r\n")
    path.write_bytes(normalized.encode("utf-8"))


manifest = Path("crates/allow-rust/Cargo.toml")
replace_once(
    manifest,
    'syntax = ["dep:same-file", "dep:tree-sitter", "dep:tree-sitter-rust"]',
    'syntax = ["dep:tree-sitter", "dep:tree-sitter-rust"]',
)
replace_once(
    manifest,
    'same-file = { workspace = true, optional = true }',
    'same-file.workspace = true',
)

store = Path("crates/allow-rust/src/scan_cache_store.rs")
replace_once(
    store,
    '''#[cfg(feature = "syntax")]
struct PathIdentity(same_file::Handle);

#[cfg(not(feature = "syntax"))]
struct PathIdentity;

impl PathIdentity {
    fn from_path(path: &Path) -> Option<Self> {
        #[cfg(feature = "syntax")]
        {
            same_file::Handle::from_path(path).ok().map(Self)
        }
        #[cfg(not(feature = "syntax"))]
        {
            let _ = path;
            Some(Self)
        }
    }

    fn from_file(file: &File) -> Option<Self> {
        #[cfg(feature = "syntax")]
        {
            let cloned = file.try_clone().ok()?;
            same_file::Handle::from_file(cloned).ok().map(Self)
        }
        #[cfg(not(feature = "syntax"))]
        {
            let _ = file;
            Some(Self)
        }
    }

    fn matches_path(&self, path: &Path) -> bool {
        #[cfg(feature = "syntax")]
        {
            same_file::Handle::from_path(path).is_ok_and(|current| current == self.0)
        }
        #[cfg(not(feature = "syntax"))]
        {
            let _ = (self, path);
            true
        }
    }
}
''',
    '''struct PathIdentity(same_file::Handle);

impl PathIdentity {
    fn from_path(path: &Path) -> Option<Self> {
        same_file::Handle::from_path(path).ok().map(Self)
    }

    fn from_file(file: &File) -> Option<Self> {
        let cloned = file.try_clone().ok()?;
        same_file::Handle::from_file(cloned).ok().map(Self)
    }

    fn matches_path(&self, path: &Path) -> bool {
        same_file::Handle::from_path(path).is_ok_and(|current| current == self.0)
    }
}
''',
)
replace_once(
    store,
    '''        if !regular_file_identity_matches(&tmp, &temp_identity) {
            return false;
        }
''',
    '''        if !regular_file_identity_matches(&tmp, &temp_identity) {
            remove_bound_file(&tmp, &temp_identity);
            return false;
        }
''',
)
replace_once(store, '    #[cfg(feature = "syntax")]\n    #[test]\n    fn temp_regular_file_replacement_after_sync_is_rejected()', '    #[test]\n    fn temp_regular_file_replacement_after_sync_is_rejected()')
replace_once(store, '    #[cfg(feature = "syntax")]\n    #[test]\n    fn destination_regular_file_replacement_after_temp_sync_is_rejected()', '    #[test]\n    fn destination_regular_file_replacement_after_temp_sync_is_rejected()')
replace_once(store, '    #[cfg(all(feature = "syntax", unix))]\n    #[test]\n    fn lock_regular_file_replacement_while_waiting_is_rejected()', '    #[cfg(unix)]\n    #[test]\n    fn lock_regular_file_replacement_while_waiting_is_rejected()')

alias_tests = Path("crates/allow-rust/src/tests/cache_root_alias.rs")
insert_before = '''#[test]
fn unsupported_or_missing_roots_never_admit_persistence() -> Result<(), String> {
'''
new_test = '''#[cfg(unix)]
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

'''
replace_once(alias_tests, insert_before, new_test + insert_before)

world = Path("crates/cargo-allow/src/world.rs")
world_marker = '''    #[test]
    fn persistent_cache_off_policy_preserves_result_without_creating_store() -> Result<(), String> {
'''
world_test = '''    #[cfg(unix)]
    #[test]
    fn persistent_cache_admission_failure_falls_back_without_outside_write() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let root = fixture_dir();
        fs::create_dir_all(root.join("src")).map_err(|err| format!("src dir: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "fn fallback(value: Result<(), ()>) { value.unwrap(); }\n",
        )
        .map_err(|err| format!("source: {err}"))?;
        let outside = root.join("outside-target");
        fs::create_dir_all(&outside).map_err(|err| format!("outside dir: {err}"))?;
        symlink(&outside, root.join("target")).map_err(|err| format!("target alias: {err}"))?;
        let files = vec![PathBuf::from("src/lib.rs")];

        let persistent = scan_rust_files_with_cache_mode(&root, &files, true)
            .map_err(|err| format!("persistent fallback scan: {err}"))?;
        let ordinary = scan_rust_files_with_cache_mode(&root, &files, false)
            .map_err(|err| format!("ordinary scan: {err}"))?;
        if persistent.findings != ordinary.findings
            || persistent.file_statuses != ordinary.file_statuses
            || persistent.files_considered != ordinary.files_considered
            || persistent.files_skipped != ordinary.files_skipped
            || persistent.files_with_parse_errors != ordinary.files_with_parse_errors
        {
            return Err("cache admission failure changed scan semantics".to_string());
        }
        if outside
            .join("cargo-allow/cache/scan-cache.v2.bin")
            .exists()
        {
            return Err("cache admission failure wrote through in-root alias".to_string());
        }
        fs::remove_file(root.join("target")).map_err(|err| format!("remove alias: {err}"))?;
        fs::remove_dir_all(root).map_err(|err| format!("remove fixture: {err}"))?;
        Ok(())
    }

'''
replace_once(world, world_marker, world_test + world_marker)
