use std::path::{Path, PathBuf};

use allow_core::read_text_file_capped;
use serde::Deserialize;

use crate::policy_header::{SUPPORTED_SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSION_ALIAS};
use crate::toml_de::option_schema_version;

/// Relative paths searched in order when discovering a cargo-allow policy ledger.
pub const DISCOVERY_REL_PATHS: [&str; 4] = [
    "policy/cargo-allow.toml",
    "policy/allow.toml",
    ".cargo/allow.toml",
    "allow.toml",
];

/// Preferred side-by-side native ledger filename when a foreign dialect owns
/// `policy/allow.toml`.
pub const NATIVE_LEDGER_REL_PATH: &str = "policy/cargo-allow.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPolicyCandidate {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverConfigResult {
    pub selected: Option<PathBuf>,
    pub skipped: Vec<SkippedPolicyCandidate>,
}

#[derive(Debug, Default, Deserialize)]
struct PolicyHeaderProbe {
    #[serde(default, deserialize_with = "option_schema_version")]
    schema_version: Option<String>,
    policy: Option<String>,
}

pub fn discover_config(start: impl AsRef<Path>) -> DiscoverConfigResult {
    let mut dir = match start.as_ref().canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return DiscoverConfigResult {
                selected: None,
                skipped: Vec::new(),
            };
        }
    };
    let mut skipped = Vec::new();
    loop {
        for rel in DISCOVERY_REL_PATHS {
            let candidate = dir.join(rel);
            if !candidate.exists() {
                continue;
            }
            match classify_candidate(&candidate, rel) {
                CandidateClass::Accept => {
                    return DiscoverConfigResult {
                        selected: Some(candidate),
                        skipped,
                    };
                }
                CandidateClass::Skip(reason) => skipped.push(SkippedPolicyCandidate {
                    path: candidate,
                    reason,
                }),
            }
        }
        if !dir.pop() {
            break;
        }
    }
    DiscoverConfigResult {
        selected: None,
        skipped,
    }
}

enum CandidateClass {
    Accept,
    Skip(String),
}

fn classify_candidate(path: &Path, rel: &str) -> CandidateClass {
    let text = match read_text_file_capped(path) {
        Ok(text) => text,
        Err(err) => {
            return CandidateClass::Skip(format!(
                "not cargo-allow dialect (failed to read policy config: {err})"
            ));
        }
    };
    let header = match toml::from_str::<PolicyHeaderProbe>(&text) {
        Ok(header) => header,
        Err(err) => {
            return CandidateClass::Skip(format!(
                "not cargo-allow dialect (failed to parse policy header: {err})"
            ));
        }
    };
    if !supported_schema_version(header.schema_version.as_deref()) {
        let version = header
            .schema_version
            .unwrap_or_else(|| "<missing>".to_string());
        return CandidateClass::Skip(format!(
            "not cargo-allow dialect (unsupported schema_version `{version}`)"
        ));
    }
    if rel == NATIVE_LEDGER_REL_PATH {
        return match header.policy.as_deref() {
            None | Some("cargo-allow") => CandidateClass::Accept,
            Some(policy) => CandidateClass::Skip(format!(
                "not cargo-allow dialect (unsupported policy `{policy}`)"
            )),
        };
    }
    match header.policy.as_deref() {
        Some("cargo-allow") => CandidateClass::Accept,
        Some(policy) => CandidateClass::Skip(format!(
            "not cargo-allow dialect (unsupported policy `{policy}`)"
        )),
        None if header.schema_version.is_none()
            || header.schema_version.as_deref() == Some(SUPPORTED_SCHEMA_VERSION) =>
        {
            CandidateClass::Accept
        }
        None => CandidateClass::Skip(
            "not cargo-allow dialect (missing policy = \"cargo-allow\" marker)".to_string(),
        ),
    }
}

fn supported_schema_version(version: Option<&str>) -> bool {
    match version {
        None | Some(SUPPORTED_SCHEMA_VERSION) | Some(SUPPORTED_SCHEMA_VERSION_ALIAS) => true,
        Some(_) => false,
    }
}

#[cfg(test)]
mod tests;
