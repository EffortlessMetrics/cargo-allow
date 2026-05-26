use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, LastSeen, Lifecycle,
    Requirements, Selector, WorkspaceConfig,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

#[derive(Debug, Default, Deserialize)]
struct WorkspaceToml {
    root: Option<String>,
    inventory: Option<String>,
    default_mode: Option<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    ignored: Vec<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    generated: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    owner_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    reason_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    classification_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    expires_or_review_after_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    allow_bare_allow_attributes: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    stale_entries_fail: Option<bool>,
    #[serde(default, rename = "unsafe")]
    unsafe_requirements: UnsafeRequirementsToml,
}

#[derive(Debug, Default, Deserialize)]
struct UnsafeRequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    evidence_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    safety_comment_required: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct AllowEntryToml {
    id: Option<String>,
    kind: Option<String>,
    family: Option<String>,
    path: Option<PathBuf>,
    glob: Option<String>,
    owner: Option<String>,
    classification: Option<String>,
    #[serde(alias = "explanation")]
    reason: Option<String>,
    #[serde(default, alias = "covered_by", deserialize_with = "string_or_vec")]
    evidence: Vec<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    links: Vec<String>,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
    #[serde(default)]
    selector: SelectorToml,
    #[serde(default)]
    last_seen: LastSeenToml,
}

