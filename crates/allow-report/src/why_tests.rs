use super::*;
use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, Span, StructuralIdentity};
use std::path::PathBuf;

#[test]
fn render_why_json_emits_schema_id_and_candidates() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.callee = Some("unwrap".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 1,
        }),
        identity,
        message: "unwrap call".to_string(),
        ledger: None,
    };
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic.unwrap at src/lib.rs:10:1".to_string(),
        score: 0,
    };
    let reasons =
        vec!["callee mismatch: entry requires `expect`, finding has `unwrap`".to_string()];
    let candidates = [WhyCandidateEntry {
        id: "allow-near-miss",
        kind: "panic",
        family: Some("unwrap"),
        path: Some("src/lib.rs"),
        glob: None,
        selector_glob: None,
        mismatch_reasons: &reasons,
    }];
    let actions = ["Receipt this occurrence with cargo-allow add.".to_string()];
    let proofs = [
        "cargo-allow add --kind panic --path src/lib.rs --line 10 --owner <owner> --reason \"...\" --evidence <ref> --write policy/allow.toml"
            .to_string(),
    ];
    let report = WhyReport {
        inventory: InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(12)),
        finding: &finding,
        outcome: &outcome,
        candidate_entries: &candidates,
        suggested_actions: &actions,
        proof_commands: &proofs,
    };

    let json = render_why_json(report);
    assert!(json.contains("\"schema_id\": \"cargo-allow.why.v1\""));
    assert!(json.contains("\"command\": \"why\""));
    assert!(json.contains("\"id\": \"allow-near-miss\""));
    assert!(json.contains("callee mismatch"));
    assert!(json.contains("\"status\": \"new\""));
}
