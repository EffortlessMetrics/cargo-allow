use std::path::{Component, Path, PathBuf};

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

/// Provenance label for a config selected from package metadata.
pub const SOURCE_PACKAGE_METADATA: &str = "package_metadata";
/// Provenance label for a config selected from workspace metadata.
pub const SOURCE_WORKSPACE_METADATA: &str = "workspace_metadata";
/// Provenance label for a config selected from a conventional path.
pub const SOURCE_CONVENTIONAL_PATH: &str = "conventional_path";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPolicyCandidate {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverConfigResult {
    pub selected: Option<PathBuf>,
    pub selected_source: Option<&'static str>,
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
                selected_source: None,
                skipped: Vec::new(),
            };
        }
    };
    let mut skipped = Vec::new();
    loop {
        if let Some((candidate, source)) = discover_cargo_metadata_config(&dir, &mut skipped) {
            return DiscoverConfigResult {
                selected: Some(candidate),
                selected_source: Some(source),
                skipped,
            };
        }
        for rel in DISCOVERY_REL_PATHS {
            let candidate = dir.join(rel);
            if !candidate.exists() {
                continue;
            }
            match classify_candidate(&candidate, rel == NATIVE_LEDGER_REL_PATH) {
                CandidateClass::Accept => {
                    // Warn if the native ledger (cargo-allow.toml) shadows a
                    // conventional allow.toml at the same directory level (#3232).
                    // The native path wins on discovery precedence, but an
                    // operator who just ran `init` (which writes allow.toml)
                    // may be confused that their new file is ignored.
                    if rel == NATIVE_LEDGER_REL_PATH {
                        let shadowed = dir.join("policy/allow.toml");
                        if shadowed.exists() && shadowed != candidate {
                            eprintln!(
                                "warning: {} is shadowed by the native ledger {} on the discovery path; \
                                 only {} will be used (#3232)",
                                shadowed.display(),
                                candidate.display(),
                                candidate.display(),
                            );
                        }
                    }
                    return DiscoverConfigResult {
                        selected: Some(candidate),
                        selected_source: Some(SOURCE_CONVENTIONAL_PATH),
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
        selected_source: None,
        skipped,
    }
}

pub(crate) fn skipped_metadata_candidate_source(
    start: &Path,
    candidate: &Path,
) -> Option<&'static str> {
    let mut dir = start.canonicalize().ok()?;
    loop {
        let manifest_path = dir.join("Cargo.toml");
        if manifest_path.exists()
            && let Ok(text) = read_text_file_capped(&manifest_path)
            && let Ok(manifest) = toml::from_str::<CargoManifestProbe>(&text)
        {
            let package = manifest
                .package
                .as_ref()
                .and_then(|package| package.metadata.as_ref())
                .and_then(|metadata| metadata.cargo_allow.as_ref())
                .and_then(|metadata| metadata.config.as_deref())
                .map(|config| (config, SOURCE_PACKAGE_METADATA));
            let workspace = manifest
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.metadata.as_ref())
                .and_then(|metadata| metadata.cargo_allow.as_ref())
                .and_then(|metadata| metadata.config.as_deref())
                .map(|config| (config, SOURCE_WORKSPACE_METADATA));
            if let Some((config, source)) = package.or(workspace)
                && (dir.join(config) == candidate || manifest_path == candidate)
            {
                return Some(source);
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct CargoManifestProbe {
    package: Option<CargoPackageProbe>,
    workspace: Option<CargoWorkspaceProbe>,
}

#[derive(Debug, Default, Deserialize)]
struct CargoPackageProbe {
    metadata: Option<CargoMetadataProbe>,
}

#[derive(Debug, Default, Deserialize)]
struct CargoWorkspaceProbe {
    metadata: Option<CargoMetadataProbe>,
}

#[derive(Debug, Default, Deserialize)]
struct CargoMetadataProbe {
    #[serde(rename = "cargo-allow")]
    cargo_allow: Option<CargoAllowMetadataProbe>,
}

#[derive(Debug, Default, Deserialize)]
struct CargoAllowMetadataProbe {
    config: Option<String>,
}

fn discover_cargo_metadata_config(
    dir: &Path,
    skipped: &mut Vec<SkippedPolicyCandidate>,
) -> Option<(PathBuf, &'static str)> {
    let manifest_path = dir.join("Cargo.toml");
    if !manifest_path.exists() {
        return None;
    }
    let text = match read_text_file_capped(&manifest_path) {
        Ok(text) => text,
        Err(err) => {
            skipped.push(SkippedPolicyCandidate {
                path: manifest_path,
                reason: format!("cargo-allow metadata could not be read: {err}"),
            });
            return None;
        }
    };
    let manifest = match toml::from_str::<CargoManifestProbe>(&text) {
        Ok(manifest) => manifest,
        Err(err) => {
            skipped.push(SkippedPolicyCandidate {
                path: manifest_path,
                reason: format!("cargo-allow metadata could not be parsed: {err}"),
            });
            return None;
        }
    };
    let (config, source) = manifest
        .package
        .and_then(|package| package.metadata)
        .and_then(|metadata| metadata.cargo_allow)
        .and_then(|metadata| metadata.config)
        .map(|config| (config, SOURCE_PACKAGE_METADATA))
        .or_else(|| {
            manifest
                .workspace
                .and_then(|workspace| workspace.metadata)
                .and_then(|metadata| metadata.cargo_allow)
                .and_then(|metadata| metadata.config)
                .map(|config| (config, SOURCE_WORKSPACE_METADATA))
        })?;
    let config_path = Path::new(&config);
    if config.is_empty()
        || config_path.is_absolute()
        || config_path
            .components()
            .any(|component| component == Component::ParentDir)
    {
        skipped.push(SkippedPolicyCandidate {
            path: manifest_path,
            reason: format!(
                "cargo-allow metadata config `{config}` must be a non-empty relative path without `..`"
            ),
        });
        return None;
    }
    let candidate = dir.join(config_path);
    if !candidate.exists() {
        skipped.push(SkippedPolicyCandidate {
            path: candidate,
            reason: "cargo-allow metadata config path does not exist".to_string(),
        });
        return None;
    }
    match classify_candidate(&candidate, true) {
        CandidateClass::Accept => Some((candidate, source)),
        CandidateClass::Skip(reason) => {
            skipped.push(SkippedPolicyCandidate {
                path: candidate,
                reason: format!("cargo-allow metadata config {reason}"),
            });
            None
        }
    }
}

enum CandidateClass {
    Accept,
    Skip(String),
}

fn classify_candidate(path: &Path, native: bool) -> CandidateClass {
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
    if native {
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
