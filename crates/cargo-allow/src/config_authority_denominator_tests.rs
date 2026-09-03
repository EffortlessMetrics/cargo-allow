//! Checked denominator for current cargo-allow configuration authority (#3876).
//!
//! The canonical resolver and the few existing observation/loading owners are
//! declared in `policy/config-authority-consumers.toml`. This guard prevents a
//! new command module from quietly adding another selector or policy loader.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const DENOMINATOR: &str = "policy/config-authority-consumers.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Denominator {
    schema: String,
    schema_version: u32,
    controlling_issue: u32,
    guard: String,
    scan_roots: Vec<String>,
    consumers: Vec<Consumer>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Consumer {
    id: String,
    path: String,
    role: String,
    allowed_markers: Vec<String>,
    claim_boundary: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_file(root: &Path, relative: &str) -> Result<String, String> {
    fs::read_to_string(root.join(relative)).map_err(|error| format!("read {relative}: {error}"))
}

fn parse_denominator(root: &Path) -> Result<Denominator, String> {
    let source = read_file(root, DENOMINATOR)?;
    toml::from_str(&source).map_err(|error| format!("parse {DENOMINATOR}: {error}"))
}

fn contains_call(source: &str, marker: &str) -> bool {
    source.match_indices(marker).any(|(index, _)| {
        source
            .char_indices()
            .rev()
            .find(|(position, _)| *position < index)
            .map(|(_, character)| character)
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_')
    })
}

fn tracked_rust_sources(
    root: &Path,
    scan_roots: &[String],
) -> Result<Vec<(String, String)>, String> {
    let paths = allow_inventory::git_ls_files(root)
        .map_err(|error| format!("list tracked files: {error}"))?;
    paths
        .into_iter()
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("rs"))
        .filter(|path| {
            let relative = path.to_string_lossy().replace('\\', "/");
            scan_roots.iter().any(|scan_root| {
                let scan_root = scan_root.trim_end_matches('/');
                relative == scan_root || relative.starts_with(&format!("{scan_root}/"))
            })
        })
        .filter(|path| {
            !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_tests.rs") || name == "tests.rs")
        })
        .map(|path| {
            let relative = path.to_string_lossy().replace('\\', "/");
            let source = read_file(root, &relative)?;
            Ok((relative, source))
        })
        .collect()
}

#[test]
fn config_authority_denominator_is_current_and_blocks_new_shadow_resolvers() -> Result<(), String> {
    let root = repo_root();
    let denominator = parse_denominator(&root)?;
    if denominator.schema != "cargo-allow.config-authority-consumers.v1"
        || denominator.schema_version != 1
        || denominator.controlling_issue != 3876
        || denominator.guard != "config_authority_denominator_tests"
    {
        return Err("configuration authority denominator metadata drifted".to_string());
    }
    if denominator.scan_roots.is_empty() || denominator.consumers.is_empty() {
        return Err(
            "configuration authority denominator must declare scan roots and consumers".to_string(),
        );
    }

    let mut by_path = BTreeMap::new();
    for consumer in denominator.consumers {
        if consumer.id.trim().is_empty()
            || consumer.role.trim().is_empty()
            || consumer.claim_boundary.trim().is_empty()
            || consumer.allowed_markers.is_empty()
        {
            return Err(format!("consumer {:?} is incomplete", consumer.id));
        }
        let path = consumer.path.replace('\\', "/");
        if !path.starts_with("crates/cargo-allow/src/") {
            return Err(format!(
                "consumer {} escapes the cargo-allow source root",
                consumer.id
            ));
        }
        if by_path.insert(path.clone(), consumer).is_some() {
            return Err(format!("duplicate configuration consumer path {path}"));
        }
    }

    let sources = tracked_rust_sources(&root, &denominator.scan_roots)?;
    let mut observed = BTreeSet::new();
    for (path, source) in sources {
        let markers = by_path.get(&path);
        for marker in [
            "discover_config_path(",
            "select_policy_path(",
            "evaluate_source_exception_policy(",
            "load_policy_at_path_with_digest(",
            "load_policy_with_reportable_evidence",
            "config_path(",
        ] {
            if !contains_call(&source, marker) {
                continue;
            }
            let Some(consumer) = markers else {
                return Err(format!(
                    "unlisted configuration authority marker {marker} in {path}"
                ));
            };
            if !consumer
                .allowed_markers
                .iter()
                .any(|allowed| allowed == marker)
            {
                return Err(format!(
                    "configuration consumer {} is not permitted to use {marker}",
                    consumer.id
                ));
            }
            observed.insert(path.clone());
        }
    }

    for path in by_path.keys() {
        if !root.join(path).is_file() {
            return Err(format!("denominator consumer path is missing: {path}"));
        }
    }
    let declared = by_path.keys().cloned().collect::<BTreeSet<_>>();
    if observed != declared {
        return Err(format!(
            "configuration authority denominator drifted; observed {observed:?}, declared {declared:?}"
        ));
    }
    Ok(())
}
