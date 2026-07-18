use allow_core::Selector;
use serde::Deserialize;

use crate::toml_de::option_u32_or_string;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectorToml {
    ast_kind: Option<String>,
    container: Option<String>,
    callee: Option<String>,
    macro_name: Option<String>,
    lint: Option<String>,
    symbol: Option<String>,
    receiver_fingerprint: Option<String>,
    target_fingerprint: Option<String>,
    normalized_snippet_hash: Option<String>,
    // line_hint is accepted in TOML for backward compatibility but no longer
    // propagated into the runtime Selector. The leading underscore suppresses
    // the dead-code warning without introducing a lint_exception finding.
    #[serde(
        default,
        rename = "line_hint",
        deserialize_with = "option_u32_or_string"
    )]
    _line_hint: Option<u32>,
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
            // line_hint is accepted in TOML for backward compatibility but no
            // longer propagated into the runtime Selector. It was never read by
            // the matching engine (scoring.rs); numeric line-distance scoring
            // was retired in favor of explicit match-strength tiers. Keeping it
            // None here makes it inert everywhere downstream (fingerprint,
            // render, validation) without breaking existing policy TOML.
            line_hint: None,
            glob: self.glob,
        }
    }
}
