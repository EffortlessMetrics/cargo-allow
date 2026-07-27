use super::*;
use proptest::prelude::*;

proptest! {
    #[test]
    fn classify_match_is_deterministic(
        ast_kind in "(unwrap|expect|panic|unsafe_fn|method_call)",
        callee in "(unwrap|expect|panic|foo|bar)",
        container in "(load|init|parse|process)",
    ) {
        let finding = test_finding(&ast_kind, &callee, &container);
        let entry = test_entry(&ast_kind, &callee, &container);

        let result1 = classify_match(&entry, &finding);
        let result2 = classify_match(&entry, &finding);

        prop_assert_eq!(result1, result2, "classify_match must be deterministic");
    }

    #[test]
    fn classify_match_stable_under_selector_whitespace(
        ast_kind in "(unwrap|expect|panic|unsafe_fn|method_call)",
        callee in "(unwrap|expect|panic|foo|bar)",
        container in "(load|init|parse|process)",
    ) {
        let finding = test_finding(&ast_kind, &callee, &container);
        let entry = test_entry(&ast_kind, &callee, &container);
        let baseline = classify_match(&entry, &finding);

        // Pad each selector field with leading/trailing whitespace.
        // The match decision should not change because classify_match uses
        // exact equality, and the finding's identity fields are unpadded.
        let mut padded_entry = entry.clone();
        if let Some(ref mut ak) = padded_entry.selector.ast_kind {
            *ak = format!("  {ak}  ");
        }
        if let Some(ref mut c) = padded_entry.selector.callee {
            *c = format!("  {c}  ");
        }
        if let Some(ref mut ct) = padded_entry.selector.container {
            *ct = format!("  {ct}  ");
        }

        let padded_result = classify_match(&padded_entry, &finding);
        // Padding selector fields should never CREATE a match that didn't
        // exist before. If baseline was None, padded must also be None.
        // If baseline was Some, padding may break the match (expected).
        if baseline.is_none() {
            prop_assert!(
                padded_result.is_none(),
                "padding selector fields should not create a false match"
            );
        }
    }

    #[test]
    fn classify_match_stable_under_repeated_calls(
        n in 1usize..=20,
        ast_kind in "(unwrap|expect|method_call)",
        callee in "(unwrap|expect|foo)",
    ) {
        let finding = test_finding(&ast_kind, &callee, "load");
        let entry = test_entry(&ast_kind, &callee, "load");

        let first = classify_match(&entry, &finding);
        for _ in 0..n {
            let again = classify_match(&entry, &finding);
            prop_assert_eq!(first, again, "repeated calls must be stable");
        }
    }
}

fn test_finding(ast_kind: &str, callee: &str, container: &str) -> Finding {
    let mut identity = StructuralIdentity::new("rust", ast_kind);
    identity.callee = Some(callee.to_string());
    identity.container = Some(container.to_string());
    identity.normalized_snippet_hash = Some(format!("fnv1a64:{callee}"));
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: "test finding".to_string(),
        ledger: None,
    }
}

fn test_entry(ast_kind: &str, callee: &str, container: &str) -> AllowEntry {
    AllowEntry {
        id: "allow-prop".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "test".to_string(),
        classification: "reviewed".to_string(),
        reason: "property test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-01-01".to_string()),
            review_after: Some("2027-01-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some(ast_kind.to_string()),
            callee: Some(callee.to_string()),
            container: Some(container.to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}
