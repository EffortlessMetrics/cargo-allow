use super::Member;
use serde::de::{DeserializeOwned, DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub(super) const JSON_LIMIT: u64 = 8 * 1024 * 1024;
const BINARY_LIMIT: u64 = 256 * 1024 * 1024;

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
    read_regular(&absolute, JSON_LIMIT)
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
        let bytes = read_regular(&path, JSON_LIMIT)?;
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
        reject_symlink_components(&path)?;
        let mut file = File::open(&path).map_err(|error| format!("open binary: {error}"))?;
        let before = file.metadata().map_err(|error| error.to_string())?;
        if !before.is_file() || before.len() != member.size_bytes {
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
        let after = file.metadata().map_err(|error| error.to_string())?;
        let digest = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        reject_symlink_components(&path)?;
        if total != member.size_bytes
            || !stable_identity(&before, &after)
            || !stable_identity(
                &after,
                &fs::metadata(&path).map_err(|error| error.to_string())?,
            )
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
        // Retained members are independently created copies. A hard link can
        // alias another role or external owner and is refused where the host
        // exposes link counts. This is readback, not a filesystem sandbox.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .nlink()
                != 1
            {
                return Err("retained member has multiple hard-link owners".to_string());
            }
        }
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
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err("input path contains an ambiguous component".to_string());
        }
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("inspect {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "symlink input/member refused: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    reject_symlink_components(path)?;
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let before = file.metadata().map_err(|error| error.to_string())?;
    if !before.is_file() || before.len() > limit {
        return Err(format!("not a bounded regular file: {}", path.display()));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let after = file.metadata().map_err(|error| error.to_string())?;
    reject_symlink_components(path)?;
    if bytes.len() as u64 != before.len()
        || !stable_identity(&before, &after)
        || !stable_identity(
            &after,
            &fs::metadata(path).map_err(|error| error.to_string())?,
        )
    {
        return Err(format!("member changed during read: {}", path.display()));
    }
    Ok(bytes)
}

fn stable_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino() {
            return false;
        }
    }
    left.is_file() == right.is_file()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

struct UniqueKeys;

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
