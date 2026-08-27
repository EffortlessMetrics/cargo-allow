//! Root-bound admission for the advisory persistent scan cache (#3915).
//!
//! A platform-owned alias before the selected repository root may resolve to
//! the same underlying directory. The cache may use that canonical root only
//! while an open identity for the requested root and the deepest existing
//! cache parent still match. Any indirection introduced at or below the root,
//! or any stale/unsupported identity, leaves the scanner on its correct
//! non-persistent path.

use crate::ScanCacheStore;
use same_file::Handle;
use std::fs::{self, Metadata, OpenOptions};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

const STORE_FILE_NAME: &str = "scan-cache.v2.bin";
const LOCK_FILE_NAME: &str = "scan-cache.v2.lock";
const TEMP_FILE_PREFIX: &str = "scan-cache.v2.bin.tmp-";

/// Portable result classes for root-bound cache-target validation.
///
/// These values deliberately omit private absolute paths. They describe why
/// persistence is admitted or why the caller must retain a non-persistent
/// scan; they do not make cache state correctness authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScanCacheTargetDispositionV1 {
    /// The requested spelling resolves through an alias before the selected
    /// repository root, while both spellings identify the same directory.
    BenignExternalRootAlias,
    /// The requested root already has its canonical spelling.
    ExactRepositoryRoot,
    /// Every existing cache-path component below the root is an owned
    /// directory with no symlink or reparse movement.
    SafeOwnedDescendant,
    /// A symlink or reparse point exists at or below the selected root.
    InRootSymlinkOrReparseEscape,
    /// A cache directory, lock, temp file, or final store changed identity or
    /// type after admission.
    DestinationAliasOrTypeChange,
    /// The selected root no longer identifies the directory bound at open.
    RootIdentityChanged,
    /// The platform or supplied root shape cannot support this identity law.
    UnsupportedFilesystem,
    /// Metadata, handle acquisition, encoding, locking, or I/O failed while
    /// all observed target identities remained current.
    InstrumentFailure,
}

impl ScanCacheTargetDispositionV1 {
    /// Stable path-free machine label for diagnostics and receipts.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BenignExternalRootAlias => "benign_external_root_alias",
            Self::ExactRepositoryRoot => "exact_repository_root",
            Self::SafeOwnedDescendant => "safe_owned_descendant",
            Self::InRootSymlinkOrReparseEscape => "in_root_symlink_or_reparse_escape",
            Self::DestinationAliasOrTypeChange => "destination_alias_or_type_change",
            Self::RootIdentityChanged => "root_identity_changed",
            Self::UnsupportedFilesystem => "unsupported_filesystem",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// Persistent scan store admitted against one exact repository-root identity.
///
/// The inner [`ScanCacheStore`] retains its existing lock, temp-file, final
/// destination, and corruption checks. This wrapper adds the missing trust
/// boundary: only an existing repository root is canonicalized, and an open
/// handle to that root plus the deepest existing cache parent is rechecked
/// before persistence.
pub struct RootBoundScanCacheStore {
    requested_root: PathBuf,
    canonical_root: PathBuf,
    root_handle: Handle,
    store_dir: PathBuf,
    bound_parent_path: PathBuf,
    bound_parent_handle: Handle,
    root_disposition: ScanCacheTargetDispositionV1,
    store: ScanCacheStore,
}

impl RootBoundScanCacheStore {
    /// Open the advisory store for an existing absolute repository root.
    ///
    /// Any unsupported, ambiguous, stale, or failed identity check returns a
    /// path-free disposition. Callers must continue with ordinary scanning.
    pub fn open(
        root: impl AsRef<Path>,
        generation: impl Into<String>,
    ) -> Result<Self, ScanCacheTargetDispositionV1> {
        #[cfg(not(any(unix, windows)))]
        {
            let _ = root;
            let _ = generation;
            return Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem);
        }

