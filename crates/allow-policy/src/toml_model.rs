use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

use crate::toml_de::option_schema_version;
use crate::toml_entry::AllowEntryToml;
use crate::toml_lanes::LanesToml;
use crate::toml_requirements::RequirementsToml;
use crate::toml_workspace::WorkspaceToml;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyToml {
    #[serde(default, deserialize_with = "option_schema_version")]
    schema_version: Option<String>,
    policy: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    #[serde(default)]
    workspace: WorkspaceToml,
    #[serde(default)]
    requirements: RequirementsToml,
    #[serde(default)]
    lanes: LanesToml,
    #[serde(default)]
    allow: Vec<AllowEntryToml>,
}

impl PolicyToml {
    fn into_config(self) -> CargoAllowResult<AllowConfig> {
        let allow = self
            .allow
            .into_iter()
            .enumerate()
            .map(|(index, entry)| entry.into_allow_entry(index))
            .collect::<CargoAllowResult<Vec<_>>>()?;
        Ok(AllowConfig {
            schema_version: self.schema_version.unwrap_or_else(|| "0.1".to_string()),
            policy: self.policy.unwrap_or_else(|| "cargo-allow".to_string()),
            owner: self.owner,
            status: self.status,
            workspace: self.workspace.into_workspace_config()?,
            requirements: self.requirements.into_requirements(),
            lanes: self.lanes.into_lane_configs()?,
            allow,
        })
    }
}

