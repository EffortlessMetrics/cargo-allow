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
fn receipt_render_for_unregistered_command_fails_instead_of_panicking() -> Result<(), String> {
    // #4546: the shared parser accepts `receipt` for every command, but the
    // receipt renderer asserts on unregistered commands. The emitter must
    // surface that as a render error so the validation gate types it.
    let report = allow_report::ReportContext::default();
    let ctx = ArtifactEmitContext {
        command: "audit",
        ..context(&report)
    };
    match render_one(RendererFormatV1::Receipt, &ctx) {
        Ok(_) => Err("receipt render on audit must not succeed".to_string()),
        Err(error) => {
            if !error.contains("receipt artifacts support only") {
                return Err(format!("error must name the receipt contract: {error}"));
            }
            if !error.contains("audit") {
                return Err(format!("error must name the offending command: {error}"));
            }
            Ok(())
        }
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
    let report_a = allow_report::ReportContext {
        diff_analysis: Some(allow_report::DiffAnalysisContext {
            head_revision: Some("head-a"),
            ..allow_report::DiffAnalysisContext::default()
        }),
        ..allow_report::ReportContext::default()
    };
    let report_b = allow_report::ReportContext {
        diff_analysis: Some(allow_report::DiffAnalysisContext {
            head_revision: Some("head-b"),
            ..allow_report::DiffAnalysisContext::default()
        }),
        ..allow_report::ReportContext::default()
    };
    let ctx_a = context(&report_a);
    let ctx_b = context(&report_b);
    let cfg = config("policy-a", "subject-a");
    if semantic_result_digest(&cfg, &ctx_a) == semantic_result_digest(&cfg, &ctx_b) {
        return Err("diff head revision must affect the digest".to_string());
    }
    Ok(())
}

#[test]
fn semantic_digest_binds_report_and_receipt_context() -> Result<(), String> {
    let report_a = allow_report::ReportContext::default();
    let mut report_b = report_a;
    report_b.rust_files_skipped = 1;
    let receipt = allow_report::ReportContext::default();
    let ctx_a = context(&report_a);
    let ctx_b = ArtifactEmitContext {
        report_context: &report_b,
        ..context(&report_b)
    };
    let ctx_c = ArtifactEmitContext {
        receipt_context: Some(&receipt),
        ..context(&report_a)
    };
    let cfg = config("policy-a", "subject-a");
    let baseline = semantic_result_digest(&cfg, &ctx_a);
    if baseline == semantic_result_digest(&cfg, &ctx_b) {
        return Err("report completeness must affect the digest".to_string());
    }
    if baseline == semantic_result_digest(&cfg, &ctx_c) {
        return Err("receipt context must affect the digest".to_string());
    }
    Ok(())
}

#[test]
fn artifact_dir_separation_covers_spelling_and_none_cases() {
    // #4561/#4576-review: the guard must refuse a directory that contains
    // the selected policy file across relative/absolute spellings, accept
    // separate locations, and no-op without a protected file.
    let root =
        std::env::temp_dir().join(format!("artifact-separation-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let policy = root.join("policy/allow.toml");
    std::fs::create_dir_all(policy.parent().unwrap_or_else(|| std::path::Path::new(".")))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    std::fs::write(&policy, "policy = \"cargo-allow\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    // Containing directories are refused across spellings: plain absolute,
    // and a parent-segment spelling that folds to the same directory.
    assert!(ensure_artifact_dir_separate(&root.join("policy"), Some(&policy),).is_err());
    assert!(ensure_artifact_dir_separate(&root.join("policy/../policy"), Some(&policy),).is_err());

    // A missing directory followed by `..` resolves through the fold: the
    // OS creates the missing segment then pops it, so the guard must treat
    // `newdir/..` as the parent (#4583 review).
    assert!(ensure_artifact_dir_separate(&root.join("newdir/../policy"), Some(&policy),).is_err());

    // A missing directory followed by `..` resolves through the fold: the
    // OS creates the missing segment then pops it, so the guard must treat
    // `newdir/..` as the parent (#4583 review).
    assert!(ensure_artifact_dir_separate(&root.join("newdir/../policy"), Some(&policy),).is_err());

    // A separate directory is accepted.
    assert!(
        ensure_artifact_dir_separate(std::path::Path::new("artifacts"), Some(&policy),).is_ok()
    );

    // No protected file: nothing to refuse.
    assert!(ensure_artifact_dir_separate(std::path::Path::new("policy"), None).is_ok());

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn artifact_dir_separation_resolves_symlinks_in_existing_prefixes() {
    // #4576 review: emission resolves `link/..` through the OS (link target
    // first, then the parent), so the guard must check the same directory.
    // A policy file inside the link target IS inside the resolved artifact
    // directory's parent chain and must be refused.
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "artifact-separation-symlink-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let target = root.join("target-dir");
    std::fs::create_dir_all(target.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create target dir: {err}")));
    let policy = target.join("policy/allow.toml");
    std::fs::write(&policy, "policy = \"cargo-allow\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    symlink(&target, root.join("link"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create symlink: {err}")));

    // `link/..` resolves through the link to the PARENT of target-dir; a
    // directory named target-dir/policy is not inside it, so this accepted
    // case pins that resolution happens at all...
    let resolved_parent = best_effort_canonical(&root.join("link/.."));
    assert_eq!(
        resolved_parent,
        root.canonicalize()
            .unwrap_or_else(|err| { std::panic::panic_any(format!("canonicalize root: {err}")) }),
        "link/.. must resolve through the link to root"
    );

    // ...and `link/policy` resolves INSIDE the link target, where the
    // policy file lives, so that containment is refused.
    assert!(ensure_artifact_dir_separate(&root.join("link/policy"), Some(&policy),).is_err());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn best_effort_canonical_degrades_gracefully_on_unresolvable_inputs() {
    // #4585 codecov: the defensive arms (pop past the filesystem root, and
    // an empty/unresolvable walk input) must return without panicking so
    // the guard degrades to a no-match instead of failing the command.
    let empty = best_effort_canonical(std::path::Path::new(""));
    assert!(empty.as_os_str().is_empty() || empty.as_os_str() == ".");

    // Leading `..` segments past the filesystem root cannot resolve; the
    // walk returns the lexically folded input rather than panicking.
    let escaped = best_effort_canonical(std::path::Path::new("/../../etc"));
    assert!(escaped.ends_with("etc"));
}
