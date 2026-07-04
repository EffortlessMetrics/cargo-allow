use super::*;

#[test]
fn matches_moved_line_by_structure() {
    let mut id = StructuralIdentity::new("rust", "method_call");
    id.container = Some("load".to_string());
    id.callee = Some("unwrap".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 50,
            column: 12,
        }),
        identity: id,
        message: String::new(),
        ledger: None,
    };
    let entry = AllowEntry {
        id: "allow-1".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "test".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-12-31".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("unwrap".to_string()),
            line_hint: Some(12),
            ..Selector::default()
        },
        last_seen: None,
    };
    assert!(score_match(&entry, &finding).unwrap() >= 80);
}

#[test]
fn snippet_hash_selector_rejects_different_source() {
    let finding = finding_with_hash("fnv1a64:actual");
    let entry = entry_with_hash("fnv1a64:expected");

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn snippet_hash_selector_accepts_same_source() {
    let finding = finding_with_hash("fnv1a64:actual");
    let entry = entry_with_hash("fnv1a64:actual");

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn structural_field_mismatch_rejects_match() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.container = Some("other_container".to_string());

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn score_match_accepts_selector_glob_when_entry_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = None;
    entry.selector.glob = Some("src/**/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn top_level_glob_matches_when_entry_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("src/**/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn receiver_fingerprint_requires_exact_match() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.receiver_fingerprint = Some("workspace.config.requirements".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");

    // Exact match scores.
    entry.selector.receiver_fingerprint = Some("workspace.config.requirements".to_string());
    let exact_score = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("exact receiver should match"));

    // Substring (partial) does NOT match — exact-only (#1800).
    entry.selector.receiver_fingerprint = Some("config".to_string());
    assert_eq!(
        score_match(&entry, &finding),
        None,
        "substring receiver must not match after #1800 fix"
    );

    // Sanity: the exact score is above threshold.
    assert!(exact_score > 0);
}

#[test]
fn target_fingerprint_selector_requires_exact_match() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");

    // Exact match scores.
    entry.selector.target_fingerprint = Some("safety-comment:present".to_string());
    assert!(score_match(&entry, &finding).is_some());

    // Substring does NOT match — exact-only (#1800).
    entry.selector.target_fingerprint = Some("comment:present".to_string());
    assert_eq!(
        score_match(&entry, &finding),
        None,
        "substring target must not match after #1800 fix"
    );

    entry.selector.target_fingerprint = Some("comment:missing".to_string());
    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn score_match_rejects_when_no_path_or_glob_matches() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("tests/**/*.rs".to_string());
    entry.selector.glob = Some("examples/**/*.rs".to_string());

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn score_match_requires_exact_receiver_no_partial() {
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.receiver_fingerprint = Some("value".to_string());

    let mut exact = finding_with_hash("fnv1a64:actual");
    exact.identity.receiver_fingerprint = Some("value".to_string());
    let mut partial = finding_with_hash("fnv1a64:actual");
    partial.identity.receiver_fingerprint = Some("context.value".to_string());

    let exact_score = score_match(&entry, &exact)
        .unwrap_or_else(|| std::panic::panic_any("exact receiver fingerprint should match"));
    // Partial (substring) receiver must NOT match after #1800.
    assert_eq!(
        score_match(&entry, &partial),
        None,
        "partial receiver must not match after #1800"
    );
    assert!(exact_score > 0);
}

#[test]
fn score_match_uses_last_seen_line_when_selector_line_hint_is_absent() {
    // #2041: line-distance scoring was cosmetic and never affected matching
    // decisions. With the MatchStrength model, last_seen no longer contributes
    // to the priority — the entry still matches at the same strength tier.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.line_hint = None;

    let without_last_seen = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("entry should match without last_seen"));
    entry.last_seen = Some(LastSeen {
        line: 50,
        column: 12,
    });
    let with_last_seen = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("entry should match with last_seen"));

    assert_eq!(
        with_last_seen, without_last_seen,
        "MatchStrength priority is tier-based, not line-distance-adjusted"
    );
}

#[test]
fn entry_glob_matches_finding_path_when_exact_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("src/**/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn selector_glob_matches_finding_path_when_entry_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.selector.glob = Some("src/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn source_code_scope_only_selector_does_not_match() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector = Selector {
        glob: Some("src/lib.rs".to_string()),
        ..Selector::default()
    };

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn receiver_fingerprint_requires_exact_match_only() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.receiver_fingerprint = Some("config.loader.result".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");

    // Exact match.
    entry.selector.receiver_fingerprint = Some("config.loader.result".to_string());
    let exact = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("exact receiver should match"));

    // Substring (partial) does NOT match — exact-only (#1800).
    entry.selector.receiver_fingerprint = Some("loader".to_string());
    assert_eq!(
        score_match(&entry, &finding),
        None,
        "partial receiver must not match after #1800"
    );

    // Mismatch does not match.
    entry.selector.receiver_fingerprint = Some("other".to_string());
    assert_eq!(score_match(&entry, &finding), None);
    assert!(exact > 0);
}

