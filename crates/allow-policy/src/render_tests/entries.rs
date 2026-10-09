use allow_core::{AllowConfig, AllowEntry, FindingKind, LastSeen, Lifecycle, Selector};
use std::path::PathBuf;

use crate::{
    PolicyLineEnding, append_policy_entry, detect_dominant_line_ending, parse_policy,
    render_policy, transpose_policy_rendering,
};

/// Count `\r\n` pairs and lone `\n` bytes, mirroring the writer's detector.
fn count_endings(bytes: &[u8]) -> (usize, usize) {
    let mut crlf = 0;
    let mut lone_lf = 0;
    let mut previous = 0u8;
    for &byte in bytes {
        if byte == b'\n' {
            if previous == b'\r' {
                crlf += 1;
            } else {
                lone_lf += 1;
            }
        }
        previous = byte;
    }
    (crlf, lone_lf)
}

fn appended_entry() -> Result<AllowEntry, Box<dyn std::error::Error>> {
    let cfg = parse_policy(
        "policy = 'cargo-allow'\n\n[[allow]]\n\
         id = 'allow-appended'\nkind = 'panic'\nfamily = 'unwrap'\n\
         path = 'src/new.rs'\nowner = 'parser'\nclassification = 'reviewed_exception'\n\
         reason = 'Retain one reviewed finding.'\nreview_after = '2027-01-01'\n\
         [allow.selector]\nast_kind = 'method_call'\ncallee = 'unwrap'\n",
    )?;
    cfg.allow
        .into_iter()
        .next()
        .ok_or("fixture entry missing".into())
}

#[test]
fn append_policy_entry_preserves_bytes_and_is_deterministic()
-> Result<(), Box<dyn std::error::Error>> {
    let entry = appended_entry()?;
    let historical = "\u{feff}# historical\r\npolicy = 'cargo-allow'\r\n\
        [[allow]]\r\nid = 'allow-existing'\r\nkind = 'panic'\r\n\
        path = 'src/old.rs'\r\nowner = 'owner'\r\nclassification = 'reviewed_exception'\r\n\
        reason = '''Quoted historical text'''\r\nreview_after = '2027-01-01'\r\n\
        [allow.selector]\r\nast_kind = 'method_call'\r\ncallee = 'unwrap'\r\n";
    for ending in ["", "\n", "\r\n", "\n\n", "\n# trailing comment"] {
        let input = format!("{historical}{ending}");
        let result = append_policy_entry(&input, &entry)?;
        assert!(result.as_bytes().starts_with(input.as_bytes()));
        assert_eq!(result, append_policy_entry(&input, &entry)?);
        let before = parse_policy(&input)?;
        let after = parse_policy(&result)?;
        assert_eq!(after.allow.first(), before.allow.first());
        assert_eq!(after.allow.last(), Some(&entry));
        assert_eq!(after.allow.len(), before.allow.len() + 1);
        assert!(
            append_policy_entry(&result, &entry).is_err(),
            "duplicate ID must fail"
        );
    }
    Ok(())
}

#[cfg(any(unix, windows))]
#[test]
fn append_policy_entry_refuses_non_utf8_paths() -> Result<(), Box<dyn std::error::Error>> {
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    #[cfg(windows)]
    use std::os::windows::ffi::OsStringExt;

    let input = "policy = 'cargo-allow'\n";
    let mut entry = appended_entry()?;
    entry.path = Some(PathBuf::from("src/日本語.rs"));
    let unicode = parse_policy(&append_policy_entry(input, &entry)?)?;
    assert_eq!(
        unicode.allow.first().map(|entry| &entry.path),
        Some(&entry.path)
    );

    let mut path = OsString::from("src/");
    #[cfg(unix)]
    path.push(OsString::from_vec(vec![0xff]));
    #[cfg(windows)]
    path.push(OsString::from_wide(&[0xd800]));
    path.push(".rs");
    entry.path = Some(PathBuf::from(path));
    let error = append_policy_entry(input, &entry)
        .err()
        .ok_or("a non-UTF-8 path must reject the append")?;
    assert!(
        error
            .to_string()
            .contains("allow-appended path must be valid UTF-8")
    );
    Ok(())
}

