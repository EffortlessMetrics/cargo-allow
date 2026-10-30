use super::{
    AddBroadRequest, AddEntryRequest, allow_entry_broad, allow_entry_from_finding,
    count_in_scope_findings, ensure_addable_outcome, ensure_broad_scope_addable, next_allow_id,
    select_add_finding,
};
use allow_core::{
    AllowConfig, AllowEntry, CargoAllowErrorKind, Finding, FindingKind, Lifecycle, MatchStatus,
    Selector, Span, StructuralIdentity,
};
use std::path::{Path, PathBuf};

fn entry_with_id(id: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "owner".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec!["test:fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector::default(),
        last_seen: None,
    }
}

#[test]
fn ensure_addable_outcome_boundary_accepts_new_status() {
    assert!(ensure_addable_outcome(MatchStatus::New).is_ok());
}

#[test]
fn ensure_addable_outcome_rejects_matched_findings_with_exact_error() {
    let error = ensure_addable_outcome(MatchStatus::Matched)
        .expect_err("matched findings should be rejected");
    assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
}

#[test]
fn ensure_addable_outcome_rejects_stale_findings_with_exact_error() {
    let error =
        ensure_addable_outcome(MatchStatus::Stale).expect_err("stale findings should be rejected");
    assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
}

#[test]
fn next_allow_id_returns_first_available_candidate() {
    let cfg = AllowConfig::empty();

    assert_eq!(next_allow_id(&cfg), "allow-0001");
}

#[test]
fn next_allow_id_skips_existing_allow_ids() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry_with_id("allow-0001"));

    assert_eq!(next_allow_id(&cfg), "allow-0002");
}

fn panic_unwrap_finding(path: &str, line: u32) -> Finding {
    let mut identity = StructuralIdentity::new(path, "method_call");
    identity.callee = Some("unwrap".to_string());
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 10 }),
        identity,
        message: "panic.unwrap".to_string(),
        ledger: None,
    }
}

#[test]
fn allow_entry_broad_builds_a_no_snippet_hash_selector() {
    // #2056: a broad baseline selector must NOT carry a normalized_snippet_hash
    // (it covers every in-scope occurrence), and must scope to the glob.
    let entry = allow_entry_broad(AddBroadRequest {
        id: "allow-broad".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/foo.rs".to_string(),
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "baseline".to_string(),
        evidence: Vec::new(),
        review_after: "2026-12-01".to_string(),
        expires: Some("2027-12-31".to_string()),
    });

    assert_eq!(entry.id, "allow-broad");
    assert_eq!(entry.kind, FindingKind::Panic);
    assert_eq!(entry.family.as_deref(), Some("unwrap"));
    assert_eq!(entry.glob.as_deref(), Some("src/foo.rs"));
    assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
    assert_eq!(entry.selector.glob.as_deref(), Some("src/foo.rs"));
    assert!(
        entry.selector.normalized_snippet_hash.is_none(),
        "broad selector must not pin a single snippet"
    );
    assert!(
        entry.selector.has_structural_identity(),
        "callee counts as structural identity so Panic matches"
    );
    assert_eq!(entry.occurrence_limit, None, "cmd_add pins the count");
}

#[test]
fn count_in_scope_findings_counts_every_matching_occurrence() {
    // Two unwraps in the scoped file both match a broad callee=unwrap selector.
    let entry = allow_entry_broad(AddBroadRequest {
        id: "allow-broad".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/foo.rs".to_string(),
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "baseline".to_string(),
        evidence: Vec::new(),
        review_after: "2026-12-01".to_string(),
        expires: Some("2027-12-31".to_string()),
    });
    let findings = vec![
        panic_unwrap_finding("src/foo.rs", 1),
        panic_unwrap_finding("src/foo.rs", 2),
    ];

    assert_eq!(count_in_scope_findings(&findings, &entry), 2);
}

#[test]
fn count_in_scope_findings_excludes_out_of_scope_and_non_matching() {
    let entry = allow_entry_broad(AddBroadRequest {
        id: "allow-broad".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/foo.rs".to_string(),
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "baseline".to_string(),
        evidence: Vec::new(),
        review_after: "2026-12-01".to_string(),
        expires: Some("2027-12-31".to_string()),
    });
    // One in-scope unwrap, one in a different file (out of scope), one expect
    // in-scope (different callee -> does not match).
    let mut expect_finding = panic_unwrap_finding("src/foo.rs", 5);
    expect_finding.identity.callee = Some("expect".to_string());
    let findings = vec![
        panic_unwrap_finding("src/foo.rs", 1),
        panic_unwrap_finding("src/other.rs", 2),
        expect_finding,
    ];

    assert_eq!(
        count_in_scope_findings(&findings, &entry),
        1,
        "only the in-scope unwrap matches"
    );
}

fn broad_unwrap_entry(id: &str) -> AllowEntry {
    allow_entry_broad(AddBroadRequest {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/foo.rs".to_string(),
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "baseline".to_string(),
        evidence: Vec::new(),
        review_after: "2099-12-31".to_string(),
        expires: None,
    })
}

