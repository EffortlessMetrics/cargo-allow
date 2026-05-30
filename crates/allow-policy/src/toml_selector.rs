use allow_core::Selector;
use serde::Deserialize;

use crate::toml_de::option_u32_or_string;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct SelectorToml {
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

impl SelectorToml {
    pub(crate) fn into_selector(self) -> Selector {
        Selector {
            ast_kind: self.ast_kind,
            container: self.container,
            callee: self.callee,
            macro_name: self.macro_name,
            lint: self.lint,
            symbol: self.symbol,
            receiver_fingerprint: self.receiver_fingerprint,
            target_fingerprint: self.target_fingerprint,
            normalized_snippet_hash: self.normalized_snippet_hash,
            line_hint: self.line_hint,
            glob: self.glob,
        }
    }
}
