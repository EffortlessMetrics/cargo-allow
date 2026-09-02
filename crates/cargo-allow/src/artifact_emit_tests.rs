use super::*;

fn context<'a>(report: &'a allow_report::ReportContext<'a>) -> ArtifactEmitContext<'a> {
    static FINDINGS: [allow_core::Finding; 0] = [];
    static OUTCOMES: [allow_core::MatchOutcome; 0] = [];
    ArtifactEmitContext {
        command: "check",
        findings: &FINDINGS,
        outcomes: &OUTCOMES,
        failed: false,
        report_context: report,
        receipt_context: None,
    }
}

fn config<'a>(identity: &'a str, source: &'a str) -> EmitConfig<'a> {
    EmitConfig {
        operation: "check",
        formats: &[],
        result_class: EvaluationResultClassV2::Passed,
        blocking: false,
        resolved_config_identity: identity,
        source_subject: source,
    }
}

#[test]
fn semantic_digest_binds_identity_and_subject() -> Result<(), String> {
    let report = allow_report::ReportContext::default();
    let ctx = context(&report);
    let baseline = semantic_result_digest(&config("policy-a", "subject-a"), &ctx);
    if baseline == semantic_result_digest(&config("policy-b", "subject-a"), &ctx) {
        return Err("configuration identity must affect the digest".to_string());
    }
    if baseline == semantic_result_digest(&config("policy-a", "subject-b"), &ctx) {
        return Err("source subject must affect the digest".to_string());
    }
    if baseline != semantic_result_digest(&config("policy-a", "subject-a"), &ctx) {
        return Err("equal inputs must produce equal digests".to_string());
    }
    Ok(())
}

#[test]
fn semantic_digest_binds_outcome_payload() -> Result<(), String> {
    let report = allow_report::ReportContext::default();
    let empty = context(&report);
    let matched = allow_core::MatchOutcome {
        status: allow_core::MatchStatus::Matched,
        allow_id: Some("policy-a".to_string()),
        candidate_ids: vec!["policy-a".to_string()],
        finding_index: None,
        message: "matched".to_string(),
        score: 1,
    };
    let with_outcome = ArtifactEmitContext {
        outcomes: std::slice::from_ref(&matched),
        ..empty
    };
    if semantic_result_digest(&config("policy-a", "subject-a"), &empty)
        == semantic_result_digest(&config("policy-a", "subject-a"), &with_outcome)
    {
        return Err("outcome payload must affect the digest".to_string());
    }
    Ok(())
}

#[test]
fn semantic_digest_binds_finding_message() -> Result<(), String> {
    let report = allow_report::ReportContext::default();
    let finding_a = allow_core::Finding {
        kind: allow_core::FindingKind::Panic,
        family: None,
        path: std::path::PathBuf::from("src/lib.rs"),
        span: None,
        identity: allow_core::StructuralIdentity::new("rust", "call"),
        message: "first message".to_string(),
        ledger: None,
    };
    let finding_b = allow_core::Finding {
        message: "second message".to_string(),
        ..finding_a.clone()
    };
    let ctx_a = ArtifactEmitContext {
        findings: std::slice::from_ref(&finding_a),
        report_context: &report,
        ..context(&report)
    };
    let ctx_b = ArtifactEmitContext {
        findings: std::slice::from_ref(&finding_b),
        report_context: &report,
        ..context(&report)
    };
    let cfg = config("policy-a", "subject-a");
    if semantic_result_digest(&cfg, &ctx_a) == semantic_result_digest(&cfg, &ctx_b) {
        return Err("finding message must affect the digest".to_string());
    }
    Ok(())
}

#[test]
fn semantic_digest_binds_diff_head_revision() -> Result<(), String> {
    let mut report_a = allow_report::ReportContext::default();
    report_a.diff_analysis = Some(allow_report::DiffAnalysisContext {
        head_revision: Some("head-a"),
        ..allow_report::DiffAnalysisContext::default()
    });
    let mut report_b = report_a;
    report_b.diff_analysis = Some(allow_report::DiffAnalysisContext {
        head_revision: Some("head-b"),
        ..allow_report::DiffAnalysisContext::default()
    });
    let ctx_a = context(&report_a);
    let ctx_b = context(&report_b);
    let cfg = config("policy-a", "subject-a");
    if semantic_result_digest(&cfg, &ctx_a) == semantic_result_digest(&cfg, &ctx_b) {
        return Err("diff head revision must affect the digest".to_string());
    }
    Ok(())
}