#[derive(Debug, Default, Deserialize)]
struct SelectorToml {
    #[serde(alias = "kind")]
    ast_kind: Option<String>,
    container: Option<String>,
    callee: Option<String>,
    #[serde(alias = "macro")]
    macro_name: Option<String>,
    lint: Option<String>,
    symbol: Option<String>,
    receiver_fingerprint: Option<String>,
    target_fingerprint: Option<String>,
    normalized_snippet_hash: Option<String>,
    #[serde(default, deserialize_with = "option_u32_or_string")]
    line_hint: Option<u32>,
    glob: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LastSeenToml {
    #[serde(default, deserialize_with = "option_u32_or_string")]
    line: Option<u32>,
    #[serde(default, deserialize_with = "option_u32_or_string")]
    column: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringList {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Boolish {
    Bool(bool),
    String(String),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum U32ish {
    Number(u32),
    String(String),
}

pub fn find_config(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut dir = start.as_ref().canonicalize().ok()?;
    loop {
        for rel in ["policy/allow.toml", ".cargo/allow.toml", "allow.toml"] {
            let candidate = dir.join(rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub fn load_policy(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = fs::read_to_string(path.as_ref()).map_err(|e| {
        CargoAllowError::new(format!("failed to read {}: {e}", path.as_ref().display()))
    })?;
    parse_policy(&text)
}

pub fn parse_policy(input: &str) -> CargoAllowResult<AllowConfig> {
    let raw = toml::from_str::<PolicyToml>(input)
        .map_err(|e| CargoAllowError::new(format!("failed to parse policy TOML: {e}")))?;
    let cfg = raw.into_config()?;
    validate_policy(&cfg)?;
    Ok(cfg)
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

impl WorkspaceToml {
    fn into_workspace_config(self) -> WorkspaceConfig {
        let default = WorkspaceConfig::default();
        WorkspaceConfig {
            root: self.root.unwrap_or(default.root),
            inventory: self.inventory.unwrap_or(default.inventory),
            ignored: if self.ignored.is_empty() {
                default.ignored
            } else {
                self.ignored
            },
            generated: if self.generated.is_empty() {
                default.generated
            } else {
                self.generated
            },
            default_mode: self.default_mode.unwrap_or(default.default_mode),
        }
    }
}

impl RequirementsToml {
    fn into_requirements(self) -> Requirements {
        let default = Requirements::default();
        Requirements {
            owner_required: self.owner_required.unwrap_or(default.owner_required),
            reason_required: self.reason_required.unwrap_or(default.reason_required),
            classification_required: self
                .classification_required
                .unwrap_or(default.classification_required),
            expires_or_review_after_required: self
                .expires_or_review_after_required
                .unwrap_or(default.expires_or_review_after_required),
            allow_bare_allow_attributes: self
                .allow_bare_allow_attributes
                .unwrap_or(default.allow_bare_allow_attributes),
            stale_entries_fail: self
                .stale_entries_fail
                .unwrap_or(default.stale_entries_fail),
            unsafe_evidence_required: self
                .unsafe_requirements
                .evidence_required
                .unwrap_or(default.unsafe_evidence_required),
            unsafe_safety_comment_required: self
                .unsafe_requirements
                .safety_comment_required
                .unwrap_or(default.unsafe_safety_comment_required),
        }
    }
}

impl AllowEntryToml {
    fn into_allow_entry(self, index: usize) -> CargoAllowResult<AllowEntry> {
        let id = self.id.unwrap_or_else(|| format!("allow-{:04}", index + 1));
        let kind_text = self
            .kind
            .ok_or_else(|| CargoAllowError::new(format!("{id} missing kind")))?;
        let kind = FindingKind::from_str(&kind_text)?;
        let last_seen = match (self.last_seen.line, self.last_seen.column) {
            (Some(line), Some(column)) => Some(LastSeen { line, column }),
            _ => None,
        };
        Ok(AllowEntry {
            id,
            kind,
            family: self.family,
            path: self.path,
            glob: self.glob,
            owner: self.owner.unwrap_or_default(),
            classification: self.classification.unwrap_or_default(),
            reason: self.reason.unwrap_or_default(),
            evidence: self.evidence,
            links: self.links,
            lifecycle: Lifecycle {
                created: self.created,
                review_after: self.review_after,
                expires: self.expires,
            },
            selector: Selector {
                ast_kind: self.selector.ast_kind,
                container: self.selector.container,
                callee: self.selector.callee,
                macro_name: self.selector.macro_name,
                lint: self.selector.lint,
                symbol: self.selector.symbol,
                receiver_fingerprint: self.selector.receiver_fingerprint,
                target_fingerprint: self.selector.target_fingerprint,
                normalized_snippet_hash: self.selector.normalized_snippet_hash,
                line_hint: self.selector.line_hint,
                glob: self.selector.glob,
            },
            last_seen,
        })
    }
}

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    if cfg.policy != "cargo-allow" {
        return Err(CargoAllowError::new(format!(
            "unsupported policy `{}`",
            cfg.policy
        )));
    }
    let mut ids = BTreeSet::new();
    for entry in &cfg.allow {
        if entry.id.trim().is_empty() {
            return Err(CargoAllowError::new("allow entry has empty id"));
        }
        if !ids.insert(entry.id.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate allow id `{}`",
                entry.id
            )));
        }
        if entry.path.is_none() && entry.glob.is_none() && entry.selector.glob.is_none() {
            return Err(CargoAllowError::new(format!(
                "{} has no path or glob",
                entry.id
            )));
        }
        if cfg.requirements.owner_required && entry.owner.trim().is_empty() {
            return Err(CargoAllowError::new(format!("{} missing owner", entry.id)));
        }
        if cfg.requirements.reason_required && entry.reason.trim().is_empty() {
            return Err(CargoAllowError::new(format!("{} missing reason", entry.id)));
        }
        if cfg.requirements.classification_required && entry.classification.trim().is_empty() {
            return Err(CargoAllowError::new(format!(
                "{} missing classification",
                entry.id
            )));
        }
        if cfg.requirements.expires_or_review_after_required
            && entry.lifecycle.expires.is_none()
            && entry.lifecycle.review_after.is_none()
        {
            return Err(CargoAllowError::new(format!(
                "{} missing expires or review_after",
                entry.id
            )));
        }
        if cfg.requirements.unsafe_evidence_required
            && entry.kind == FindingKind::Unsafe
            && entry.evidence.is_empty()
        {
            return Err(CargoAllowError::new(format!(
                "{} unsafe entry missing evidence",
                entry.id
            )));
        }
    }
    Ok(())
}

pub fn starter_policy(strict: bool) -> String {
    let stale = if strict { "true" } else { "false" };
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "{}"
ignored = [".git/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
expires_or_review_after_required = true
allow_bare_allow_attributes = false
stale_entries_fail = {stale}

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
        if strict { "strict" } else { "no-new" }
    )
}

pub fn render_policy(cfg: &AllowConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "schema_version = \"{}\"\npolicy = \"{}\"\n",
        cfg.schema_version, cfg.policy
    ));
    if let Some(owner) = &cfg.owner {
        out.push_str(&format!("owner = \"{}\"\n", escape_toml(owner)));
    }
    if let Some(status) = &cfg.status {
        out.push_str(&format!("status = \"{}\"\n", escape_toml(status)));
    }
    out.push('\n');
    out.push_str("[workspace]\n");
    out.push_str(&format!(
        "root = \"{}\"\ninventory = \"{}\"\ndefault_mode = \"{}\"\n",
        escape_toml(&cfg.workspace.root),
        escape_toml(&cfg.workspace.inventory),
        escape_toml(&cfg.workspace.default_mode)
    ));
    out.push_str(&format!(
        "ignored = [{}]\n",
        render_array(&cfg.workspace.ignored)
    ));
    out.push_str(&format!(
        "generated = [{}]\n\n",
        render_array(&cfg.workspace.generated)
    ));
    out.push_str("[requirements]\n");
    out.push_str(&format!("owner_required = {}\nreason_required = {}\nclassification_required = {}\nexpires_or_review_after_required = {}\nallow_bare_allow_attributes = {}\nstale_entries_fail = {}\n\n", cfg.requirements.owner_required, cfg.requirements.reason_required, cfg.requirements.classification_required, cfg.requirements.expires_or_review_after_required, cfg.requirements.allow_bare_allow_attributes, cfg.requirements.stale_entries_fail));
    out.push_str("[requirements.unsafe]\n");
    out.push_str(&format!(
        "evidence_required = {}\nsafety_comment_required = {}\n",
        cfg.requirements.unsafe_evidence_required, cfg.requirements.unsafe_safety_comment_required
    ));
    for entry in &cfg.allow {
        out.push_str("\n[[allow]]\n");
        out.push_str(&format!(
            "id = \"{}\"\nkind = \"{}\"\n",
            escape_toml(&entry.id),
            entry.kind.as_str()
        ));
        if let Some(family) = &entry.family {
            out.push_str(&format!("family = \"{}\"\n", escape_toml(family)));
        }
        if let Some(path) = &entry.path {
            out.push_str(&format!(
                "path = \"{}\"\n",
                escape_toml(&path.to_string_lossy())
            ));
        }
        if let Some(glob) = &entry.glob {
            out.push_str(&format!("glob = \"{}\"\n", escape_toml(glob)));
        }
        out.push_str(&format!(
            "owner = \"{}\"\nclassification = \"{}\"\nreason = \"{}\"\n",
            escape_toml(&entry.owner),
            escape_toml(&entry.classification),
            escape_toml(&entry.reason)
        ));
        if !entry.evidence.is_empty() {
            out.push_str(&format!("evidence = [{}]\n", render_array(&entry.evidence)));
        }
        if !entry.links.is_empty() {
            out.push_str(&format!("links = [{}]\n", render_array(&entry.links)));
        }
        if let Some(created) = &entry.lifecycle.created {
            out.push_str(&format!("created = \"{}\"\n", escape_toml(created)));
        }
        if let Some(review_after) = &entry.lifecycle.review_after {
            out.push_str(&format!(
                "review_after = \"{}\"\n",
                escape_toml(review_after)
            ));
        }
        if let Some(expires) = &entry.lifecycle.expires {
            out.push_str(&format!("expires = \"{}\"\n", escape_toml(expires)));
        }
        out.push_str("\n[allow.selector]\n");
        if let Some(v) = &entry.selector.ast_kind {
            out.push_str(&format!("ast_kind = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.container {
            out.push_str(&format!("container = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.callee {
            out.push_str(&format!("callee = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.macro_name {
            out.push_str(&format!("macro_name = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.lint {
            out.push_str(&format!("lint = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.symbol {
            out.push_str(&format!("symbol = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.receiver_fingerprint {
            out.push_str(&format!("receiver_fingerprint = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.target_fingerprint {
            out.push_str(&format!("target_fingerprint = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.normalized_snippet_hash {
            out.push_str(&format!(
                "normalized_snippet_hash = \"{}\"\n",
                escape_toml(v)
            ));
        }
        if let Some(v) = entry.selector.line_hint {
            out.push_str(&format!("line_hint = {}\n", v));
        }
        if let Some(v) = &entry.selector.glob {
            out.push_str(&format!("glob = \"{}\"\n", escape_toml(v)));
        }
        if let Some(last) = &entry.last_seen {
            out.push_str("\n[allow.last_seen]\n");
            out.push_str(&format!("line = {}\ncolumn = {}\n", last.line, last.column));
        }
    }
    out
}

fn escape_toml(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match StringList::deserialize(deserializer)? {
        StringList::One(value) => Ok(vec![value]),
        StringList::Many(values) => Ok(values),
    }
}

fn option_bool_or_string<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<Boolish>::deserialize(deserializer)? {
        Some(Boolish::Bool(value)) => Ok(Some(value)),
        Some(Boolish::String(value)) => value
            .parse::<bool>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

fn option_u32_or_string<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<U32ish>::deserialize(deserializer)? {
        Some(U32ish::Number(value)) => Ok(Some(value)),
        Some(U32ish::String(value)) => value
            .parse::<u32>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

fn render_array(values: &[String]) -> String {
    values
        .iter()
        .map(|v| format!("\"{}\"", escape_toml(v)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_policy_with_allow() {
        let cfg = parse_policy(
            r#"
                schema_version = "0.1"
                policy = "cargo-allow"

                [requirements]
                expires_or_review_after_required = true

                [[allow]]
                id = "allow-0001"
                kind = "panic"
                family = "unwrap"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"

                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                container = "load"
            "#,
        )
        .expect("policy parses");
        assert_eq!(cfg.allow.len(), 1);
        assert_eq!(cfg.allow[0].selector.callee.as_deref(), Some("unwrap"));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let err = parse_policy(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "x"
                kind = "non_rust_file"
                path = "a.py"
                owner = "o"
                classification = "c"
                reason = "r"
                expires = "2026-08-01"
                [allow.selector]
                glob = "a.py"
                [[allow]]
                id = "x"
                kind = "non_rust_file"
                path = "b.py"
                owner = "o"
                classification = "c"
                reason = "r"
                expires = "2026-08-01"
                [allow.selector]
                glob = "b.py"
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn parses_legacy_aliases_and_scalar_arrays() {
        let cfg = parse_policy(
            r#"
                policy = "cargo-allow"

                [workspace]
                ignored = ".git/**"

                [requirements]
                owner_required = "true"

                [[allow]]
                id = "allow-legacy"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "legacy"
                explanation = "legacy reason field"
                covered_by = "test:legacy"
                expires = "2026-08-01"

                [allow.selector]
                kind = "macro_call"
                macro = "panic"
                line_hint = "12"
            "#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("legacy aliases parse: {err}")));

        assert_eq!(cfg.workspace.ignored, vec![".git/**"]);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one allow entry"));
        assert_eq!(entry.reason, "legacy reason field");
        assert_eq!(entry.evidence, vec!["test:legacy"]);
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("macro_call"));
        assert_eq!(entry.selector.macro_name.as_deref(), Some("panic"));
        assert_eq!(entry.selector.line_hint, Some(12));
    }

    #[test]
    fn reports_toml_parse_errors() {
        let err = parse_policy("policy = [").unwrap_err();

        assert!(err.to_string().contains("failed to parse policy TOML"));
    }

    #[test]
    fn parses_current_repository_policy() {
        let cfg = parse_policy(include_str!("../../../policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("repo policy parses: {err}")));

        assert_eq!(cfg.policy, "cargo-allow");
        assert!(cfg.allow.len() >= 70);
    }
}
