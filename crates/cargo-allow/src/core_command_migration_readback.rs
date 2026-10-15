use super::Member;
use serde::de::{DeserializeOwned, DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub(super) const JSON_LIMIT: u64 = 8 * 1024 * 1024;
const BINARY_LIMIT: u64 = 256 * 1024 * 1024;

struct FileSnapshot {
    metadata: fs::Metadata,
    #[cfg(windows)]
    identity: (u64, u64, u64),
}

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    UniqueKeys
        .deserialize(&mut decoder)
        .map_err(|error| error.to_string())?;
    decoder.end().map_err(|error| error.to_string())?;
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub(crate) fn read_input(path: &Path) -> Result<Vec<u8>, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    reject_symlink_components(&absolute)?;
    read_regular(&absolute, JSON_LIMIT, false)
}

pub(crate) fn validate_new_output(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let parent = absolute.parent().ok_or("output lacks a parent directory")?;
    reject_symlink_components(parent)?;
    if absolute.file_name().is_none() {
        return Err("output lacks a new file name".to_string());
    }
    match fs::symlink_metadata(&absolute) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(absolute),
        Ok(_) => Err("admission output already exists; refusing replacement".to_string()),
        Err(error) => Err(format!("inspect new output: {error}")),
    }
}

pub(super) struct MemberReader {
    root: PathBuf,
    paths: BTreeSet<String>,
}

impl MemberReader {
    pub(super) fn new(root: &Path) -> Result<Self, String> {
        let root = if root.is_absolute() {
            root.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| error.to_string())?
                .join(root)
        };
        reject_symlink_components(&root)?;
        if !fs::metadata(&root)
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            return Err("bundle root is not a directory".to_string());
        }
        Ok(Self {
            root,
            paths: BTreeSet::new(),
        })
    }

    pub(super) fn read(&mut self, member: &Member) -> Result<Vec<u8>, String> {
        let path = self.claim(member)?;
        if member.size_bytes > JSON_LIMIT {
            return Err(format!("member exceeds size bound: {}", member.path));
        }
        let bytes = read_regular(&path, JSON_LIMIT, true)?;
        if bytes.len() as u64 != member.size_bytes
            || allow_core::sha256_v1_bytes(&bytes) != member.digest
        {
            return Err(format!("member size/digest mismatch: {}", member.path));
        }
        Ok(bytes)
    }

    pub(super) fn verify_binary(&mut self, member: &Member) -> Result<(), String> {
        let path = self.claim(member)?;
        if member.size_bytes == 0 || member.size_bytes > BINARY_LIMIT {
            return Err("binary member has invalid size".to_string());
        }
        let (mut file, before) = open_regular(&path, BINARY_LIMIT, true)?;
        if before.metadata.len() != member.size_bytes {
            return Err("binary member is not the declared regular file".to_string());
        }
        let mut hasher = Sha256::new();
        let mut total = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > member.size_bytes {
                return Err("binary grew during read".to_string());
            }
            let chunk = buffer.get(..count).ok_or("invalid binary read length")?;
            hasher.update(chunk);
        }
        let digest = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if total != member.size_bytes
            || !file_still_matches(&file, &before, &path, BINARY_LIMIT, true)?
            || format!("sha256:v1:{digest}") != member.digest
        {
            return Err("binary member changed or has the wrong digest".to_string());
        }
        Ok(())
    }

    fn claim(&mut self, member: &Member) -> Result<PathBuf, String> {
        safe_relative(&member.path)?;
        if !valid_digest(&member.digest) || !self.paths.insert(member.path.clone()) {
            return Err(format!(
                "malformed digest or duplicate member role: {}",
                member.path
            ));
        }
        let path = self.root.join(&member.path);
        reject_symlink_components(&path)?;
        Ok(path)
    }
}

pub(super) fn safe_relative(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || Path::new(value)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("unsafe member path: {value:?}"));
    }
    Ok(())
}

