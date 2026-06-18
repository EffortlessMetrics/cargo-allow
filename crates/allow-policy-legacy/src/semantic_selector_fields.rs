use allow_core::Selector;

use crate::fields::string_field;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LegacySemanticSelectorExtras {
    pub receiver_fingerprint: Option<String>,
    pub target_fingerprint: Option<String>,
    pub symbol: Option<String>,
    pub normalized_snippet_hash: Option<String>,
}

impl LegacySemanticSelectorExtras {
    pub(crate) fn from_selector_table(selector: Option<&toml::Table>) -> Self {
        let Some(selector) = selector else {
            return Self::default();
        };
        Self {
            receiver_fingerprint: legacy_receiver_fingerprint(selector),
            target_fingerprint: legacy_target_fingerprint(selector),
            symbol: string_field(selector, "symbol"),
            normalized_snippet_hash: string_field(selector, "normalized_snippet_hash"),
        }
    }

    pub(crate) fn apply_to_selector(&self, selector: &mut Selector) {
        if self.receiver_fingerprint.is_some() {
            selector.receiver_fingerprint = self.receiver_fingerprint.clone();
        }
        if self.target_fingerprint.is_some() {
            selector.target_fingerprint = self.target_fingerprint.clone();
        }
        if self.symbol.is_some() {
            selector.symbol = self.symbol.clone();
        }
        if self.normalized_snippet_hash.is_some() {
            selector.normalized_snippet_hash = self.normalized_snippet_hash.clone();
        }
    }
}

pub(crate) fn legacy_receiver_fingerprint(table: &toml::Table) -> Option<String> {
    string_field(table, "receiver_fingerprint").or_else(|| string_field(table, "receiver"))
}

pub(crate) fn legacy_target_fingerprint(table: &toml::Table) -> Option<String> {
    string_field(table, "target_fingerprint").or_else(|| string_field(table, "target"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn semantic_selector_extras_prefer_canonical_names_over_legacy_aliases() {
        let table = parse_table(
            r#"
receiver = "legacy-receiver"
receiver_fingerprint = "canonical-receiver"
target = "legacy-target"
target_fingerprint = "canonical-target"
symbol = "load"
normalized_snippet_hash = "abc123"
"#,
        );

        let extras = LegacySemanticSelectorExtras::from_selector_table(Some(&table));

        assert_eq!(
            extras.receiver_fingerprint.as_deref(),
            Some("canonical-receiver")
        );
        assert_eq!(
            extras.target_fingerprint.as_deref(),
            Some("canonical-target")
        );
        assert_eq!(extras.symbol.as_deref(), Some("load"));
        assert_eq!(extras.normalized_snippet_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn semantic_selector_extras_accept_legacy_receiver_and_target_aliases() {
        let table = parse_table(
            r#"
receiver = "optional_value"
target = "policy:fixture-clippy"
"#,
        );

        let extras = LegacySemanticSelectorExtras::from_selector_table(Some(&table));

        assert_eq!(
            extras.receiver_fingerprint.as_deref(),
            Some("optional_value")
        );
        assert_eq!(
            extras.target_fingerprint.as_deref(),
            Some("policy:fixture-clippy")
        );
    }

    #[test]
    fn apply_to_selector_sets_only_present_semantic_fields() {
        let extras = LegacySemanticSelectorExtras {
            receiver_fingerprint: Some("param:0".to_string()),
            ..LegacySemanticSelectorExtras::default()
        };
        let mut selector = Selector {
            container: Some("load".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        };

        extras.apply_to_selector(&mut selector);

        assert_eq!(selector.receiver_fingerprint.as_deref(), Some("param:0"));
        assert_eq!(selector.container.as_deref(), Some("load"));
        assert_eq!(selector.target_fingerprint, None);
    }
}