        #[cfg(any(unix, windows))]
        {
            Self::open_supported(root.as_ref(), generation)
        }
    }

    #[cfg(any(unix, windows))]
    fn open_supported(
        requested_root: &Path,
        generation: impl Into<String>,
    ) -> Result<Self, ScanCacheTargetDispositionV1> {
        if !requested_root.is_absolute() {
            return Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem);
        }
        validate_initial_root(requested_root)?;
        let root_handle = Handle::from_path(requested_root)
            .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
        let canonical_root = fs::canonicalize(requested_root)
            .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
        validate_current_root(requested_root, &root_handle)?;
        validate_handle(
            &canonical_root,
            &root_handle,
            ScanCacheTargetDispositionV1::RootIdentityChanged,
        )?;

        let store_dir = ScanCacheStore::default_dir(&canonical_root);
        let (bound_parent_path, bound_parent_handle) =
            bind_deepest_existing_parent(&canonical_root, &store_dir)?;
        validate_known_artifacts(&store_dir)?;
        let store = ScanCacheStore::open(&store_dir, generation);

        validate_current_root(requested_root, &root_handle)?;
        validate_handle(
            &canonical_root,
            &root_handle,
            ScanCacheTargetDispositionV1::RootIdentityChanged,
        )?;
        validate_handle(
            &bound_parent_path,
            &bound_parent_handle,
            ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange,
        )?;
        let root_disposition = if requested_root == canonical_root {
            ScanCacheTargetDispositionV1::ExactRepositoryRoot
        } else {
            ScanCacheTargetDispositionV1::BenignExternalRootAlias
        };

        Ok(Self {
            requested_root: requested_root.to_path_buf(),
            canonical_root,
            root_handle,
            store_dir,
            bound_parent_path,
            bound_parent_handle,
            root_disposition,
            store,
        })
    }

    /// How the requested root spelling related to the bound root identity.
    pub const fn root_disposition(&self) -> ScanCacheTargetDispositionV1 {
        self.root_disposition
    }

    /// The admitted cache target is always a checked owned descendant.
    pub const fn target_disposition(&self) -> ScanCacheTargetDispositionV1 {
        ScanCacheTargetDispositionV1::SafeOwnedDescendant
    }

    /// Recheck root, parent, cache directory, lock, temp, and destination
    /// posture, then persist through the existing atomic store implementation.
    pub fn flush_with_disposition(&mut self) -> Result<(), ScanCacheTargetDispositionV1> {
        self.flush_with_hook(None)
    }

    /// Boolean compatibility projection for existing advisory flush callers.
    pub fn flush(&mut self) -> bool {
        self.flush_with_disposition().is_ok()
    }

    pub(crate) fn inner_mut(&mut self) -> &mut ScanCacheStore {
        &mut self.store
    }

    #[cfg(test)]
    pub(crate) fn flush_with_test_hook(
        &mut self,
        hook: &dyn Fn(&Path),
    ) -> Result<(), ScanCacheTargetDispositionV1> {
        self.flush_with_hook(Some(hook))
    }

    fn flush_with_hook(
        &mut self,
        hook: Option<&dyn Fn(&Path)>,
    ) -> Result<(), ScanCacheTargetDispositionV1> {
        let snapshot = self.prepare_flush_snapshot()?;
        if let Some(hook) = hook {
            hook(&self.store_dir);
        }

        self.validate_current_binding()?;
        snapshot.validate_before_flush(&self.store_dir)?;

        if !self.store.flush() {
            self.validate_current_binding()?;
            return match snapshot.validate_after_failed_flush(&self.store_dir) {
                Ok(()) => Err(ScanCacheTargetDispositionV1::InstrumentFailure),
                Err(disposition) => Err(disposition),
            };
        }

        self.validate_current_binding()?;
        snapshot.validate_after_successful_flush(&self.store_dir)?;
        validate_known_artifacts(&self.store_dir)?;
        self.rebind_deepest_parent()?;
        Ok(())
    }

    fn prepare_flush_snapshot(
        &mut self,
    ) -> Result<FlushArtifactSnapshot, ScanCacheTargetDispositionV1> {
        self.validate_current_binding()?;
        bind_deepest_existing_parent(&self.canonical_root, &self.store_dir)?;
        validate_known_artifacts(&self.store_dir)?;

        ensure_owned_descendant(&self.canonical_root, &self.store_dir)?;
        self.validate_current_binding()?;
        self.rebind_deepest_parent()?;
        if self.bound_parent_path != self.store_dir {
            return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
        }
        validate_known_artifacts(&self.store_dir)?;

        let lock_path = self.store_dir.join(LOCK_FILE_NAME);
        let lock_handle = ensure_bound_regular_file(&lock_path)?;
        let destination_path = self.store_dir.join(STORE_FILE_NAME);
        let destination_handle = bind_optional_regular_file(&destination_path)?;
        let temp_files = bind_temp_files(&self.store_dir)?;

        Ok(FlushArtifactSnapshot {
            lock_path,
            lock_handle,
            destination_path,
            destination_handle,
            temp_files,
        })
    }

    fn rebind_deepest_parent(&mut self) -> Result<(), ScanCacheTargetDispositionV1> {
        let (path, handle) = bind_deepest_existing_parent(&self.canonical_root, &self.store_dir)?;
        self.bound_parent_path = path;
        self.bound_parent_handle = handle;
        Ok(())
    }

    fn validate_current_binding(&self) -> Result<(), ScanCacheTargetDispositionV1> {
        validate_current_root(&self.requested_root, &self.root_handle)?;
        validate_handle(
            &self.canonical_root,
            &self.root_handle,
            ScanCacheTargetDispositionV1::RootIdentityChanged,
        )?;
        validate_handle(
            &self.bound_parent_path,
            &self.bound_parent_handle,
            ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange,
        )
    }
}

