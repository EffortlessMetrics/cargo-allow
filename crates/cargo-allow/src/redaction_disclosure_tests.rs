use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, Selector, Span, StructuralIdentity,
};

use super::{
    redaction_dead_entries, redaction_dead_entries_error, redaction_dead_entry_add_warning_message,
    redaction_dead_selector_fields,
};

fn selector(fields: SelectorFields) -> Selector {
    let SelectorFields {
        container,
        callee,
        macro_name,
        lint,
        symbol,
        receiver_fingerprint,
        target_fingerprint,
        normalized_snippet_hash,
    } = fields;
    Selector {
        ast_kind: Some("method_call".to_string()),
        container,
        callee,
        macro_name,
        lint,
        symbol,
        receiver_fingerprint,
        target_fingerprint,
        normalized_snippet_hash,
        line_hint: None,
        glob: None,
    }
}

#[derive(Default)]
struct SelectorFields {
    container: Option<String>,
    callee: Option<String>,
    macro_name: Option<String>,
    lint: Option<String>,
    symbol: Option<String>,
    receiver_fingerprint: Option<String>,
    target_fingerprint: Option<String>,
    normalized_snippet_hash: Option<String>,
}

fn entry(id: &str, selector: Selector) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(std::path::PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "test".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "test fixture".to_string(),
        evidence: vec!["test:fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector,
        last_seen: None,
    }
}

#[test]
fn dead_selector_fields_list_each_cleared_text_field() {
    // #4628: every selector field whose finding-side value is cleared before
    // matching is guaranteed dead and must be named individually.
    let cases = [
        (
            SelectorFields {
                container: Some("mod::Impl".to_string()),
                ..SelectorFields::default()
            },
            vec!["container"],
        ),
        (
            SelectorFields {
                callee: Some("unwrap".to_string()),
                ..SelectorFields::default()
            },
            vec!["callee"],
        ),
        (
            SelectorFields {
                macro_name: Some("debug_assert".to_string()),
                ..SelectorFields::default()
            },
            vec!["macro_name"],
        ),
        (
            SelectorFields {
                lint: Some("clippy::unwrap_used".to_string()),
                ..SelectorFields::default()
            },
            vec!["lint"],
        ),
        (
            SelectorFields {
                symbol: Some("value.unwrap()".to_string()),
                ..SelectorFields::default()
            },
            vec!["symbol"],
        ),
    ];
    for (fields, expected) in cases {
        assert_eq!(redaction_dead_selector_fields(&selector(fields)), expected);
    }
}

#[test]
fn dead_selector_fields_list_raw_text_fingerprints_but_not_slot_forms() {
    // #4629: raw-text fingerprints are cleared under redaction and are dead
    // for matching; `param:N` slot forms are preserved and stay live.
    let raw_receiver = SelectorFields {
        receiver_fingerprint: Some("customer_ssn".to_string()),
        ..SelectorFields::default()
    };
    assert_eq!(
        redaction_dead_selector_fields(&selector(raw_receiver)),
        vec!["receiver_fingerprint"]
    );

    let slot_receiver = SelectorFields {
        receiver_fingerprint: Some("param:0".to_string()),
        ..SelectorFields::default()
    };
    assert!(redaction_dead_selector_fields(&selector(slot_receiver)).is_empty());

    let target = SelectorFields {
        target_fingerprint: Some("0".to_string()),
        ..SelectorFields::default()
    };
    assert_eq!(
        redaction_dead_selector_fields(&selector(target)),
        vec!["target_fingerprint"]
    );
}

#[test]
fn dead_selector_fields_preserved_anchors_stay_live() {
    // #4628: ast_kind and normalized_snippet_hash are preserved under
    // redaction, so a snippet-hash-anchored entry must never be disclosed.
    let anchored = SelectorFields {
        normalized_snippet_hash: Some("fnv1a64:abc123".to_string()),
        ..SelectorFields::default()
    };
    assert!(redaction_dead_selector_fields(&selector(anchored)).is_empty());
    assert!(redaction_dead_selector_fields(&selector(SelectorFields::default())).is_empty());
}

#[test]
fn dead_entries_are_reported_sorted_with_their_fields() {
    // #4628: the disclosure must name every dead entry id; the listing is
    // id-sorted so repeated runs render identically.
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry(
        "allow-0002",
        selector(SelectorFields {
            callee: Some("unwrap".to_string()),
            ..SelectorFields::default()
        }),
    ));
    cfg.allow.push(entry(
        "allow-0001",
        selector(SelectorFields {
            container: Some("mod::Impl".to_string()),
            receiver_fingerprint: Some("customer_ssn".to_string()),
            ..SelectorFields::default()
        }),
    ));
    cfg.allow.push(entry(
        "allow-0003",
        selector(SelectorFields {
            normalized_snippet_hash: Some("fnv1a64:abc123".to_string()),
            ..SelectorFields::default()
        }),
    ));

    let dead = redaction_dead_entries(&cfg);

    let ids = dead.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert_eq!(ids, vec!["allow-0001", "allow-0002"]);
    assert_eq!(
        dead[0].1,
        vec!["container", "receiver_fingerprint"],
        "fields render in selector order"
    );
    assert_eq!(dead[1].1, vec!["callee"]);
}