pub(crate) fn parse_policy_toml_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<AllowConfig> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        let location = path
            .map(|p| format!(" in {}", p.display()))
            .unwrap_or_default();
        return Err(CargoAllowError::new(format!(
            "policy file{location} is empty; an accidentally emptied or truncated ledger parses as a permissive state"
        )));
    }
    // Strip leading UTF-8 BOM so Windows-saved policy files parse correctly.
    // The toml crate treats \u{FEFF} as part of the first bare key, making
    // schema_version unparseable and causing the file to be skipped as a
    // foreign dialect during discovery (#2003).
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let raw = toml::from_str::<PolicyToml>(input).map_err(|e| {
        let message = match path {
            Some(path) => format!("failed to parse policy TOML in {}: {e}", path.display()),
            None => format!("failed to parse policy TOML: {e}"),
        };
        CargoAllowError::with_kind(allow_core::CargoAllowErrorKind::InvalidPolicy, message)
            .with_toml_span(path, input, e.span())
    })?;
    raw.into_config()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_policy_file_is_rejected_not_silent_defaults() {
        // Regression for #2002: empty/whitespace-only policy must not
        // silently parse as a permissive empty ledger.
        assert!(parse_policy_toml_at(None, "").is_err());
        assert!(parse_policy_toml_at(None, "   \n  \n").is_err());
    }

    #[test]
    fn policy_toml_into_config_applies_header_and_empty_defaults() {
        let cfg = PolicyToml::default()
            .into_config()
            .unwrap_or_else(|err| std::panic::panic_any(format!("defaults convert: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner, None);
        assert_eq!(cfg.status, None);
        assert_eq!(cfg.workspace.root, ".");
        assert_eq!(cfg.workspace.inventory, "git-tracked");
        assert_eq!(cfg.workspace.default_mode, "no-new");
        assert_eq!(cfg.workspace.ignored, vec![".git/**", "target/**"]);
        assert_eq!(cfg.workspace.generated, vec!["target/**", "vendor/**"]);
        assert!(cfg.requirements.owner_required);
        assert!(cfg.requirements.reason_required);
        assert!(cfg.requirements.classification_required);
        assert!(!cfg.requirements.evidence_required);
        assert!(cfg.requirements.expires_or_review_after_required);
        assert!(!cfg.requirements.allow_bare_allow_attributes);
        assert!(!cfg.requirements.lint_policy_id_required);
        assert!(!cfg.requirements.stale_entries_fail);
        assert!(cfg.requirements.unsafe_evidence_required);
        assert!(!cfg.requirements.unsafe_safety_comment_required);
        assert!(cfg.allow.is_empty());
    }

    #[test]
    fn parse_policy_toml_preserves_workspace_requirements_and_entries() {
        let cfg = parse_policy_toml_at(
            None,
            r#"
schema_version = "0.2"
policy = "cargo-allow"
owner = "policy"
status = "advisory"

[workspace]
root = "."
inventory = "git_tracked"
default_mode = "audit"
ignored = [".git/**"]
generated = ["target/**"]

[[workspace.file_family]]
id = "model-artifact"
family = "ml_model"
glob = "models/**/*.onnx"
reason = "Govern versioned model artifacts."

[requirements]
owner_required = true
reason_required = true
evidence_required = true

[requirements.unsafe]
safety_comment_required = true

[[allow]]
id = "allow-unsafe"
kind = "unsafe"
family = "unsafe_block"
path = "src/lib.rs"
owner = "runtime"
classification = "reviewed"
reason = "Fixture policy."
evidence = ["test:fixture"]
expires = "2026-12-31"

[allow.selector]
ast_kind = "unsafe_block"
container = "load"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

        assert_eq!(cfg.schema_version, "0.2");
        assert_eq!(cfg.owner.as_deref(), Some("policy"));
        assert_eq!(cfg.status.as_deref(), Some("advisory"));
        assert_eq!(cfg.workspace.inventory, "git-tracked");
        assert_eq!(cfg.workspace.default_mode, "audit");
        assert_eq!(cfg.workspace.ignored, vec![".git/**"]);
        assert_eq!(cfg.workspace.generated, vec!["target/**"]);
        assert_eq!(cfg.workspace.file_families.len(), 1);
        assert_eq!(cfg.workspace.file_families[0].id, "model-artifact");
        assert_eq!(cfg.workspace.file_families[0].family, "ml_model");
        assert_eq!(cfg.workspace.file_families[0].glob, "models/**/*.onnx");
        assert_eq!(
            cfg.workspace.file_families[0].reason,
            "Govern versioned model artifacts."
        );
        assert!(cfg.requirements.owner_required);
        assert!(cfg.requirements.reason_required);
        assert!(cfg.requirements.evidence_required);
        assert!(cfg.requirements.unsafe_safety_comment_required);

        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("allow entry exists"));
        assert_eq!(entry.id, "allow-unsafe");
        assert_eq!(entry.kind.as_str(), "unsafe");
        assert_eq!(entry.family.as_deref(), Some("unsafe_block"));
        assert_eq!(
            entry.path.as_deref(),
            Some(std::path::Path::new("src/lib.rs"))
        );
        assert_eq!(entry.owner, "runtime");
        assert_eq!(entry.classification, "reviewed");
        assert_eq!(entry.reason, "Fixture policy.");
        assert_eq!(entry.evidence, vec!["test:fixture"]);
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2026-12-31"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("unsafe_block"));
        assert_eq!(entry.selector.container.as_deref(), Some("load"));
    }

    #[test]
    fn policy_toml_into_config_reports_allow_entry_conversion_errors() {
        let err = parse_policy_toml_at(
            None,
            r#"
policy = "cargo-allow"

[[allow]]
id = "missing-kind"
owner = "policy"
"#,
        )
        .expect_err("missing kind should fail conversion");

        assert!(err.to_string().contains("missing-kind missing kind"));
    }

    #[test]
    fn parse_policy_toml_accepts_integer_schema_version() {
        let cfg = parse_policy_toml_at(
            None,
            r#"
schema_version = 1
policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("integer schema parses: {err}")));

        assert_eq!(cfg.schema_version, "1");
    }

    #[test]
    fn parse_policy_toml_preserves_lane_posture() {
        let cfg = parse_policy_toml_at(
            None,
            r#"
policy = "cargo-allow"

[lanes.panic]
mode = "blocking"

[lanes.unsafe]
mode = "shadow"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

        assert_eq!(
            cfg.lane_enforcement_mode_for_kind(allow_core::FindingKind::Panic),
            allow_core::LaneEnforcementMode::Blocking
        );
        assert_eq!(
            cfg.lane_enforcement_mode_for_kind(allow_core::FindingKind::Unsafe),
            allow_core::LaneEnforcementMode::Shadow
        );
    }

    #[test]
    fn parse_policy_toml_at_includes_path_in_parse_errors() {
        let err = parse_policy_toml_at(
            Some(std::path::Path::new("policy/allow.toml")),
            "schema_version = [",
        )
        .expect_err("invalid TOML should include the ledger path");

        assert!(err.to_string().contains("policy/allow.toml"));
    }

    #[test]
    fn parse_policy_toml_at_preserves_structured_parse_location() -> Result<(), String> {
        let err = match parse_policy_toml_at(
            Some(std::path::Path::new("policy/allow.toml")),
            "policy = \"cargo-allow\"\nowner = [",
        ) {
            Ok(_) => return Err("invalid TOML unexpectedly parsed".to_string()),
            Err(err) => err,
        };

        let location = err
            .location()
            .ok_or_else(|| "policy parse error should have a location".to_string())?;
        assert_eq!(location.path.as_deref(), Some("policy/allow.toml"));
        assert_eq!(location.line, 2);
        assert!(location.column > 0);
        Ok(())
    }

    #[test]
    fn parse_policy_toml_reports_toml_parse_errors() -> Result<(), String> {
        let invalid = "policy = [";
        let e =
            toml::from_str::<PolicyToml>(invalid).expect_err("invalid TOML should fail parsing");
        let err = match parse_policy_toml_at(None, invalid) {
            Ok(_) => return Err("invalid TOML unexpectedly parsed".to_string()),
            Err(err) => err,
        };

        assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::InvalidPolicy);
        assert_eq!(err.message(), format!("failed to parse policy TOML: {e}"));
        let location = err
            .location()
            .ok_or_else(|| "parse error should preserve TOML location".to_string())?;
        assert_eq!(location.line, 1);
        assert_eq!(location.column, 11);
        Ok(())
    }

    #[test]
    fn parse_policy_toml_rejects_unknown_top_level_field_typo() {
        let err = parse_policy_toml_at(
            None,
            r#"
polcy = "cargo-allow"
"#,
        )
        .expect_err("unknown top-level field should be rejected");

        assert!(
            err.to_string().contains("unknown field"),
            "error should mention unknown field: {err}"
        );
    }

    #[test]
    fn parse_policy_toml_rejects_unknown_selector_field_typo() {
        let err = parse_policy_toml_at(
            None,
            r#"
policy = "cargo-allow"

[[allow]]
id = "allow-0001"
kind = "non_rust_file"
path = "README.md"
owner = "core"
classification = "fixture"
reason = "fixture"

[allow.selector]
modlue = "alpha"
"#,
        )
        .expect_err("unknown selector field should be rejected");

        assert!(
            err.to_string().contains("unknown field"),
            "error should mention unknown field: {err}"
        );
    }

    #[test]
    fn parse_policy_toml_rejects_unknown_workspace_field_typo() {
        let err = parse_policy_toml_at(
            None,
            r#"
policy = "cargo-allow"

[workspace]
defult_mode = "no-new"
"#,
        )
        .expect_err("unknown workspace field should be rejected");

        assert!(
            err.to_string().contains("unknown field"),
            "error should mention unknown field: {err}"
        );
    }

    #[test]
    fn parse_policy_toml_rejects_unknown_file_family_field_typo() {
        let err = parse_policy_toml_at(
            None,
            r#"
policy = "cargo-allow"

[[workspace.file_family]]
id = "model-artifact"
family = "ml_model"
glob = "models/**/*.onnx"
reasn = "typo"
"#,
        )
        .expect_err("unknown file-family field should be rejected");

        assert!(
            err.to_string().contains("unknown field"),
            "error should mention unknown field: {err}"
        );
    }
}
