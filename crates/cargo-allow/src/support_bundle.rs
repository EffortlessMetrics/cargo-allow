use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use serde::Serialize;
use std::path::Path;

pub(crate) const SUPPORT_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub(crate) const SUPPORT_BUNDLE_SCHEMA_ID: &str = "cargo-allow.support-bundle.v1";

const CLAIM_BOUNDARY: &[&str] = &[
    "bounded_support_diagnostic",
    "source_tree_inventory_metadata_only",
    "repository_relative_paths_only",
    "no_source_contents",
    "no_policy_contents",
    "no_environment_dump",
    "no_network_or_upload",
];

const EXCLUDED_DATA: &[&str] = &[
    "source_file_contents",
    "policy_reasons_and_evidence",
    "credentials_and_tokens",
    "environment_variables",
    "git_remotes",
    "private_absolute_paths",
    "unowned_artifacts",
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct SupportBundleFacts<'a> {
    pub(crate) root_discovery: &'a str,
    pub(crate) repository_kind: &'a str,
    pub(crate) config_found: bool,
    pub(crate) config_path: Option<&'a str>,
    pub(crate) config_schema_version: Option<&'a str>,
    pub(crate) config_valid: Option<bool>,
    pub(crate) inventory_source: &'a str,
    pub(crate) inventory_completeness: &'a str,
    pub(crate) files_scanned: usize,
    pub(crate) deleted_tracked_files: usize,
    pub(crate) skipped_paths: usize,
    pub(crate) submodule_paths: usize,
    pub(crate) federation_found: bool,
    pub(crate) federation_valid: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SupportBundle<'a> {
    schema_version: u32,
    schema_id: &'static str,
    tool: &'static str,
    command: &'static str,
    result: &'static str,
    claim_boundary: &'static [&'static str],
    excluded_data: &'static [&'static str],
    platform: Platform,
    root: Root<'a>,
    config: Config<'a>,
    inventory: Inventory<'a>,
    federation: Federation,
}

#[derive(Debug, Serialize)]
struct Platform {
    os: &'static str,
    architecture: &'static str,
}

#[derive(Debug, Serialize)]
struct Root<'a> {
    discovery: &'a str,
    repository_kind: &'a str,
    path: &'static str,
}

#[derive(Debug, Serialize)]
struct Config<'a> {
    found: bool,
    path: Option<&'a str>,
    schema_version: Option<&'a str>,
    valid: Option<bool>,
}

#[derive(Debug, Serialize)]
struct Inventory<'a> {
    source: &'a str,
    completeness: &'a str,
    files_scanned: usize,
    deleted_tracked_files: usize,
    skipped_paths: usize,
    submodule_paths: usize,
}

#[derive(Debug, Serialize)]
struct Federation {
    found: bool,
    valid: Option<bool>,
}

pub(crate) fn write_support_bundle(
    root: &Path,
    output: &Path,
    facts: SupportBundleFacts<'_>,
) -> CargoAllowResult<()> {
    crate::assert_path_within_root(root, output)?;
    let json = render_support_bundle_json(facts)?;
    crate::write_file(output, &format!("{json}\n"))
        .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)
}

fn render_support_bundle_json(facts: SupportBundleFacts<'_>) -> CargoAllowResult<String> {
    let bundle = SupportBundle {
        schema_version: SUPPORT_BUNDLE_SCHEMA_VERSION,
        schema_id: SUPPORT_BUNDLE_SCHEMA_ID,
        tool: "cargo-allow",
        command: "doctor --support-bundle",
        result: "BundleComplete",
        claim_boundary: CLAIM_BOUNDARY,
        excluded_data: EXCLUDED_DATA,
        platform: Platform {
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        },
        root: Root {
            discovery: facts.root_discovery,
            repository_kind: facts.repository_kind,
            path: "<redacted>",
        },
        config: Config {
            found: facts.config_found,
            path: facts.config_path,
            schema_version: facts.config_schema_version,
            valid: facts.config_valid,
        },
        inventory: Inventory {
            source: facts.inventory_source,
            completeness: facts.inventory_completeness,
            files_scanned: facts.files_scanned,
            deleted_tracked_files: facts.deleted_tracked_files,
            skipped_paths: facts.skipped_paths,
            submodule_paths: facts.submodule_paths,
        },
        federation: Federation {
            found: facts.federation_found,
            valid: facts.federation_valid,
        },
    };
    serde_json::to_string_pretty(&bundle).map_err(support_bundle_json_error)
}

fn support_bundle_json_error(error: serde_json::Error) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Artifact,
        format!("failed to render support bundle: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    fn facts() -> SupportBundleFacts<'static> {
        SupportBundleFacts {
            root_discovery: "nearest_git_root",
            repository_kind: "git",
            config_found: true,
            config_path: Some("policy/allow.toml"),
            config_schema_version: Some("0.1"),
            config_valid: Some(true),
            inventory_source: "git_tracked",
            inventory_completeness: "scoped",
            files_scanned: 42,
            deleted_tracked_files: 1,
            skipped_paths: 2,
            submodule_paths: 0,
            federation_found: false,
            federation_valid: None,
        }
    }

    #[test]
    fn support_bundle_is_schema_valid_and_redacted() -> Result<(), String> {
        let json = render_support_bundle_json(facts()).map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_str(&json).map_err(|error| error.to_string())?;
        let schema: Value = serde_json::from_str(include_str!(
            "../../../docs/schemas/support-bundle.schema.json"
        ))
        .map_err(|error| error.to_string())?;
        let validator = jsonschema::validator_for(&schema).map_err(|error| error.to_string())?;
        validator
            .validate(&value)
            .map_err(|error| error.to_string())?;
        if json.contains("H:/Code/Rust")
            || json.contains("secret-token")
            || json.contains("reason text")
        {
            return Err("support bundle leaked a forbidden value".to_string());
        }
        if !json.contains("<redacted>") || !json.contains("source_file_contents") {
            return Err("support bundle did not state its redaction boundary".to_string());
        }
        Ok(())
    }

    #[test]
    fn support_bundle_json_errors_are_artifacts() -> Result<(), String> {
        let serialization_error = serde_json::from_str::<Value>("{")
            .expect_err("incomplete JSON should produce a serialization error");
        let error = support_bundle_json_error(serialization_error);
        assert_eq!(error.kind(), CargoAllowErrorKind::Artifact);
        assert!(
            error
                .to_string()
                .contains("failed to render support bundle")
        );
        Ok(())
    }

    #[test]
    fn support_bundle_rejects_output_outside_root() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("cargo-allow-support-bundle-{}", std::process::id()));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let parent = root
            .parent()
            .ok_or_else(|| "temporary root should have a parent".to_string())?;
        let output = parent.join("cargo-allow-support-bundle-outside.json");
        let result = write_support_bundle(&root, &output, facts());
        let _ = fs::remove_file(&output);
        let _ = fs::remove_dir(&root);
        match result {
            Ok(()) => Err("support bundle accepted an outside output path".to_string()),
            Err(error) if error.to_string().contains("outside the source-tree root") => Ok(()),
            Err(error) => Err(format!("unexpected error: {error}")),
        }
    }
}