#[test]
fn dead_entries_error_names_ids_fields_and_next_step() -> Result<(), String> {
    // #4628: the hard disclosure must be unmissable — entry ids, the dead
    // fields, and a typed Next line that routes to the unredacted scan or the
    // snippet-hash re-receipt path.
    let dead = vec![
        (
            "allow-0001",
            vec!["container", "callee"] as Vec<&'static str>,
        ),
        (
            "allow-0007",
            vec!["receiver_fingerprint"] as Vec<&'static str>,
        ),
    ];

    let error = redaction_dead_entries_error(&dead);

    if error.kind() != allow_core::CargoAllowErrorKind::Unsupported {
        return Err(format!(
            "disclosure must be a typed Unsupported error, got {:?}",
            error.kind()
        ));
    }
    let message = error.message().to_string();
    if !message.contains("CARGO_ALLOW_REDACT_IDENTITY=1 is active") {
        return Err(format!("message must name the redaction mode: {message}"));
    }
    if !message.contains("`allow-0001` (container, callee)") {
        return Err(format!(
            "message must name the entry and its fields: {message}"
        ));
    }
    if !message.contains("`allow-0007` (receiver_fingerprint)") {
        return Err(format!(
            "message must name every disclosed entry: {message}"
        ));
    }
    let Some(next) = error.next_step() else {
        return Err("disclosure must carry a typed Next line".to_string());
    };
    if !next.contains("without CARGO_ALLOW_REDACT_IDENTITY=1") {
        return Err(format!(
            "Next line must route to the unredacted scan: {next}"
        ));
    }
    if !next.contains("normalized_snippet_hash") {
        return Err(format!(
            "Next line must name the snippet-hash re-receipt path: {next}"
        ));
    }
    Ok(())
}

#[test]
fn dead_entries_error_summarizes_long_lists_by_count() -> Result<(), String> {
    // #4628: a ledger with many dead entries still discloses unmissably, but
    // the id list is capped so one message cannot grow unbounded.
    let ids = (0..12)
        .map(|index| format!("allow-{index:04}"))
        .collect::<Vec<_>>();
    let dead = ids
        .iter()
        .map(|id| (id.as_str(), vec!["callee"] as Vec<&'static str>))
        .collect::<Vec<_>>();

    let error = redaction_dead_entries_error(&dead);

    let message = error.message().to_string();
    if !message.contains("`allow-0007`") {
        return Err(format!(
            "disclosure should list ids up to the cap: {message}"
        ));
    }
    if message.contains("`allow-0008`") {
        return Err(format!("ids past the cap must be summarized: {message}"));
    }
    if !message.contains("and 4 more") {
        return Err(format!("cap remainder must be counted: {message}"));
    }
    Ok(())
}

#[test]
fn add_warning_names_entry_and_dead_fields() -> Result<(), String> {
    // #4628: the add advisory tells the operator the generated entry only
    // matches unredacted scans; it does not refuse.
    let receipt = entry(
        "allow-0009",
        selector(SelectorFields {
            container: Some("mod::Impl".to_string()),
            normalized_snippet_hash: Some("fnv1a64:abc123".to_string()),
            ..SelectorFields::default()
        }),
    );

    let Some(warning) = redaction_dead_entry_add_warning_message(&receipt) else {
        return Err("expected an add warning for a dead-field entry".to_string());
    };

    if !warning.contains("entry `allow-0009`") {
        return Err(format!("warning must name the entry: {warning}"));
    }
    if !warning.contains("(container)") {
        return Err(format!("warning must list the dead fields: {warning}"));
    }
    if !warning.contains("only match unredacted scans") {
        return Err(format!(
            "warning must state the unredacted-only match: {warning}"
        ));
    }
    Ok(())
}

#[test]
fn add_warning_is_silent_for_snippet_hash_anchored_entries() {
    // #4628: an entry generated from a redacted finding anchors on
    // normalized_snippet_hash alone and matches both modes — no warning.
    let receipt = entry(
        "allow-0010",
        selector(SelectorFields {
            normalized_snippet_hash: Some("fnv1a64:abc123".to_string()),
            ..SelectorFields::default()
        }),
    );

    assert!(redaction_dead_entry_add_warning_message(&receipt).is_none());
}

#[test]
fn disclosure_predicate_reads_scan_redacted_findings_shape() {
    // #4628 end-to-end shape check at the unit boundary: a finding whose
    // identity went through `redact_source_text_fields` carries None in every
    // field the disclosure treats as dead on the selector side.
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some("mod::Impl".to_string());
    identity.callee = Some("unwrap".to_string());
    identity.receiver_fingerprint = Some("customer_ssn".to_string());
    identity.normalized_snippet_hash = Some("fnv1a64:abc123".to_string());
    identity.redact_source_text_fields();
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: std::path::PathBuf::from("src/lib.rs"),
        span: Some(Span { line: 4, column: 5 }),
        identity,
        message: "panic unwrap syntax found".to_string(),
        ledger: None,
    };

    assert_eq!(finding.identity.container, None);
    assert_eq!(finding.identity.callee, None);
    assert_eq!(finding.identity.receiver_fingerprint, None);
    assert_eq!(
        finding.identity.normalized_snippet_hash.as_deref(),
        Some("fnv1a64:abc123"),
        "the snippet-hash anchor is preserved for exact-occurrence matching"
    );
}