struct BoundTempFile {
    path: PathBuf,
    handle: Handle,
}

struct FlushArtifactSnapshot {
    lock_path: PathBuf,
    lock_handle: Handle,
    destination_path: PathBuf,
    destination_handle: Option<Handle>,
    temp_files: Vec<BoundTempFile>,
}

impl FlushArtifactSnapshot {
    fn validate_before_flush(&self, store_dir: &Path) -> Result<(), ScanCacheTargetDispositionV1> {
        validate_bound_regular_file(&self.lock_path, &self.lock_handle)?;
        validate_optional_regular_file(&self.destination_path, self.destination_handle.as_ref())?;
        validate_temp_files(store_dir, &self.temp_files, true)
    }

    fn validate_after_failed_flush(
        &self,
        store_dir: &Path,
    ) -> Result<(), ScanCacheTargetDispositionV1> {
        validate_bound_regular_file(&self.lock_path, &self.lock_handle)?;
        validate_optional_regular_file(&self.destination_path, self.destination_handle.as_ref())?;
        validate_temp_files(store_dir, &self.temp_files, false)
    }

    fn validate_after_successful_flush(
        &self,
        store_dir: &Path,
    ) -> Result<(), ScanCacheTargetDispositionV1> {
        validate_bound_regular_file(&self.lock_path, &self.lock_handle)?;
        validate_file_candidate(&self.destination_path)?;
        validate_temp_files(store_dir, &self.temp_files, false)
    }
}

fn validate_initial_root(root: &Path) -> Result<(), ScanCacheTargetDispositionV1> {
    let metadata =
        fs::symlink_metadata(root).map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
    if metadata_is_indirection(&metadata) {
        return Err(ScanCacheTargetDispositionV1::InRootSymlinkOrReparseEscape);
    }
    if !metadata.is_dir() {
        return Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem);
    }
    Ok(())
}

