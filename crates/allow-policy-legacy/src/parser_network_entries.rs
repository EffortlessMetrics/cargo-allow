use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, required_bool_field, required_string_field, string_field};
use crate::parser_support::canonicalize_legacy_date;
use crate::parser_support::normalize_legacy_date_field;
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyNetworkRule;

pub(crate) fn parse_network_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyNetworkRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("network-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_network_rule(index, entry))
        .collect()
}

fn parse_network_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyNetworkRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("network allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("network allow entry {index}"))?;
    Ok(LegacyNetworkRule {
        destination: required_string_field(table, "destination", &id)?,
        auth_required: required_bool_field(table, "auth_required", &id)?,
        auth_secret: string_field(table, "auth_secret"),
        lane: required_string_field(table, "lane", &id)?,
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        evidence: legacy_evidence(table),
        created: Some(canonicalize_legacy_date(&required_string_field(
            table, "created", &id,
        )?)),
        review_after: normalize_legacy_date_field(string_field(table, "review_after")),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn parse_network_rules_preserves_public_and_authenticated_fields() {
        let table = parse_table(
            r#"
[[allow]]
id = "net-crates-io"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "ci"
reason = "Build lane fetches public crates."
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[[allow]]
id = "net-github-api"
destination = "api.github.com"
auth_required = true
auth_secret = "GITHUB_TOKEN"
lane = "release"
owner = "release"
reason = "Release lane publishes GitHub releases."
covered_by = "test:github_release_publish_is_pinned"
created = "2026-05-10"
"#,
        );

        let mut rules = parse_network_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("network allow entries parse: {err}"))
        });

        assert_eq!(rules.len(), 2);
        let public = rules.remove(0);
        assert_eq!(public.id, "net-crates-io");
        assert_eq!(public.destination, "crates.io");
        assert!(!public.auth_required);
        assert_eq!(public.auth_secret, None);
        assert_eq!(public.lane, "build");
        assert_eq!(public.owner, "ci");
        assert_eq!(public.reason, "Build lane fetches public crates.");
        assert_eq!(public.evidence, vec!["doc:docs/ci.md".to_string()]);
        assert_eq!(public.created.as_deref(), Some("2026-05-09"));
        assert_eq!(public.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(public.expires.as_deref(), Some("never"));

        let authenticated = rules.remove(0);
        assert_eq!(authenticated.id, "net-github-api");
        assert_eq!(authenticated.destination, "api.github.com");
        assert!(authenticated.auth_required);
        assert_eq!(authenticated.auth_secret.as_deref(), Some("GITHUB_TOKEN"));
        assert_eq!(authenticated.lane, "release");
        assert_eq!(authenticated.owner, "release");
        assert_eq!(
            authenticated.reason,
            "Release lane publishes GitHub releases."
        );
        assert_eq!(
            authenticated.evidence,
            vec!["test:github_release_publish_is_pinned".to_string()]
        );
        assert_eq!(authenticated.created.as_deref(), Some("2026-05-10"));
        assert!(authenticated.review_after.is_none());
        assert!(authenticated.expires.is_none());
    }

    #[test]
    fn parse_network_rules_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"network-allowlist\"");
        let err = parse_network_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("network-allowlist missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_network_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("network allow entry 0 is not a table")
        );

        let missing_id = parse_table(
            r#"
[[allow]]
destination = "crates.io"
auth_required = false
lane = "build"
owner = "ci"
reason = "Build lane fetches public crates."
created = "2026-05-09"
"#,
        );
        let err = parse_network_rules(&missing_id)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("id is required"));
        assert!(err.to_string().contains("network allow entry 0 missing id"));

        let missing_auth_required = parse_table(
            r#"
[[allow]]
id = "net-missing-auth"
destination = "crates.io"
lane = "build"
owner = "ci"
reason = "Build lane fetches public crates."
created = "2026-05-09"
"#,
        );
        let err = parse_network_rules(&missing_auth_required)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("auth_required is required"));
        assert!(
            err.to_string()
                .contains("net-missing-auth missing auth_required")
        );

        let missing_lane = parse_table(
            r#"
[[allow]]
id = "net-missing-lane"
destination = "crates.io"
auth_required = false
owner = "ci"
reason = "Build lane fetches public crates."
created = "2026-05-09"
"#,
        );
        let err = parse_network_rules(&missing_lane)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("lane is required"));
        assert!(err.to_string().contains("net-missing-lane missing lane"));
    }
}
