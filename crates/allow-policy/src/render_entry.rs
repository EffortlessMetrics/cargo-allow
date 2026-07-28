use allow_core::AllowEntry;

use crate::render_last_seen::render_last_seen;
use crate::render_selector::render_selector;
use crate::render_toml::{
    escape_toml, render_array, render_optional_string_field, render_string_field,
};

pub(crate) fn render_allow_entry(out: &mut String, entry: &AllowEntry) {
    out.push_str("\n[[allow]]\n");
    out.push_str(&format!(
        "id = \"{}\"\nkind = \"{}\"\n",
        escape_toml(&entry.id),
        entry.kind.as_str()
    ));
    render_optional_string_field(out, "family", entry.family.as_deref());
    // Render path OR glob, never both — validate_scope_consistency rejects
    // entries with both path and glob, so rendering both would produce TOML
    // that fails to round-trip through parse_policy (#1836). When both are
    // set, prefer glob (the more general scope).
    if entry.glob.is_some() {
        render_optional_string_field(out, "glob", entry.glob.as_deref());
    } else if let Some(path) = &entry.path {
        render_string_field(out, "path", path.to_string_lossy().as_ref());
    }
    render_string_field(out, "owner", &entry.owner);
    render_string_field(out, "classification", &entry.classification);
    render_string_field(out, "reason", &entry.reason);
    if !entry.evidence.is_empty() {
        out.push_str(&format!("evidence = [{}]\n", render_array(&entry.evidence)));
    }
    if !entry.links.is_empty() {
        out.push_str(&format!("links = [{}]\n", render_array(&entry.links)));
    }
    if let Some(limit) = entry.occurrence_limit {
        out.push_str(&format!("occurrence_limit = {limit}\n"));
    }
    if let Some(created) = &entry.lifecycle.created {
        render_string_field(out, "created", created);
    }
    if let Some(review_after) = &entry.lifecycle.review_after {
        render_string_field(out, "review_after", review_after);
    }
    if let Some(expires) = &entry.lifecycle.expires {
        render_string_field(out, "expires", expires);
    }
    render_selector(out, &entry.selector);
    if let Some(last_seen) = &entry.last_seen {
        render_last_seen(out, last_seen);
    }
}

#[cfg(test)]
mod tests {
    use super::render_allow_entry;
    use allow_core::{AllowEntry, FindingKind, LastSeen, Lifecycle, Selector};
    use std::path::PathBuf;

    #[test]
    fn render_allow_entry_writes_all_entry_fields() {
        let entry = AllowEntry {
            id: "allow-rendered-entry".to_string(),
            kind: FindingKind::PolicyException,
            family: Some("process_spawn".to_string()),
            path: Some(PathBuf::from(".github/workflows/ci.yml")),
            glob: Some(".github/workflows/*.yml".to_string()),
            owner: "repo-infra".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Workflow needs a retained process exception.".to_string(),
            evidence: vec![
                "spec:CARGO-ALLOW-SPEC-0001".to_string(),
                "test:workflow-review".to_string(),
            ],
            links: vec!["doc:docs/ci.md".to_string(), "adr:ADR-0001".to_string()],
            occurrence_limit: Some(2),
            lifecycle: Lifecycle {
                created: Some("2026-06-01".to_string()),
                review_after: Some("2026-07-01".to_string()),
                expires: Some("2026-09-01".to_string()),
            },
            selector: Selector {
                ast_kind: Some("workflow_step".to_string()),
                container: Some("ci".to_string()),
                callee: Some("shell".to_string()),
                lint: Some("policy_exception::process_spawn".to_string()),
                symbol: Some("run: cargo test".to_string()),
                line_hint: Some(24),
                glob: Some(".github/workflows/ci.yml".to_string()),
                ..Selector::default()
            },
            last_seen: Some(LastSeen {
                line: 24,
                column: 9,
            }),
        };
        let mut out = String::new();

        render_allow_entry(&mut out, &entry);

        for expected in [
            "[[allow]]",
            "id = \"allow-rendered-entry\"",
            "kind = \"policy_exception\"",
            "family = \"process_spawn\"",
            // When both path and glob are set, only glob is rendered (#1836)
            "glob = \".github/workflows/*.yml\"",
            "owner = \"repo-infra\"",
            "classification = \"reviewed_exception\"",
            "reason = \"Workflow needs a retained process exception.\"",
            "evidence = [\"spec:CARGO-ALLOW-SPEC-0001\", \"test:workflow-review\"]",
            "links = [\"doc:docs/ci.md\", \"adr:ADR-0001\"]",
            "occurrence_limit = 2",
            "created = \"2026-06-01\"",
            "review_after = \"2026-07-01\"",
            "expires = \"2026-09-01\"",
            "[allow.selector]",
            "ast_kind = \"workflow_step\"",
            "container = \"ci\"",
            "callee = \"shell\"",
            "lint = \"policy_exception::process_spawn\"",
            "symbol = \"run: cargo test\"",
            "[allow.last_seen]",
            "line = 24",
            "column = 9",
        ] {
            assert!(
                out.contains(expected),
                "rendered entry should contain `{expected}`:\n{out}"
            );
        }
        // path must NOT appear when glob is set (#1836 round-trip fix)
        assert!(
            !out.contains("path = "),
            "path should not render when glob is set:\n{out}"
        );
    }

    #[test]
    fn render_allow_entry_omits_absent_optional_fields() {
        let entry = AllowEntry {
            id: "allow-minimal-entry".to_string(),
            kind: FindingKind::NonRustFile,
            family: None,
            path: None,
            glob: Some("docs/*.md".to_string()),
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "Tracked documentation is part of source-tree inventory.".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: None,
                review_after: None,
                expires: None,
            },
            selector: Selector::default(),
            last_seen: None,
        };
        let mut out = String::new();

        render_allow_entry(&mut out, &entry);

        for expected in [
            "id = \"allow-minimal-entry\"",
            "kind = \"non_rust_file\"",
            "glob = \"docs/*.md\"",
            "owner = \"docs\"",
            "classification = \"documentation\"",
            "reason = \"Tracked documentation is part of source-tree inventory.\"",
        ] {
            assert!(
                out.contains(expected),
                "rendered entry should contain `{expected}`:\n{out}"
            );
        }
        for omitted in [
            "family =",
            "path =",
            "evidence =",
            "links =",
            "occurrence_limit =",
            "created =",
            "review_after =",
            "expires =",
            "[allow.last_seen]",
        ] {
            assert!(
                !out.contains(omitted),
                "rendered entry should omit `{omitted}`:\n{out}"
            );
        }
        assert!(out.contains("[allow.selector]"));
        for omitted_selector_field in [
            "ast_kind =",
            "container =",
            "callee =",
            "macro_name =",
            "lint =",
            "symbol =",
            "receiver_fingerprint =",
            "target_fingerprint =",
            "normalized_snippet_hash =",
            "line_hint =",
        ] {
            assert!(
                !out.contains(omitted_selector_field),
                "default selector should omit `{omitted_selector_field}`:\n{out}"
            );
        }
    }
}
