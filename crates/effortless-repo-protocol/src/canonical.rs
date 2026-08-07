use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const DIGEST_PREFIX: &str = "sha256:v1:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalError {
    Serialize(String),
}

impl CanonicalError {
    pub fn message(&self) -> &str {
        match self {
            Self::Serialize(message) => message,
        }
    }
}

pub fn stable_digest_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{DIGEST_PREFIX}{hex}")
}

pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    let json =
        serde_json::to_value(value).map_err(|err| CanonicalError::Serialize(err.to_string()))?;
    let sorted = sort_json_value(json);
    serde_json::to_vec(&sorted).map_err(|err| CanonicalError::Serialize(err.to_string()))
}

pub fn stable_digest_json<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    let bytes = canonical_json_bytes(value)?;
    Ok(stable_digest_hex(&bytes))
}

fn sort_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = serde_json::Map::new();
            for (key, child) in entries {
                sorted.insert(key, sort_json_value(child));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json_value).collect()),
        other => other,
    }
}
