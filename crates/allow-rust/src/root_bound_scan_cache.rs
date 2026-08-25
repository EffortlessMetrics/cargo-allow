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
use std::fs::{self, Metadata};
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
    /// Metadata or handle acquisition failed, so persistence is not proven.
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
        self.validate_current_binding()?;
        bind_deepest_existing_parent(&self.canonical_root, &self.store_dir)?;
        validate_known_artifacts(&self.store_dir)?;

        if !self.store.flush() {
            self.validate_current_binding()?;
            bind_deepest_existing_parent(&self.canonical_root, &self.store_dir)?;
            validate_known_artifacts(&self.store_dir)?;
            return Err(ScanCacheTargetDispositionV1::InstrumentFailure);
        }

        self.validate_current_binding()?;
        let (current_parent_path, current_parent_handle) =
            bind_deepest_existing_parent(&self.canonical_root, &self.store_dir)?;
        validate_known_artifacts(&self.store_dir)?;
        self.bound_parent_path = current_parent_path;
        self.bound_parent_handle = current_parent_handle;
        Ok(())
    }

    /// Boolean compatibility projection for existing advisory flush callers.
    pub fn flush(&mut self) -> bool {
        self.flush_with_disposition().is_ok()
    }

    pub(crate) fn inner_mut(&mut self) -> &mut ScanCacheStore {
        &mut self.store
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

fn validate_initial_root(root: &Path) -> Result<(), ScanCacheTargetDispositionV1> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;
    if metadata_is_indirection(&metadata) {
        return Err(ScanCacheTargetDispositionV1::InRootSymlinkOrReparseEscape);
    }
    if !metadata.is_dir() {
        return Err(ScanCacheTargetDispositionV1::UnsupportedFilesystem);
    }
    Ok(())
}

fn validate_current_root(
    root: &Path,
    bound: &Handle,
) -> Result<(), ScanCacheTargetDispositionV1> {
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

fn bind_deepest_existing_parent(
    root: &Path,
    target: &Path,
) -> Result<(PathBuf, Handle), ScanCacheTargetDispositionV1> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| ScanCacheTargetDispositionV1::UnsupportedFilesystem)?;
    let mut current = root.to_path_buf();
    let mut deepest_path = current.clone();
    let mut deepest_handle = Handle::from_path(&current)
        .map_err(|_| ScanCacheTargetDispositionV1::InstrumentFailure)?;

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