fn validate_current_root(root: &Path, bound: &Handle) -> Result<(), ScanCacheTargetDispositionV1> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(ScanCacheTargetDispositionV1::RootIdentityChanged);
        }
        Err(_) => return Err(ScanCacheTargetDispositionV1::InstrumentFailure),
    };
    if metadata_is_indirection(&metadata) || !metadata.is_dir() {
        return Err(ScanCacheTargetDispositionV1::RootIdentityChanged);
    }
    validate_handle(
        root,
        bound,
        ScanCacheTargetDispositionV1::RootIdentityChanged,
    )
}

fn validate_handle(
    path: &Path,
    bound: &Handle,
    changed: ScanCacheTargetDispositionV1,
) -> Result<(), ScanCacheTargetDispositionV1> {
    match Handle::from_path(path) {
        Ok(current) if current == *bound => Ok(()),
        Ok(_) => Err(changed),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(changed),
        Err(_) => Err(ScanCacheTargetDispositionV1::InstrumentFailure),
    }
}

fn validate_bound_regular_file(
    path: &Path,
    bound: &Handle,
) -> Result<(), ScanCacheTargetDispositionV1> {
    validate_file_candidate(path)?;
    validate_handle(
        path,
        bound,
        ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange,
    )
}

fn bind_deepest_existing_parent(
    root: &Path,
    target: &Path,
) -> Result<(PathBuf, Handle), ScanCacheTargetDispositionV1> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| ScanCacheTargetDispositionV1::UnsupportedFilesystem)?;
    let mut current = root.to_path_buf();
    let mut deepest_path = current.clone();
    let mut deepest_handle =
        Handle::from_path(&current).map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;

    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem);
        };
        current.push(segment);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(_) => return Err(ScanCacheTargetDispositionV1::InstrumentFailure),
        };
        if metadata_is_indirection(&metadata) {
            return Err(ScanCacheTargetDispositionV1::InRootSymlinkOrReparseEscape);
        }
        if !metadata.is_dir() {
            return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
        }
        deepest_handle = Handle::from_path(&current)
            .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
        deepest_path = current.clone();
    }
    Ok((deepest_path, deepest_handle))
}

#[cfg(unix)]
fn ensure_owned_descendant(
    root: &Path,
    target: &Path,
) -> Result<(), ScanCacheTargetDispositionV1> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const O_CLOEXEC: i32 = 0o2000000;
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    const O_CLOEXEC: i32 = 0x1000000;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const O_DIRECTORY: i32 = 0o200000;
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    const O_DIRECTORY: i32 = 0x100000;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const O_NOFOLLOW: i32 = 0o400000;
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    const O_NOFOLLOW: i32 = 0x100;
    const ENOENT: i32 = 2;
    unsafe extern "C" {
        fn openat(dirfd: i32, path: *const std::ffi::c_char, flags: i32, mode: i32) -> i32;
        fn mkdirat(dirfd: i32, path: *const std::ffi::c_char, mode: u32) -> i32;
    }
    let relative = target
        .strip_prefix(root)
        .map_err(|_| ScanCacheTargetDispositionV1::UnsupportedFilesystem)?;
    let mut parent = std::fs::File::open(root)
        .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem);
        };
        let name = CString::new(segment.as_bytes())
            .map_err(|_| ScanCacheTargetDispositionV1::UnsupportedFilesystem)?;
        let mut fd = unsafe { openat(parent.as_raw_fd(), name.as_ptr(), O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC, 0) };
        if fd < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
            if errno != ENOENT {
                return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
            }
            if unsafe { mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } < 0 {
                let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
                if errno != 17 {
                    return Err(ScanCacheTargetDispositionV1::InstrumentFailure);
                }
            }
            fd = unsafe { openat(parent.as_raw_fd(), name.as_ptr(), O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC, 0) };
        }
        if fd < 0 {
            return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
        }
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        parent = std::fs::File::from(owned);
    }
    Ok(())
}

