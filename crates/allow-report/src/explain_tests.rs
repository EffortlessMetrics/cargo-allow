use super::*;
use allow_core::{
    AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchOutcome, MatchStatus, Selector,
    Span, StructuralIdentity,
};
use std::path::PathBuf;

#[test]
fn explain_json_renderer_records_context_and_current_status() {
    let entry = AllowEntry {
        id: "allow-explain-json".to_string(),
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: Some(PathBuf::from("src\\ffi.rs")),
        glob: None,
        owner: "runtime".to_string(),
        classification: "ffi_boundary".to_string(),
        reason: "FFI pointer boundary requires unsafe.".to_string(),
        evidence: vec!["doc:docs/safety/ffi.md".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-27".to_string()),
            review_after: Some("2026-11-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("unsafe_block".to_string()),
            container: Some("read_byte".to_string()),
            callee: None,
            macro_name: None,
            lint: None,
            symbol: None,
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: Some("fnv1a64:unsafe".to_string()),
            line_hint: Some(9),
            glob: None,
        },
        last_seen: Some(LastSeen { line: 9, column: 5 }),
    };
    let mut identity = StructuralIdentity::new("rust", "unsafe_block");
    identity.crate_name = Some("runtime".to_string());
    identity.container = Some("read_byte".to_string());
    let finding = Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: PathBuf::from("src\\ffi.rs"),
        span: Some(Span { line: 9, column: 5 }),
        identity,
        message: "unsafe block".to_string(),
    };
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::EvidenceMissing,
        allow_id: Some("allow-explain-json".to_string()),
        finding_index: Some(0),
        message: "unsafe entry has missing evidence".to_string(),
        score: 9,
    }];
    let evidence_references = vec![EvidenceReference {
        raw: "doc:docs/safety/ffi.md",
        prefix: Some("doc"),
        target: Some("docs/safety/ffi.md"),
        status: "missing",
        message: "local evidence file is missing",
    }];
    let suggested_actions = vec!["add missing evidence".to_string()];
    let proof_commands = vec!["cargo-allow check --kind unsafe".to_string()];

    let report = ExplainReport {
        inventory: InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        entry: &entry,
        current_findings: &[finding],
        match_outcomes: &outcomes,
        evidence_references: &evidence_references,
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
    };

    let json = render_explain_json(report);

    assert!(json.contains("\"schema_id\": \"cargo-allow.explain.v1\""));
    assert!(json.contains("\"command\": \"explain\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"id\": \"allow-explain-json\""));
    assert!(json.contains("\"current_status\": \"evidence_missing\""));
    assert!(json.contains("\"current_matches\": 1"));
    assert!(json.contains("\"match_outcomes\": 1"));
    assert!(json.contains("\"raw\": \"doc:docs/safety/ffi.md\""));
    assert!(json.contains("\"target\": \"docs/safety/ffi.md\""));
    assert!(json.contains("\"status\": \"missing\""));
    assert!(json.contains("\"path\": \"src/ffi.rs\""));
    assert!(json.contains("\"source_package\": \"runtime\""));
    assert!(json.contains("\"score\": 9"));
    assert!(json.contains("\"add missing evidence\""));
    assert!(json.contains("\"cargo-allow check --kind unsafe\""));

    let text = render_explain_human(report);

    assert!(text.contains("allow-explain-json"));
    assert!(text.contains("kind: unsafe.unsafe_block"));
    assert!(text.contains("scope: src/ffi.rs"));
    assert!(text.contains("owner: runtime"));
    assert!(text.contains("classification: ffi_boundary"));
    assert!(text.contains("evidence references:"));
    assert!(text.contains(
            "- doc:docs/safety/ffi.md prefix=doc target=docs/safety/ffi.md status=missing message=local evidence file is missing"
        ));
    assert!(text.contains("current_status: evidence_missing"));
    assert!(text.contains("current_matches: 1"));
    assert!(
        text.contains("- evidence_missing: src/ffi.rs:9:5 (unsafe_block, source_package=runtime)")
    );
    assert!(text.contains("- evidence_missing: unsafe entry has missing evidence"));
    assert!(text.contains("- action: add missing evidence"));
    assert!(text.contains("- proof: cargo-allow check --kind unsafe"));
}
