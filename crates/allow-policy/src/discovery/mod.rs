use std::fs;
use std::io::ErrorKind;
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
/// Provenance label for a Cargo manifest metadata attempt whose package or
/// workspace origin could not be distinguished.
pub const SOURCE_CARGO_METADATA: &str = "cargo_metadata";
/// Provenance label for a config selected from a conventional path.
pub const SOURCE_CONVENTIONAL_PATH: &str = "conventional_path";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPolicyCandidate {
    pub path: PathBuf,
    pub source: &'static str,
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
            match candidate_target_posture(&dir, &candidate) {
                CandidateTargetPosture::Missing => continue,
                CandidateTargetPosture::Rejected(reason) => {
                    skipped.push(SkippedPolicyCandidate {
                        path: candidate,
                        source: SOURCE_CONVENTIONAL_PATH,
                        reason,
                    });
                    continue;
                }
                CandidateTargetPosture::Contained(read_path) => {
                    match classify_candidate(&read_path, rel == NATIVE_LEDGER_REL_PATH) {
                        CandidateClass::Accept => {
                            // Warn if the native ledger (cargo-allow.toml) shadows a
                            // conventional allow.toml at the same directory level (#3232).
                            // The native path wins on discovery precedence, but an
                            // operator who just ran `init` (which writes allow.toml)
                            // may be confused that their new file is ignored.
                            if rel == NATIVE_LEDGER_REL_PATH {
                                let shadowed = dir.join("policy/allow.toml");
                                if matches!(
                                    candidate_target_posture(&dir, &shadowed),
                                    CandidateTargetPosture::Contained(_)
                                ) && shadowed != candidate
                                {
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
                            source: SOURCE_CONVENTIONAL_PATH,
                            reason,
                        }),
                    }
                    continue;
                }
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
    let manifest_read_path = match candidate_target_posture(dir, &manifest_path) {
        CandidateTargetPosture::Missing => return None,
        CandidateTargetPosture::Rejected(reason) => {
            skipped.push(SkippedPolicyCandidate {
                path: manifest_path,
                source: SOURCE_CARGO_METADATA,
                reason: format!("cargo-allow metadata {reason}"),
            });
            return None;
        }
        CandidateTargetPosture::Contained(path) => path,
    };
    let text = match read_text_file_capped(&manifest_read_path) {
        Ok(text) => text,
        Err(err) => {
            skipped.push(SkippedPolicyCandidate {
                path: manifest_path,
                source: SOURCE_CARGO_METADATA,
                reason: format!("cargo-allow metadata could not be read: {err}"),
            });
            return None;
        }
    };
    let manifest = match toml::from_str::<CargoManifestProbe>(&text) {
        Ok(manifest) => manifest,
        Err(_) => {
            skipped.push(SkippedPolicyCandidate {
                path: manifest_path,
                source: SOURCE_CARGO_METADATA,
                reason: "cargo-allow metadata could not be parsed".to_string(),
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
            source,
            reason: "cargo-allow metadata config must be a non-empty source-tree-relative path without parent traversal"
                .to_string(),
        });
        return None;
    }
    let candidate = dir.join(config_path);
    let candidate_read_path = match candidate_target_posture(dir, &candidate) {
        CandidateTargetPosture::Missing => {
            skipped.push(SkippedPolicyCandidate {
                path: candidate,
                source,
                reason: "cargo-allow metadata config path does not exist".to_string(),
            });
            return None;
        }
        CandidateTargetPosture::Rejected(reason) => {
            skipped.push(SkippedPolicyCandidate {
                path: candidate,
                source,
                reason: format!("cargo-allow metadata config {reason}"),
            });
            return None;
        }
        CandidateTargetPosture::Contained(path) => path,
    };
    match classify_candidate(&candidate_read_path, true) {
        CandidateClass::Accept => Some((candidate, source)),
        CandidateClass::Skip(reason) => {
            skipped.push(SkippedPolicyCandidate {
                path: candidate,
                source,
                reason: format!("cargo-allow metadata config {reason}"),
            });
            None
        }
    }
}

enum CandidateTargetPosture {
    Missing,
    Contained(PathBuf),
    Rejected(String),
}

fn candidate_target_posture(anchor: &Path, candidate: &Path) -> CandidateTargetPosture {
    match fs::symlink_metadata(candidate) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return CandidateTargetPosture::Missing;
        }
        Err(_) => {
            return CandidateTargetPosture::Rejected(
                "candidate metadata could not be inspected".to_string(),
            );
        }
    }
    let target = match candidate.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return CandidateTargetPosture::Rejected(
                "candidate target could not be resolved".to_string(),
            );
        }
    };
    let resolved_anchor = match anchor.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return CandidateTargetPosture::Rejected(
                "candidate discovery anchor could not be resolved".to_string(),
            );
        }
    };
    if target.strip_prefix(resolved_anchor).is_ok() {
        CandidateTargetPosture::Contained(target)
    } else {
        CandidateTargetPosture::Rejected(
            "candidate target resolves outside its discovery anchor".to_string(),
        )
    }
}

enum CandidateClass {
    Accept,
    Skip(String),
}

fn classify_candidate(path: &Path, native: bool) -> CandidateClass {
    let text = match read_text_file_capped(path) {
        Ok(text) => text,
        Err(_) => {
            return CandidateClass::Skip(
                "not cargo-allow dialect (policy config could not be read)".to_string(),
            );
        }
    };
    let header = match toml::from_str::<PolicyHeaderProbe>(&text) {
        Ok(header) => header,
        Err(_) => {
            return CandidateClass::Skip(
                "not cargo-allow dialect (policy header could not be parsed)".to_string(),
            );
        }
    };
    if !supported_schema_version(header.schema_version.as_deref()) {
        return CandidateClass::Skip(
            "not cargo-allow dialect (missing or unsupported schema_version)".to_string(),
        );
    }
    if native {
        return match header.policy.as_deref() {
            None | Some("cargo-allow") => CandidateClass::Accept,
            Some(_) => CandidateClass::Skip(
                "not cargo-allow dialect (unsupported policy marker)".to_string(),
            ),
        };
    }
    match header.policy.as_deref() {
        Some("cargo-allow") => CandidateClass::Accept,
        Some(_) => {
            CandidateClass::Skip("not cargo-allow dialect (unsupported policy marker)".to_string())
        }
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