/// #4603: when the live ledger already holds the identical broad baseline, a
/// repeated broad add must be refused with the typed Usage posture error
/// naming the covering entry, instead of appending an equal-strength duplicate
/// that would flip the satisfied scope to `Ambiguous`.
#[test]
fn ensure_broad_scope_addable_refuses_an_identical_broad_baseline()
-> Result<(), Box<dyn std::error::Error>> {
    let entry = broad_unwrap_entry("allow-broad");
    let mut cfg = AllowConfig::empty();
    // Mirror what cmd_add wrote on the first run: the broad entry with the
    // in-scope count pinned as its ratchet floor.
    let mut receipted = entry.clone();
    let findings = vec![panic_unwrap_finding("src/foo.rs", 1)];
    receipted.occurrence_limit = Some(count_in_scope_findings(&findings, &receipted));
    cfg.allow.push(receipted);

    let error = match ensure_broad_scope_addable(&cfg, &entry) {
        Err(error) => error,
        Ok(()) => {
            return Err("an identical broad baseline must be refused".into());
        }
    };
    assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
    let message = error.to_string();
    assert!(
        message.contains("already baselined by existing entry"),
        "the refusal must name the satisfied posture: {message}"
    );
    assert!(
        message.contains("allow-broad"),
        "the refusal must name the covering entry: {message}"
    );
    assert!(
        message.contains("`ambiguous`"),
        "the refusal must name the ambiguity the duplicate would create: {message}"
    );
    assert!(
        message.contains("use list or explain before editing policy"),
        "the refusal must mirror the --path/--line posture remedy: {message}"
    );
    Ok(())
}

/// #4603 judgment: coverage by a differently-shaped (anchored) entry stays
/// writable — an anchored receipt outscores a broad selector, so the match
/// winner stays unique and no ambiguity is created. This is the documented
/// init self-receipt then broad-baseline adoption flow.
#[test]
fn ensure_broad_scope_addable_keeps_differently_shaped_coverage_writable()
-> Result<(), Box<dyn std::error::Error>> {
    let finding = panic_unwrap_finding("src/foo.rs", 1);
    // The live ledger receipts the occurrence with an anchored --path/--line
    // entry, not a broad baseline.
    let anchored = allow_entry_from_finding(AddEntryRequest {
        finding: &finding,
        id: "allow-anchored".to_string(),
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "receipt one occurrence".to_string(),
        evidence: vec!["test:fixture".to_string()],
        review_after: "2099-12-31".to_string(),
        expires: None,
    });
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(anchored);

    let entry = broad_unwrap_entry("allow-broad");
    match ensure_broad_scope_addable(&cfg, &entry) {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(format!("differently-shaped coverage must stay writable, got: {error}").into())
        }
    }
}

/// #4603 judgment: a broad baseline over a different callee is a different
/// selector shape, not a duplicate.
#[test]
fn ensure_broad_scope_addable_keeps_a_different_callee_baseline_writable()
-> Result<(), Box<dyn std::error::Error>> {
    let mut expect_receipt = broad_unwrap_entry("allow-expect");
    expect_receipt.family = Some("expect".to_string());
    expect_receipt.selector.callee = Some("expect".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(expect_receipt);

    let entry = broad_unwrap_entry("allow-broad");
    match ensure_broad_scope_addable(&cfg, &entry) {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(format!("a different-callee baseline is not a duplicate, got: {error}").into())
        }
    }
}

/// A first-time broad baseline on an empty ledger has nothing to duplicate.
#[test]
fn ensure_broad_scope_addable_allows_a_first_time_baseline() {
    let entry = broad_unwrap_entry("allow-broad");
    let cfg = AllowConfig::empty();

    assert!(ensure_broad_scope_addable(&cfg, &entry).is_ok());
}

fn panic_kind_filter() -> Result<crate::KindFilter, Box<dyn std::error::Error>> {
    match crate::parse_kind_filter("panic") {
        Ok(kind) => Ok(kind),
        Err(error) => Err(format!("parse panic kind filter: {error}").into()),
    }
}

/// #4610: the no-current-finding usage error carries a typed next step, so the
/// rendered report complements the in-message remedy instead of appending the
/// contradictory generic "review the command syntax" line.
#[test]
fn select_add_finding_no_finding_error_carries_a_typed_next_step()
-> Result<(), Box<dyn std::error::Error>> {
    let kind = panic_kind_filter()?;
    let error = match select_add_finding(&[], kind, Path::new("src/lib.rs"), 1) {
        Err(error) => error,
        Ok(_) => return Err("an empty finding list must refuse the add".into()),
    };

    let next_step = error
        .next_step()
        .ok_or("the no-finding error must carry a typed next step")?;
    assert!(
        next_step.contains("pass the exact --line"),
        "the next step must direct at an exact --line: {next_step}"
    );
    let report = crate::error_report::format_cli_error(&error);
    assert!(
        report.contains("Next: pass the exact --line"),
        "the typed next step must render: {report}"
    );
    assert!(
        !report.contains("review the command syntax"),
        "the generic syntax line must not contradict the typed remedy: {report}"
    );
    assert_eq!(report.matches("Next:").count(), 1, "{report}");
    Ok(())
}