#[test]
fn target_and_symbol_selectors_require_exact_matches() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.symbol =
        Some(r#"#[expect(clippy::unwrap_used, reason = "policy:allow-1")]"#.to_string());
    finding.identity.target_fingerprint = Some("policy:allow-1".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");

    // Exact symbol + exact target match.
    entry.selector.symbol =
        Some(r#"#[expect(clippy::unwrap_used, reason = "policy:allow-1")]"#.to_string());
    entry.selector.target_fingerprint = Some("policy:allow-1".to_string());
    assert!(score_match(&entry, &finding).is_some());

    // Substring symbol does NOT match — exact-only (#1800).
    entry.selector.symbol = Some("clippy::unwrap_used".to_string());
    assert_eq!(
        score_match(&entry, &finding),
        None,
        "substring symbol must not match after #1800"
    );

    // Substring target does NOT match.
    entry.selector.symbol =
        Some(r#"#[expect(clippy::unwrap_used, reason = "policy:allow-1")]"#.to_string());
    entry.selector.target_fingerprint = Some("allow-1".to_string());
    assert_eq!(
        score_match(&entry, &finding),
        None,
        "substring target must not match after #1800"
    );

    // Mismatch does not match.
    entry.selector.target_fingerprint = Some("policy:allow-1".to_string());
    entry.selector.symbol = Some("clippy::panic".to_string());
    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn last_seen_line_hint_contributes_when_selector_hint_is_absent() {
    // #2041: line-distance scoring was cosmetic; MatchStrength is tier-based.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    let base = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("baseline match should score"));

    entry.last_seen = Some(allow_core::LastSeen {
        line: 50,
        column: 12,
    });
    let with_last_seen = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("last seen match should score"));

    assert_eq!(
        with_last_seen, base,
        "MatchStrength priority is tier-based, not line-distance-adjusted"
    );
}

#[test]
fn scoring_accepts_entry_glob_and_selector_glob() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.path = PathBuf::from("crates/parser/src/lib.rs");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("crates/*/src/*.rs".to_string());
    assert!(score_match(&entry, &finding).is_some());

    entry.glob = None;
    entry.selector.glob = Some("crates/**/lib.rs".to_string());
    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn scoring_rejects_kind_family_and_path_mismatches() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.kind = FindingKind::Panic;
    assert_eq!(score_match(&entry, &finding), None);

    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.family = Some("unsafe_block".to_string());
    assert_eq!(score_match(&entry, &finding), None);

    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = Some(PathBuf::from("src/other.rs"));
    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn scoring_weights_exact_fingerprints_only() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.receiver_fingerprint = Some("parser.load_result".to_string());
    finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
    let mut exact = entry_with_hash("fnv1a64:actual");
    exact.selector.receiver_fingerprint = Some("parser.load_result".to_string());
    exact.selector.target_fingerprint = Some("safety-comment:present".to_string());
    let exact_score = score_match(&exact, &finding)
        .unwrap_or_else(|| std::panic::panic_any("exact fingerprints should match"));

    // Substring receiver does NOT match — exact-only (#1800).
    let mut partial = exact.clone();
    partial.selector.receiver_fingerprint = Some("load_result".to_string());
    assert_eq!(
        score_match(&partial, &finding),
        None,
        "substring receiver must not match after #1800"
    );

    // Mismatch does not match.
    let mut mismatch = exact;
    mismatch.selector.receiver_fingerprint = Some("other_result".to_string());

    assert!(exact_score > 0);
    assert_eq!(score_match(&mismatch, &finding), None);
}

#[test]
fn classify_match_returns_scoped_family_for_path_only_entry() {
    // #2041: a path/family-only entry (no structural selector fields) matches
    // at the ScopedFamily tier — the broadest, least-specific strength.
    let entry = AllowEntry {
        id: "test".to_string(),
        kind: FindingKind::NonRustFile,
        family: Some("configuration".to_string()),
        path: Some(PathBuf::from("docs/policy.md")),
        glob: None,
        owner: "core".to_string(),
        classification: "fixture".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            glob: Some("docs/policy.md".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };
    let finding = Finding {
        kind: FindingKind::NonRustFile,
        family: Some("configuration".to_string()),
        path: PathBuf::from("docs/policy.md"),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("non-rust", "tracked_file"),
        message: "test".to_string(),
        ledger: None,
    };
    assert_eq!(
        classify_match(&entry, &finding),
        Some(MatchStrength::ScopedFamily)
    );
}

#[test]
fn classify_match_returns_exact_occurrence_when_snippet_hash_present() {
    // #2041: an entry with normalized_snippet_hash matches at the
    // ExactOccurrence tier — the strongest anchor.
    let mut entry = AllowEntry {
        id: "test".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "fixture".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            normalized_snippet_hash: Some("fnv1a64:abc".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };
    let _ = &mut entry;
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.callee = Some("unwrap".to_string());
    identity.normalized_snippet_hash = Some("fnv1a64:abc".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: "test".to_string(),
        ledger: None,
    };
    assert_eq!(
        classify_match(&entry, &finding),
        Some(MatchStrength::ExactOccurrence)
    );
}

#[test]
fn match_strength_priority_orders_correctly() {
    // #2041: ExactOccurrence > Structural > ScopedFamily for tie-breaking.
    assert!(MatchStrength::ExactOccurrence.as_priority() > MatchStrength::Structural.as_priority());
    assert!(MatchStrength::Structural.as_priority() > MatchStrength::ScopedFamily.as_priority());
}
