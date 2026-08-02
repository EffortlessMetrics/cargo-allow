use allow_core::{AllowConfig, CargoAllowResult};
use allow_policy::parse_policy_at;
use std::path::Path;

use crate::io::{legacy_table_at, read_policy};
pub use crate::loader_compat::{
    load_clippy_exceptions_compat_config, load_dependency_surface_compat_config,
    load_executable_compat_config, load_generated_compat_config, load_network_compat_config,
    load_no_panic_allowlist_compat_config, load_no_panic_baseline_compat_config,
    load_non_rust_compat_config, load_process_compat_config, load_unsafe_allowlist_compat_config,
    load_workflow_compat_config,
};
use crate::loader_legacy_dispatch::config_from_legacy_table;
pub use crate::loader_policy_dir::{
    load_legacy_policy_dir, load_legacy_policy_dir_with_non_rust_findings, migration_notes,
};
use crate::source_context::at_legacy_source;

pub fn load_legacy_or_canonical(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let path_label = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("legacy-policy");
    let result = (|| -> CargoAllowResult<AllowConfig> {
        let text = read_policy(path)?;
        if let Some(table) = legacy_table_at(Some(path), &text)?
            && let Some(config) = config_from_legacy_table(&table)
                .map_err(|err| at_legacy_source(err, path, &text))?
        {
            return Ok(config);
        }
        parse_policy_at(path, &text)
    })();

    // #1868: attach filename context without discarding the structured error
    // kind, source location, diagnostics, or causes from any load path.
    result.map_err(|err| err.with_message_prefix(format!("legacy file `{path_label}`: ")))
}

#[cfg(test)]
mod tests {
    use super::load_legacy_or_canonical;
    use std::fs;

    #[test]
    fn canonical_parse_errors_keep_filename_context() -> Result<(), String> {
        let dir = crate::test_support::fixture_dir();
        let path = dir.join("canonical-policy.toml");
        fs::write(&path, "policy = \"cargo-allow\"\nunknown_field = true\n")
            .map_err(|err| format!("write malformed canonical fixture: {err}"))?;

        let error = match load_legacy_or_canonical(&path) {
            Ok(_) => return Err("malformed canonical TOML unexpectedly loaded".to_string()),
            Err(error) => error,
        };
        let expected_path = path.display().to_string();
        let location = error
            .location()
            .ok_or_else(|| "canonical parse error should have a location".to_string())?;

        assert_eq!(location.path.as_deref(), Some(expected_path.as_str()));
        assert!(
            error
                .to_string()
                .contains("legacy file `canonical-policy.toml`"),
            "filename context should remain visible: {error}"
        );
        Ok(())
    }

    #[test]
    fn legacy_semantic_errors_keep_source_path_and_entry_line() -> Result<(), String> {
        let dir = crate::test_support::fixture_dir();
        let path = dir.join("network-allowlist.toml");
        let source = "policy = \"network-allowlist\"\nowner = \"repo\"\nstatus = \"advisory\"\n\n[[allow]]\nid = \"net-missing-auth\"\ndestination = \"crates.io\"\nlane = \"build\"\nowner = \"ci\"\nreason = \"Build lane fetches public crates.\"\ncreated = \"2026-05-09\"\n";
        fs::write(&path, source).map_err(|err| format!("write malformed legacy fixture: {err}"))?;

        let error = match load_legacy_or_canonical(&path) {
            Ok(_) => return Err("malformed legacy entry unexpectedly loaded".to_string()),
            Err(error) => error,
        };
        let expected_path = path.display().to_string();
        let location = error
            .location()
            .ok_or_else(|| "semantic legacy error should have a source location".to_string())?;

        assert_eq!(location.path.as_deref(), Some(expected_path.as_str()));
        assert_eq!(location.line, 6);
        assert!(
            error
                .message()
                .contains(&format!("legacy source {expected_path}:6:")),
            "semantic error should include path and line: {}",
            error.message()
        );
        assert!(
            error
                .message()
                .contains("net-missing-auth missing auth_required")
        );
        Ok(())
    }
}
