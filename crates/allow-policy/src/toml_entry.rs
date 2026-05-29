use allow_core::{
    AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, LastSeen, Lifecycle, Selector,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;

use crate::toml_de::{option_u32_or_string, string_or_vec};

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

impl AllowEntryToml {
    pub(crate) fn into_allow_entry(self, index: usize) -> CargoAllowResult<AllowEntry> {
        let id = self.id.unwrap_or_else(|| format!("allow-{:04}", index + 1));
        let kind_text = self
            .kind
            .ok_or_else(|| CargoAllowError::new(format!("{id} missing kind")))?;
        let kind = FindingKind::from_str(&kind_text)?;
        let last_seen = match (self.last_seen.line, self.last_seen.column) {
            (Some(line), Some(column)) => Some(LastSeen { line, column }),
            (None, None) => None,
            _ => {
                return Err(CargoAllowError::new(format!(
                    "{id} last_seen must include both line and column"
                )));
            }
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
            occurrence_limit: self.occurrence_limit,
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
