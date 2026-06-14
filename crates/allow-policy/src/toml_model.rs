use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use serde::Deserialize;

use crate::toml_entry::AllowEntryToml;
use crate::toml_requirements::RequirementsToml;
use crate::toml_workspace::WorkspaceToml;

#[derive(Debug, Default, Deserialize)]
struct PolicyToml {
    schema_version: Option<String>,
    policy: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    #[serde(default)]
    workspace: WorkspaceToml,
    #[serde(default)]
    requirements: RequirementsToml,
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
            workspace: self.workspace.into_workspace_config(),
            requirements: self.requirements.into_requirements(),
            allow,
        })
    }
}

pub(crate) fn parse_policy_toml(input: &str) -> CargoAllowResult<AllowConfig> {
    let raw = toml::from_str::<PolicyToml>(input)
        .map_err(|e| CargoAllowError::new(format!("failed to parse policy TOML: {e}")))?;
    raw.into_config()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let cfg = parse_policy_toml(
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
        let err = parse_policy_toml(
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
    fn parse_policy_toml_reports_toml_parse_errors() {
        let invalid = "policy = [";
        let e =
            toml::from_str::<PolicyToml>(invalid).expect_err("invalid TOML should fail parsing");

        assert_eq!(
            parse_policy_toml(invalid).map(|_| ()),
            Err(CargoAllowError::new(format!(
                "failed to parse policy TOML: {e}"
            )))
        );
    }
}
