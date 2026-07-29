use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchStatus, Selector,
    Span, StructuralIdentity,
};
use std::path::PathBuf;

mod evaluation;
mod invalid_date_contract;
mod lint;
mod mode;
mod parity;
mod property;
mod scoring;
mod selector_precision;

fn entry_with_hash(hash: &str) -> AllowEntry {
    AllowEntry {
        id: "allow-1".to_string(),
        kind: FindingKind::Unsafe,
        family: Some("unsafe_fn".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "test".to_string(),
        reason: "reason".to_string(),
        evidence: vec!["unsafe-review".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-12-31".to_string()),
        },
        selector: Selector {
            ast_kind: Some("unsafe_fn".to_string()),
            container: Some("scan_line".to_string()),
            normalized_snippet_hash: Some(hash.to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn finding_with_hash(hash: &str) -> Finding {
    let mut id = StructuralIdentity::new("rust", "unsafe_fn");
    id.container = Some("scan_line".to_string());
    id.normalized_snippet_hash = Some(hash.to_string());
    Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_fn".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 50,
            column: 12,
        }),
        identity: id,
        message: String::new(),
        ledger: None,
    }
}

fn lint_entry(id: &str) -> AllowEntry {
    lint_entry_with_family(id, "expect_attribute")
}

fn lint_entry_with_family(id: &str, family: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::LintException,
        family: Some(family.to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Lint suppression is linked to policy.".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-12-31".to_string()),
        },
        selector: Selector {
            ast_kind: Some("attribute".to_string()),
            lint: Some("clippy::unwrap_used".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn lint_finding_with_policy(policy_id: &str) -> Finding {
    let mut finding = lint_finding("expect_attribute");
    finding.identity.target_fingerprint = Some(format!("policy:{policy_id}"));
    finding
}

fn lint_finding(family: &str) -> Finding {
    let mut id = StructuralIdentity::new("rust", "attribute");
    id.lint = Some("clippy::unwrap_used".to_string());
    Finding {
        kind: FindingKind::LintException,
        family: Some(family.to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 1,
        }),
        identity: id,
        message: String::new(),
        ledger: None,
    }
}
