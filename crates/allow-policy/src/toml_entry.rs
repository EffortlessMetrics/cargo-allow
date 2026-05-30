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