pub(super) fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:v1:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn reject_symlink_components(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("an absolute input/member path is required".to_string());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err("input path contains an ambiguous component".to_string());
    }
    // Ancestors begin at a complete root, including Windows verbatim/UNC
    // roots. A standalone drive Prefix is not a filesystem path to inspect.
    for current in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let metadata = fs::symlink_metadata(current)
            .map_err(|error| format!("inspect {}: {error}", current.display()))?;
        if is_indirection(&metadata) {
            return Err(format!(
                "symlink input/member refused: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

fn open_regular(
    path: &Path,
    limit: u64,
    retained_member: bool,
) -> Result<(File, FileSnapshot), String> {
    reject_symlink_components(path)?;
    // Inspect before open: an unwritten FIFO blocks in an ordinary File::open,
    // before a post-open size or kind check can protect the caller.
    let named = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    validate_regular(&named, path, limit, retained_member)?;
    let mut options = OpenOptions::new();
    options.read(true);
    configure_regular_open(&mut options)?;
    let file = options
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    let opened = snapshot_file(&file, path, limit, retained_member)?;
    if !stable_metadata(&named, &opened.metadata) {
        return Err(format!("file changed before open: {}", path.display()));
    }
    Ok((file, opened))
}

fn snapshot_file(
    file: &File,
    path: &Path,
    limit: u64,
    retained_member: bool,
) -> Result<FileSnapshot, String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    validate_regular(&metadata, path, limit, retained_member)?;
    #[cfg(windows)]
    let identity = {
        // Query the already opened read-only handle. Keep it open until the
        // final named handle has been compared, so its file index cannot be
        // recycled between observations. This is the workspace same-file
        // dependency's volume/index API, with link ownership also enforced.
        let info = winapi_util::file::information(file).map_err(|error| error.to_string())?;
        if retained_member && info.number_of_links() != 1 {
            return Err("retained member has multiple hard-link owners".to_string());
        }
        (
            info.volume_serial_number(),
            info.file_index(),
            info.number_of_links(),
        )
    };
    Ok(FileSnapshot {
        metadata,
        #[cfg(windows)]
        identity,
    })
}

fn file_still_matches(
    file: &File,
    before: &FileSnapshot,
    path: &Path,
    limit: u64,
    retained_member: bool,
) -> Result<bool, String> {
    let after = snapshot_file(file, path, limit, retained_member)?;
    reject_symlink_components(path)?;
    let named = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    validate_regular(&named, path, limit, retained_member)?;
    if !stable_metadata(&before.metadata, &after.metadata)
        || !stable_metadata(&after.metadata, &named)
    {
        return Ok(false);
    }
    #[cfg(windows)]
    {
        // MetadataExt's native identity methods are unstable at Rust 1.95.
        // Reopen without following a replaced reparse leaf and compare real
        // volume/index/link observations, not just equal lengths and mtimes.
        let (named_file, named_snapshot) = open_regular(path, limit, retained_member)?;
        let same = before.identity == after.identity
            && after.identity == named_snapshot.identity
            && stable_metadata(&after.metadata, &named_snapshot.metadata);
        drop(named_file);
        return Ok(same);
    }
    #[cfg(not(windows))]
    Ok(true)
}

fn validate_regular(
    metadata: &fs::Metadata,
    path: &Path,
    limit: u64,
    retained_member: bool,
) -> Result<(), String> {
    if !metadata.is_file() || is_indirection(metadata) || metadata.len() > limit {
        return Err(format!("not a bounded regular file: {}", path.display()));
    }
    // Retained members are independent copies. Check both the named file and
    // its opened handle so an alias introduced around open is also refused.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if retained_member && metadata.nlink() != 1 {
            return Err("retained member has multiple hard-link owners".to_string());
        }
    }
    #[cfg(not(unix))]
    let _ = retained_member;
    Ok(())
}

fn configure_regular_open(options: &mut OpenOptions) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // libc 0.2.189's native ABI definitions: O_NOFOLLOW differs between
        // x86 and ARM even on the same OS. O_NONBLOCK closes a regular-to-FIFO
        // replacement race; O_NOFOLLOW refuses a replaced leaf symlink.
        // Keep unverified OS/architecture combinations explicitly unavailable.
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let flags = match (os, arch) {
            ("linux" | "android", "x86" | "x86_64") => 0x800 | 0x20000,
            ("linux" | "android", "arm" | "aarch64") => 0x800 | 0x8000,
            ("macos" | "ios" | "freebsd", "x86" | "x86_64" | "arm" | "aarch64") => 0x4 | 0x100,
            _ => {
                return Err(format!(
                    "bounded regular-file opening is unsupported for {os}/{arch}"
                ));
            }
        };
        options.custom_flags(flags);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Inspect a replaced reparse point itself instead of its target.
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    #[cfg(not(any(unix, windows)))]
    return Err("bounded file identity is unsupported on this platform".to_string());
    #[cfg(any(unix, windows))]
    Ok(())
}