#[test]
fn append_policy_entry_refuses_incompatible_or_malformed_documents()
-> Result<(), Box<dyn std::error::Error>> {
    let entry = appended_entry()?;
    let explicit_array = "policy = 'cargo-allow'\nallow = []\n";
    assert!(parse_policy(explicit_array).is_ok());
    let error = append_policy_entry(explicit_array, &entry)
        .err()
        .ok_or("explicit allow array must reject the append")?;
    assert!(error.to_string().contains("without rewriting policy"));
    assert!(append_policy_entry("policy = 'unterminated", &entry).is_err());
    Ok(())
}

#[test]
fn append_policy_entry_obeys_policy_read_limit() -> Result<(), Box<dyn std::error::Error>> {
    let mut entry = appended_entry()?;
    entry.path = Some(PathBuf::from("src/日本語.rs"));
    let prefix = "\u{feff}# Historical\r\npolicy = 'cargo-allow'\r\n# ";
    let small = append_policy_entry(prefix, &entry)?;
    let delta = small.len() - prefix.len();
    let limit = usize::try_from(allow_core::SOURCE_FILE_READ_MAX_BYTES)?;
    let mut input = prefix.to_string();
    input.push_str(&"x".repeat(limit - delta - input.len()));
    let result = append_policy_entry(&input, &entry)?;
    assert_eq!(result.len(), limit, "the exact limit is still accepted");
    assert!(result.as_bytes().starts_with(input.as_bytes()));
    assert_eq!(parse_policy(&result)?.allow.last(), Some(&entry));

    // One extra ASCII byte crosses the byte cap even though the Japanese path
    // makes the result's character count smaller than that cap.
    input.push('x');
    let error = append_policy_entry(&input, &entry)
        .err()
        .ok_or("one byte over the normal loader limit must refuse")?;
    assert!(error.to_string().contains("8388609 bytes"));
    assert!(error.to_string().contains("8388608-byte read limit"));
    Ok(())
}

#[test]
fn detect_dominant_line_ending_classifies_envelopes() {
    // No line endings at all: nothing to preserve, keep the LF default.
    assert_eq!(detect_dominant_line_ending(b""), PolicyLineEnding::Lf);
    assert_eq!(
        detect_dominant_line_ending(b"policy = 'cargo-allow'"),
        PolicyLineEnding::Lf
    );
    // Lone CR is not a line ending.
    assert_eq!(detect_dominant_line_ending(b"a\rb\n"), PolicyLineEnding::Lf);
    // Uniform envelopes.
    assert_eq!(detect_dominant_line_ending(b"a\nb\n"), PolicyLineEnding::Lf);
    assert_eq!(
        detect_dominant_line_ending(b"a\r\nb\r\n"),
        PolicyLineEnding::Crlf
    );
    // Mixed envelopes round-trip their majority (#4337).
    assert_eq!(
        detect_dominant_line_ending(b"a\r\nb\r\nc\n"),
        PolicyLineEnding::Crlf
    );
    assert_eq!(
        detect_dominant_line_ending(b"a\nb\nc\r\n"),
        PolicyLineEnding::Lf
    );
    // A tied, contested envelope fails closed instead of guessing.
    assert_eq!(
        detect_dominant_line_ending(b"a\r\nb\n"),
        PolicyLineEnding::Ambiguous
    );
    assert_eq!(
        transpose_policy_rendering("a = 1\nb = 2\n", PolicyLineEnding::Crlf),
        "a = 1\r\nb = 2\r\n"
    );
    assert_eq!(
        transpose_policy_rendering("a = 1\n", PolicyLineEnding::Lf),
        "a = 1\n"
    );
}

