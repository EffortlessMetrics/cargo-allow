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
    let add_args = [
        "add".to_string(),
        "--kind".to_string(),
        "panic".to_string(),
        "--path".to_string(),
        "src/lib.rs".to_string(),
        "--line".to_string(),
        "10".to_string(),
        "--owner".to_string(),
        "<owner>".to_string(),
        "--reason".to_string(),
        "...".to_string(),
        "--evidence".to_string(),
        "<ref>".to_string(),
        "--write".to_string(),
        "policy/allow.toml".to_string(),
    ];
    let plans = [WhyProofPlan {
        program: "cargo-allow",
        args: &add_args,
    }];
    let report = WhyReport {
        inventory: InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(12))
            .with_completeness("complete"),
        evaluation: EvaluationContext {
            scope: "scoped",
            locality: "proven",
            reasons: &[],
        },
        finding: &finding,
        outcome: &outcome,
        candidate_entries: &candidates,
        suggested_actions: &actions,
        proof_commands: &proofs,
        proof_plans: &plans,
    };

    let json = render_why_json(report);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|err| {
        std::panic::panic_any(format!("why JSON should deserialize: {err}\n{json}"))
    });
    assert_eq!(
        value
            .pointer("/schema_id")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow.why.v1")
    );
    assert_eq!(
        value
            .pointer("/command")
            .and_then(serde_json::Value::as_str),
        Some("why")
    );
    assert_eq!(
        value
            .pointer("/candidate_entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-near-miss")
    );
    assert!(
        value
            .pointer("/candidate_entries/0/mismatch_reasons/0")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|reason| reason.contains("callee mismatch"))
    );
    assert_eq!(
        value
            .pointer("/outcome/status")
            .and_then(serde_json::Value::as_str),
        Some("new")
    );
    assert_eq!(
        value
            .pointer("/evaluation/result_class")
            .and_then(serde_json::Value::as_str),
        Some("exact_scoped")
    );
    assert_eq!(
        value
            .pointer("/next/proof_plans/0/program")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow")
    );
    assert_eq!(
        value
            .pointer("/next/proof_plans/0/args/3")
            .and_then(serde_json::Value::as_str),
        Some("--path")
    );
    assert_eq!(
        value
            .pointer("/next/proof_plans/0/args/4")
            .and_then(serde_json::Value::as_str),
        Some("src/lib.rs")
    );
}

#[test]
fn render_why_json_omits_unavailable_candidate_family() -> Result<(), String> {
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 1,
        }),
        identity: StructuralIdentity::new("rust", "method_call"),
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
    let reasons = vec!["family unavailable".to_string()];
    let candidates = [WhyCandidateEntry {
        id: "allow-near-miss",
        kind: "panic",
        family: None,
        path: None,
        glob: None,
        selector_glob: None,
        mismatch_reasons: &reasons,
    }];
    let actions = ["Receipt this occurrence with cargo-allow add.".to_string()];
    let proofs = ["cargo-allow add --kind panic".to_string()];
    let add_args = ["add".to_string(), "--kind".to_string(), "panic".to_string()];
    let plans = [WhyProofPlan {
        program: "cargo-allow",
        args: &add_args,
    }];
    let json = render_why_json(WhyReport {
        inventory: InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(12))
            .with_completeness("complete"),
        evaluation: EvaluationContext {
            scope: "full_fallback",
            locality: "global_dependency",
            reasons: &reasons,
        },
        finding: &finding,
        outcome: &outcome,
        candidate_entries: &candidates,
        suggested_actions: &actions,
        proof_commands: &proofs,
        proof_plans: &plans,
    });
    let value: serde_json::Value = serde_json::from_str(&json)
        .map_err(|err| format!("why JSON should deserialize: {err}\n{json}"))?;
    if value
        .pointer("/evaluation/result_class")
        .and_then(serde_json::Value::as_str)
        != Some("exact_after_full_fallback")
    {
        return Err("full fallback should expose its stable result class".to_string());
    }
    let candidate = value
        .pointer("/candidate_entries/0")
        .ok_or_else(|| "candidate entry should be present".to_string())?;
    if candidate.get("family").is_some() {
        return Err("unavailable candidate family should be omitted".to_string());
    }
    if candidate.get("path") != Some(&serde_json::Value::Null)
        || candidate.get("glob") != Some(&serde_json::Value::Null)
        || candidate.get("selector_glob") != Some(&serde_json::Value::Null)
    {
        return Err("candidate selector relationship fields should remain null".to_string());
    }
    let partial_json = render_why_json(WhyReport {
        inventory: InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(12))
            .with_completeness("partial"),
        evaluation: EvaluationContext {
            scope: "scoped",
            locality: "proven",
            reasons: &[],
        },
        finding: &finding,
        outcome: &outcome,
        candidate_entries: &candidates,
        suggested_actions: &actions,
        proof_commands: &proofs,
        proof_plans: &plans,
    });
    let partial_value: serde_json::Value = serde_json::from_str(&partial_json)
        .map_err(|err| format!("partial why JSON should deserialize: {err}\n{partial_json}"))?;
    if partial_value
        .pointer("/evaluation/result_class")
        .and_then(serde_json::Value::as_str)
        != Some("target_scanner_partial")
    {
        return Err("partial scoped evaluation should expose its stable result class".to_string());
    }
    Ok(())
}

#[test]
fn evaluation_result_class_is_optional_for_legacy_context_pairs() {
    let legacy = EvaluationContext {
        scope: "legacy",
        locality: "unknown",
        reasons: &[],
    };
    assert_eq!(
        legacy.result_class(InventoryContext::source_syntax("git_tracked", None, None)),
        None
    );
}

#[test]
fn evaluation_result_class_names_incomplete_scoped_and_fallback_paths() {
    let scoped_evaluation = EvaluationContext {
        scope: "scoped",
        locality: "proven",
        reasons: &[],
    };
    assert_eq!(
        scoped_evaluation.result_class(
            InventoryContext::source_syntax("git_tracked", None, None).with_completeness("partial"),
        ),
        Some("target_scanner_partial")
    );

    let reasons = vec!["repository-wide policy scope".to_string()];
    let fallback_evaluation = EvaluationContext {
        scope: "full_fallback",
        locality: "global_dependency",
        reasons: &reasons,
    };
    for completeness in ["partial", "fallback"] {
        assert_eq!(
            fallback_evaluation.result_class(
                InventoryContext::source_syntax("git_tracked", None, None)
                    .with_completeness(completeness),
            ),
            Some("full_fallback_unavailable"),
            "full fallback must remain explicitly unavailable: {completeness}",
        );
    }
}

#[test]
fn scoped_result_class_uses_target_scanner_evidence_over_repository_inventory() {
    let evaluation = EvaluationContext {
        scope: "scoped",
        locality: "proven",
        reasons: &[],
    };
    let inventory =
        InventoryContext::source_syntax("git_tracked", None, None).with_completeness("partial");
    assert_eq!(
        evaluation.result_class_with_scanner_completeness(inventory, Some("complete")),
        Some("exact_scoped")
    );
    assert_eq!(
        evaluation.result_class_with_scanner_completeness(inventory, Some("partial")),
        Some("target_scanner_partial")
    );
}