fn is_indirection(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn read_regular(path: &Path, limit: u64, retained_member: bool) -> Result<Vec<u8>, String> {
    let (mut file, before) = open_regular(path, limit, retained_member)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 != before.metadata.len()
        || !file_still_matches(&file, &before, path, limit, retained_member)?
    {
        return Err(format!("member changed during read: {}", path.display()));
    }
    Ok(bytes)
}

fn stable_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino() || left.nlink() != right.nlink() {
            return false;
        }
    }
    left.is_file() == right.is_file()
        && is_indirection(left) == is_indirection(right)
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

struct UniqueKeys;

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;
    use std::fs::FileTimes;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-readback-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&root).map_err(|error| error.to_string())?;
        root.canonicalize().map_err(|error| error.to_string())
    }

    #[test]
    fn equal_size_and_mtime_replacement_is_not_the_opened_file() -> Result<(), String> {
        let root = fixture("replacement")?;
        let path = root.join("member");
        fs::write(&path, b"original").map_err(|error| error.to_string())?;
        let (mut file, before) = open_regular(&path, JSON_LIMIT, true)?;
        if !file_still_matches(&file, &before, &path, JSON_LIMIT, true)? {
            return Err("unchanged file failed its identity check".to_string());
        }
        fs::rename(&path, root.join("original")).map_err(|error| error.to_string())?;
        fs::write(&path, b"replaced").map_err(|error| error.to_string())?;
        let modified = before
            .metadata
            .modified()
            .map_err(|error| error.to_string())?;
        OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|error| error.to_string())?
            .set_times(FileTimes::new().set_modified(modified))
            .map_err(|error| error.to_string())?;
        let replacement = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if replacement.len() != before.metadata.len()
            || replacement.modified().map_err(|error| error.to_string())? != modified
        {
            return Err("replacement fixture did not preserve size and mtime".to_string());
        }
        if file_still_matches(&file, &before, &path, JSON_LIMIT, true)? {
            return Err("equal-size/equal-mtime replacement was accepted".to_string());
        }
        let mut original = Vec::new();
        file.read_to_end(&mut original)
            .map_err(|error| error.to_string())?;
        if original != b"original"
            || fs::read(&path).map_err(|error| error.to_string())? != b"replaced"
        {
            return Err("identity verification changed fixture bytes".to_string());
        }
        drop(file);
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn retained_member_link_added_after_open_is_refused() -> Result<(), String> {
        let root = fixture("hard-link")?;
        let path = root.join("member");
        let alias = root.join("alias");
        fs::write(&path, b"owned bytes").map_err(|error| error.to_string())?;
        let (file, before) = open_regular(&path, JSON_LIMIT, true)?;
        if !file_still_matches(&file, &before, &path, JSON_LIMIT, true)? {
            return Err("single-owner member was refused".to_string());
        }
        fs::hard_link(&path, &alias).map_err(|error| error.to_string())?;
        if !file_still_matches(&file, &before, &path, JSON_LIMIT, true)
            .is_err_and(|error| error.contains("hard-link"))
            || !open_regular(&alias, JSON_LIMIT, true)
                .is_err_and(|error| error.contains("hard-link"))
        {
            return Err("retained member alias was not refused for ownership".to_string());
        }
        if read_input(&path)? != b"owned bytes"
            || fs::read(&alias).map_err(|error| error.to_string())? != b"owned bytes"
        {
            return Err("ownership verification changed fixture bytes".to_string());
        }
        drop(file);
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }
}

impl<'de> DeserializeSeed<'de> for UniqueKeys {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for UniqueKeys {
    type Value = ();
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON with unique object keys")
    }
    fn visit_bool<E: Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence.next_element_seed(UniqueKeys)?.is_some() {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(A::Error::custom(format!(
                    "duplicate JSON object key {key:?}"
                )));
            }
            object.next_value_seed(UniqueKeys)?;
        }
        Ok(())
    }
}