#[test]
fn append_policy_entry_matches_the_ledgers_dominant_ending()
-> Result<(), Box<dyn std::error::Error>> {
    let entry = appended_entry()?;
    let crlf_ledger = "\u{feff}# historical\r\npolicy = 'cargo-allow'\r\n";
    let lf_ledger = "\u{feff}# historical\npolicy = 'cargo-allow'\n";

    // A CRLF ledger stays CRLF: the preimage is an exact byte prefix and the
    // appended block carries CRLF, so no lone LF bytes appear (#4279).
    let mut crlf_result = append_policy_entry(crlf_ledger, &entry)?;
    assert!(crlf_result.as_bytes().starts_with(crlf_ledger.as_bytes()));
    let (crlf_before, lone_before) = count_endings(crlf_ledger.as_bytes());
    let (crlf_after, lone_after) = count_endings(crlf_result.as_bytes());
    assert_eq!(lone_before, 0);
    assert_eq!(lone_after, 0, "a CRLF ledger must not grow lone LF bytes");
    assert!(crlf_after > crlf_before);
    assert_eq!(parse_policy(&crlf_result)?.allow.last(), Some(&entry));
    let appended = crlf_result.split_off(crlf_ledger.len());
    let (block_crlf, block_lone) = count_endings(appended.as_bytes());
    assert!(block_crlf > 0 && block_lone == 0);

    // An LF ledger stays LF.
    let mut lf_result = append_policy_entry(lf_ledger, &entry)?;
    assert!(lf_result.as_bytes().starts_with(lf_ledger.as_bytes()));
    let lf_appended = lf_result.split_off(lf_ledger.len());
    let (lf_block_crlf, _) = count_endings(lf_appended.as_bytes());
    assert_eq!(lf_block_crlf, 0, "an LF ledger must not grow CR bytes");

    // A mixed ledger round-trips its dominant envelope and never normalizes
    // the minority endings (#4337).
    let crlf_dominant_mixed = format!("{crlf_ledger}# recent LF note\n");
    let mixed_result = append_policy_entry(&crlf_dominant_mixed, &entry)?;
    assert!(
        mixed_result
            .as_bytes()
            .starts_with(crlf_dominant_mixed.as_bytes())
    );
    let (mixed_crlf_before, mixed_lone_before) = count_endings(crlf_dominant_mixed.as_bytes());
    let (mixed_crlf_after, mixed_lone_after) = count_endings(mixed_result.as_bytes());
    assert_eq!(
        mixed_lone_after, mixed_lone_before,
        "the minority LF endings must survive untouched"
    );
    assert!(mixed_crlf_after > mixed_crlf_before);

    let lf_dominant_mixed = format!("{lf_ledger}# stray CRLF comment\r\n");
    let lf_mixed_result = append_policy_entry(&lf_dominant_mixed, &entry)?;
    assert!(
        lf_mixed_result
            .as_bytes()
            .starts_with(lf_dominant_mixed.as_bytes())
    );
    let (dom_crlf_before, dom_lone_before) = count_endings(lf_dominant_mixed.as_bytes());
    let (dom_crlf_after, dom_lone_after) = count_endings(lf_mixed_result.as_bytes());
    assert_eq!(
        dom_crlf_after, dom_crlf_before,
        "the minority CRLF endings must survive untouched"
    );
    assert!(dom_lone_after > dom_lone_before);
    Ok(())
}

#[test]
fn append_policy_entry_refuses_a_contested_ending_envelope()
-> Result<(), Box<dyn std::error::Error>> {
    let entry = appended_entry()?;
    // Equal CRLF and lone-LF counts with both present have no dominant
    // envelope; the append must fail closed instead of guessing (#4337).
    let tied = "policy = 'cargo-allow'\r\n# crlf-two\r\n# lf-one\n# lf-two\n";
    let (crlf, lone_lf) = count_endings(tied.as_bytes());
    assert_eq!((crlf, lone_lf), (2, 2), "fixture must actually be tied");
    let error = append_policy_entry(tied, &entry)
        .err()
        .ok_or("a tied ending envelope must refuse the append")?;
    assert!(
        error.to_string().contains("line endings are ambiguous"),
        "unexpected error: {error}"
    );
    assert!(error.to_string().contains("policy unchanged"));
    Ok(())
}