/// #4610: the ambiguous-add usage error carries a typed next step pointing at
/// the read-only way to find the exact line numbers, replacing the generic
/// syntax line.
#[test]
fn select_add_finding_ambiguous_error_carries_a_typed_next_step()
-> Result<(), Box<dyn std::error::Error>> {
    let kind = panic_kind_filter()?;
    // Two findings one line away from the requested line: an exact tie.
    let findings = vec![
        panic_unwrap_finding("src/foo.rs", 1),
        panic_unwrap_finding("src/foo.rs", 3),
    ];
    let error = match select_add_finding(&findings, kind, Path::new("src/foo.rs"), 2) {
        Err(error) => error,
        Ok(_) => return Err("an exact two-finding tie must refuse the add".into()),
    };

    let next_step = error
        .next_step()
        .ok_or("the ambiguous-add error must carry a typed next step")?;
    assert!(
        next_step.contains("cargo-allow check --kind panic --format json"),
        "the next step must point at the read-only line-number listing: {next_step}"
    );
    let report = crate::error_report::format_cli_error(&error);
    assert!(
        report.contains("Next: run `cargo-allow check --kind panic --format json`"),
        "the typed next step must render: {report}"
    );
    assert!(
        !report.contains("review the command syntax"),
        "the generic syntax line must not contradict the typed remedy: {report}"
    );
    assert_eq!(report.matches("Next:").count(), 1, "{report}");
    Ok(())
}

/// #4678: a wrong-case `--path` can never match a finding exactly (matching is
/// case-sensitive by law), and the stock refusal then claims the file may be
/// untracked when it is tracked. The rejection must append the tracked
/// spelling as remediation while keeping the #4610 typed next step intact.
#[test]
fn select_add_finding_wrong_case_path_carries_the_case_alias_remediation()
-> Result<(), Box<dyn std::error::Error>> {
    let kind = panic_kind_filter()?;
    let findings = vec![panic_unwrap_finding("src/lib.rs", 7)];
    let error = match select_add_finding(&findings, kind, Path::new("SRC/LIB.RS"), 7) {
        Err(error) => error,
        Ok(_) => return Err("a wrong-case path must refuse the add".into()),
    };

    let message = error.to_string();
    assert!(
        message.contains("differs only by letter case"),
        "the refusal must explain the case mismatch: {message}"
    );
    assert!(
        message.contains("retry with `--path src/lib.rs`"),
        "the refusal must name the tracked spelling: {message}"
    );
    assert!(
        message.contains("SRC/LIB.RS"),
        "the refusal must keep the requested spelling for context: {message}"
    );
    // The #4610 typed next step survives the decoration and renders alone.
    let next_step = error
        .next_step()
        .ok_or("the wrong-case refusal must keep the typed next step")?;
    assert!(
        next_step.contains("pass the exact --line"),
        "the typed next step must be unchanged: {next_step}"
    );
    let report = crate::error_report::format_cli_error(&error);
    assert_eq!(report.matches("Next:").count(), 1, "{report}");
    assert!(
        !report.contains("review the command syntax"),
        "the generic syntax line must stay replaced: {report}"
    );
    Ok(())
}

/// #4678 control: with no case-fold hit the no-finding refusal keeps its
/// historical message shape (the `--include-untracked` advice is then correct).
#[test]
fn select_add_finding_no_hit_keeps_the_plain_refusal_message()
-> Result<(), Box<dyn std::error::Error>> {
    let kind = panic_kind_filter()?;
    let findings = vec![panic_unwrap_finding("src/lib.rs", 7)];
    let error = match select_add_finding(&findings, kind, Path::new("src/other.rs"), 7) {
        Err(error) => error,
        Ok(_) => return Err("an unmatchable path must refuse the add".into()),
    };

    let message = error.to_string();
    assert!(
        message.contains("no current panic finding found near"),
        "{message}"
    );
    assert!(
        !message.contains("differs only by letter case"),
        "a non-alias refusal must not gain the case remediation: {message}"
    );
    Ok(())
}

/// CodeRabbit review follow-up on #4686: the alias remediation must only name
/// findings the requested kind could actually select. A case-matched path of a
/// different finding class would still refuse the add after the spelling fix.
#[test]
fn select_add_finding_case_alias_is_kind_filtered()
-> Result<(), Box<dyn std::error::Error>> {
    let kind = panic_kind_filter()?;
    let mut unsafe_finding = panic_unwrap_finding("src/lib.rs", 7);
    unsafe_finding.kind = FindingKind::Unsafe;
    unsafe_finding.family = Some("unsafe_fn".to_string());
    let findings = vec![unsafe_finding];

    let error = match select_add_finding(&findings, kind, Path::new("SRC/LIB.RS"), 7) {
        Err(error) => error,
        Ok(_) => return Err("a cross-kind wrong-case path must refuse the add".into()),
    };

    let message = error.to_string();
    assert!(
        !message.contains("differs only by letter case"),
        "a kind-incompatible alias must not gain the case remediation: {message}"
    );
    Ok(())
}