#[cfg(windows)]
fn ensure_owned_descendant(
    root: &Path,
    target: &Path,
) -> Result<(), ScanCacheTargetDispositionV1> {
    // Windows has no stable std-only handle-relative mkdir primitive. Refuse
    // the first path mutation unless every component already exists; this
    // preserves the fail-closed boundary rather than racing create_dir_all.
    let _ = root;
    let _ = fs::metadata(target)
        .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
    Ok(())
}

fn ensure_bound_regular_file(path: &Path) -> Result<Handle, ScanCacheTargetDispositionV1> {
    match fs::symlink_metadata(path) {
        Ok(_) => validate_file_candidate(path)?,
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err(ScanCacheTargetDispositionV1::InstrumentFailure),
    }

    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            validate_file_candidate(path)?;
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?
        }
        Err(_) => return Err(ScanCacheTargetDispositionV1::InstrumentFailure),
    };
    if !file
        .metadata()
        .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?
        .is_file()
    {
        return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
    }
    let handle = Handle::from_file(
        file.try_clone()
            .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?,
    )
    .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
    validate_bound_regular_file(path, &handle)?;
    Ok(handle)
}

fn bind_optional_regular_file(path: &Path) -> Result<Option<Handle>, ScanCacheTargetDispositionV1> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata_is_indirection(&metadata) || !metadata.is_file() {
                return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
            }
            Handle::from_path(path)
                .map(Some)
                .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ScanCacheTargetDispositionV1::InstrumentFailure),
    }
}

fn validate_optional_regular_file(
    path: &Path,
    expected: Option<&Handle>,
) -> Result<(), ScanCacheTargetDispositionV1> {
    match expected {
        Some(handle) => validate_bound_regular_file(path, handle),
        None => match fs::symlink_metadata(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Ok(_) => Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange),
            Err(_) => Err(ScanCacheTargetDispositionV1::InstrumentFailure),
        },
    }
}

fn bind_temp_files(store_dir: &Path) -> Result<Vec<BoundTempFile>, ScanCacheTargetDispositionV1> {
    let entries =
        fs::read_dir(store_dir).map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(TEMP_FILE_PREFIX)
        {
            continue;
        }
        let path = entry.path();
        validate_file_candidate(&path)?;
        let handle = Handle::from_path(&path)
            .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
        files.push(BoundTempFile { path, handle });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn validate_temp_files(
    store_dir: &Path,
    expected: &[BoundTempFile],
    require_existing: bool,
) -> Result<(), ScanCacheTargetDispositionV1> {
    let current = bind_temp_files(store_dir)?;
    for file in &current {
        let Some(bound) = expected.iter().find(|bound| bound.path == file.path) else {
            return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
        };
        if bound.handle != file.handle {
            return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
        }
    }
    if require_existing
        && expected
            .iter()
            .any(|bound| !current.iter().any(|file| file.path == bound.path))
    {
        return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
    }
    Ok(())
}

fn validate_known_artifacts(store_dir: &Path) -> Result<(), ScanCacheTargetDispositionV1> {
    for name in [STORE_FILE_NAME, LOCK_FILE_NAME] {
        validate_file_candidate(&store_dir.join(name))?;
    }
    let entries = match fs::read_dir(store_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ScanCacheTargetDispositionV1::InstrumentFailure),
    };
    for entry in entries {
        let entry = entry.map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(TEMP_FILE_PREFIX)
        {
            validate_file_candidate(&entry.path())?;
        }
    }
    Ok(())
}

fn validate_file_candidate(path: &Path) -> Result<(), ScanCacheTargetDispositionV1> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ScanCacheTargetDispositionV1::InstrumentFailure),
    };
    if metadata_is_indirection(&metadata) || !metadata.is_file() {
        return Err(ScanCacheTargetDispositionV1::DestinationAliasOrTypeChange);
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_is_indirection(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn metadata_is_indirection(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