#[test]
fn renders_and_parses_occurrence_limit() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(AllowEntry {
        id: "allow-counted".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "Generated baseline debt.".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: Some(3),
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: None,
            expires: Some("2026-08-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    });

    let rendered = render_policy(&cfg);
    assert!(rendered.contains("occurrence_limit = 3"));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert_eq!(
        reparsed
            .allow
            .first()
            .and_then(|entry| entry.occurrence_limit),
        Some(3)
    );
}

#[test]
fn renders_and_parses_escaped_basic_strings() {
    let mut cfg = AllowConfig::empty();
    let reason = "Quoted \"reason\"\nwith backslash \\ and tab\tinside";
    let evidence = "test:line\nbreak";
    cfg.allow.push(AllowEntry {
        id: "allow-escaped".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: reason.to_string(),
        evidence: vec![evidence.to_string()],
        links: vec!["doc:docs/quoted\"path.md".to_string()],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            symbol: Some("value[\"key\"]\n.unwrap()".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    });

    let rendered = render_policy(&cfg);
    assert!(
        rendered
            .contains("reason = \"Quoted \\\"reason\\\"\\nwith backslash \\\\ and tab\\tinside\"")
    );
    assert!(rendered.contains("evidence = [\"test:line\\nbreak\"]"));
    assert!(rendered.contains("links = [\"doc:docs/quoted\\\"path.md\"]"));
    assert!(rendered.contains("symbol = \"value[\\\"key\\\"]\\n.unwrap()\""));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    let entry = reparsed
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("rendered policy should keep allow entry"));
    assert_eq!(entry.reason, reason);
    assert_eq!(entry.evidence, vec![evidence.to_string()]);
    assert_eq!(entry.links, vec!["doc:docs/quoted\"path.md".to_string()]);
    assert_eq!(
        entry.selector.symbol.as_deref(),
        Some("value[\"key\"]\n.unwrap()")
    );
}

#[test]
fn renders_and_parses_selector_metadata() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(AllowEntry {
        id: "allow-selector-metadata".to_string(),
        kind: FindingKind::Panic,
        family: Some("indexing_slicing".to_string()),
        path: Some(PathBuf::from("src/parser/span.rs")),
        glob: None,
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates span ranges before slicing.".to_string(),
        evidence: vec!["test:parser_rejects_invalid_span_range".to_string()],
        links: vec!["doc:docs/parser-spans.md".to_string()],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-29".to_string()),
            review_after: Some("2026-08-29".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("index_expr".to_string()),
            container: Some("slice_checked_span".to_string()),
            callee: Some("index".to_string()),
            macro_name: Some("span_guard".to_string()),
            lint: Some("clippy::indexing_slicing".to_string()),
            symbol: Some("source[range]".to_string()),
            receiver_fingerprint: Some("fnv1a64:receiver".to_string()),
            target_fingerprint: Some("fnv1a64:target".to_string()),
            normalized_snippet_hash: Some("fnv1a64:snippet".to_string()),
            line_hint: Some(42),
            glob: Some("src/parser/span.rs".to_string()),
        },
        last_seen: Some(LastSeen {
            line: 45,
            column: 17,
        }),
    });

    let rendered = render_policy(&cfg);
    for expected in [
        "[allow.selector]",
        "ast_kind = \"index_expr\"",
        "container = \"slice_checked_span\"",
        "callee = \"index\"",
        "macro_name = \"span_guard\"",
        "lint = \"clippy::indexing_slicing\"",
        "symbol = \"source[range]\"",
        "receiver_fingerprint = \"fnv1a64:receiver\"",
        "target_fingerprint = \"fnv1a64:target\"",
        "normalized_snippet_hash = \"fnv1a64:snippet\"",
        "glob = \"src/parser/span.rs\"",
        "[allow.last_seen]",
        "line = 45",
        "column = 17",
    ] {
        assert!(
            rendered.contains(expected),
            "rendered policy should contain `{expected}`:\n{rendered}"
        );
    }

    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    let entry = reparsed
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("rendered policy should keep allow entry"));
    assert_eq!(entry.selector.ast_kind.as_deref(), Some("index_expr"));
    assert_eq!(
        entry.selector.container.as_deref(),
        Some("slice_checked_span")
    );
    assert_eq!(entry.selector.callee.as_deref(), Some("index"));
    assert_eq!(entry.selector.macro_name.as_deref(), Some("span_guard"));
    assert_eq!(
        entry.selector.lint.as_deref(),
        Some("clippy::indexing_slicing")
    );
    assert_eq!(entry.selector.symbol.as_deref(), Some("source[range]"));
    assert_eq!(
        entry.selector.receiver_fingerprint.as_deref(),
        Some("fnv1a64:receiver")
    );
    assert_eq!(
        entry.selector.target_fingerprint.as_deref(),
        Some("fnv1a64:target")
    );
    assert_eq!(
        entry.selector.normalized_snippet_hash.as_deref(),
        Some("fnv1a64:snippet")
    );
    assert_eq!(entry.selector.line_hint, None);
    assert_eq!(entry.selector.glob.as_deref(), Some("src/parser/span.rs"));
    assert_eq!(
        entry
            .last_seen
            .as_ref()
            .map(|last_seen| (last_seen.line, last_seen.column)),
        Some((45, 17))
    );
}
