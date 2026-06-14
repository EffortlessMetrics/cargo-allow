use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, FindingKind};
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;

use crate::toml_de::string_or_vec;
use crate::toml_last_seen::LastSeenToml;
use crate::toml_lifecycle::LifecycleToml;
use crate::toml_selector::SelectorToml;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct AllowEntryToml {
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
    #[serde(alias = "count")]
    occurrence_limit: Option<u32>,
    #[serde(flatten)]
    lifecycle: LifecycleToml,
    #[serde(default)]
    selector: SelectorToml,
    #[serde(default)]
    last_seen: LastSeenToml,
}

impl AllowEntryToml {
    pub(crate) fn into_allow_entry(self, index: usize) -> CargoAllowResult<AllowEntry> {
        let id = self.id.unwrap_or_else(|| format!("allow-{:04}", index + 1));
        let kind_text = self
            .kind
            .ok_or_else(|| CargoAllowError::new(format!("{id} missing kind")))?;
        let kind = FindingKind::from_str(&kind_text)?;
        let last_seen = self.last_seen.into_last_seen(&id)?;
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
            occurrence_limit: self.occurrence_limit,
            lifecycle: self.lifecycle.into_lifecycle(),
            selector: self.selector.into_selector(),
            last_seen,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_allow_entry_call_presence_observer() {
        let raw = toml::from_str::<AllowEntryToml>(
            r#"
kind = "panic"
family = "panic_macro"
path = "src/lib.rs"
glob = "src/**/*.rs"
evidence = ["test:entry_conversion"]
links = ["doc:docs/policy.md"]
count = 3
created = "2026-01-01"
review_after = "2026-07-01"
expires = "2027-01-01"

[selector]
kind = "macro_call"
macro = "panic"
line_hint = 41

[last_seen]
line = 42
column = 9
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("TOML entry parses: {err}")));

        let entry = raw
            .into_allow_entry(0)
            .unwrap_or_else(|err| std::panic::panic_any(format!("TOML entry converts: {err}")));

        assert_eq!(entry.id, "allow-0001");
        assert_eq!(entry.kind.as_str(), "panic");
        assert_eq!(entry.family.as_deref(), Some("panic_macro"));
        assert_eq!(
            entry.path.as_deref(),
            Some(std::path::Path::new("src/lib.rs"))
        );
        assert_eq!(entry.glob.as_deref(), Some("src/**/*.rs"));
        assert_eq!(entry.owner, "");
        assert_eq!(entry.classification, "");
        assert_eq!(entry.reason, "");
        assert_eq!(entry.evidence, vec!["test:entry_conversion"]);
        assert_eq!(entry.links, vec!["doc:docs/policy.md"]);
        assert_eq!(entry.occurrence_limit, Some(3));
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-01-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-07-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-01-01"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("macro_call"));
        assert_eq!(entry.selector.macro_name.as_deref(), Some("panic"));
        assert_eq!(entry.selector.line_hint, Some(41));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((42, 9))
        );
    }

    #[test]
    fn into_allow_entry_field_discriminator() {
        let explicit = toml::from_str::<AllowEntryToml>(
            r#"
id = "allow-explicit"
kind = "unsafe"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("explicit entry parses: {err}")))
        .into_allow_entry(1)
        .unwrap_or_else(|err| std::panic::panic_any(format!("explicit entry converts: {err}")));

        assert_eq!(explicit.id, "allow-explicit");
        assert_eq!(explicit.kind.as_str(), "unsafe");

        let generated = toml::from_str::<AllowEntryToml>(
            r#"
kind = "generated_code"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated entry parses: {err}")))
        .into_allow_entry(1)
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated entry converts: {err}")));

        assert_eq!(generated.id, "allow-0002");
        assert_eq!(generated.kind.as_str(), "generated_code");

        let err = toml::from_str::<AllowEntryToml>("owner = \"policy\"\n")
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("missing-kind entry parses: {err}"))
            })
            .into_allow_entry(1)
            .expect_err("missing kind should fail entry conversion");

        assert!(err.to_string().contains("allow-0002 missing kind"));
    }
}
