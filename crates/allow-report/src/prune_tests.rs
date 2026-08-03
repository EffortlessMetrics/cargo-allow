use super::*;

#[test]
fn prune_json_renderer_records_mode_context_and_candidates() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale",
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "baseline_debt",
        scope: "crates/parser/src/lib.rs",
        reason: "stale baseline entry",
    }];

    let json = render_prune_json(
        &candidates,
        PruneModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
            total_entries: 0,
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(49)),
        &MutationReceipt {
            operation: "prune",
            tool_version: "0.1.10",
            repo_root: Some("H:/Code/Rust/cargo-allow"),
            config_source: Some("policy/allow.toml"),
            ledger_ids: Vec::new(),
            changed_allow_ids: vec!["allow-stale"],
            before_fingerprints: vec![Some(
                "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            )],
            after_fingerprints: vec![None],
            result: "stdout",
            next_commands: vec![
                "git diff -- policy/allow.toml".to_string(),
                "cargo-allow check --mode no-new".to_string(),
            ],
        },
        &["[[allow]]\nid = \"allow-stale\"".to_string()],
    );

    assert!(json.contains("\"schema_id\": \"cargo-allow.prune.v1\""));
    assert!(json.contains("\"command\": \"prune\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 49"));
    assert!(json.contains("\"dry_run\": true"));
    assert!(json.contains("\"write_requested\": false"));
    assert!(json.contains("\"explicit_dry_run\": true"));
    assert!(json.contains("\"written_path\": null"));
    assert!(json.contains("\"stale_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-stale\""));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(json.contains("\"removed_toml_blocks\": [\"[[allow]]\\nid = \\\"allow-stale\\\"\"]"));
    assert!(json.contains("\"mutation_receipt\""));
    let parsed = serde_json::from_str::<serde_json::Value>(&json);
    assert!(parsed.is_ok(), "prune output must remain valid JSON");
    let Some(parsed) = parsed.ok() else {
        return;
    };
    assert_eq!(
        parsed
            .pointer("/mutation_receipt/operation")
            .and_then(serde_json::Value::as_str),
        Some("prune")
    );
    assert_eq!(
        parsed
            .pointer("/mutation_receipt/changed_allow_ids/0")
            .and_then(serde_json::Value::as_str),
        Some("allow-stale")
    );
    assert_eq!(
        parsed.pointer("/mutation_receipt/after_fingerprints/0"),
        Some(&serde_json::Value::Null)
    );
}

#[test]
fn prune_json_renderer_omits_unavailable_family() -> Result<(), String> {
    let candidates = vec![PruneCandidate {
        id: "allow-stale",
        kind: "non_rust_file",
        family: None,
        owner: "release",
        classification: "release_marker",
        scope: ".changes/v0.2.0",
        reason: "retained release marker",
    }];
    let json = render_prune_json(
        &candidates,
        PruneModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
            total_entries: 1,
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(49)),
        &MutationReceipt {
            operation: "prune",
            tool_version: "0.1.10",
            repo_root: None,
            config_source: None,
            ledger_ids: Vec::new(),
            changed_allow_ids: Vec::new(),
            before_fingerprints: Vec::new(),
            after_fingerprints: Vec::new(),
            result: "stdout",
            next_commands: Vec::new(),
        },
        &[],
    );
    let value: serde_json::Value = serde_json::from_str(&json)
        .map_err(|error| format!("sparse prune report should render valid JSON: {error}"))?;
    let stale_entry = value
        .pointer("/stale_entries/0")
        .ok_or_else(|| "sparse prune report should include a stale entry".to_string())?;
    if stale_entry.get("family").is_some() {
        return Err("sparse prune report should omit family".to_string());
    }

    Ok(())
}

#[test]
fn prune_human_renderer_records_mode_and_candidates() {
    let candidates = vec![PruneCandidate {
        id: "allow|stale",
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "baseline_debt",
        scope: "crates/parser/src/lib.rs",
        reason: "old | baseline entry",
    }];

    let text = render_prune_human(
        &candidates,
        PruneModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
            total_entries: 0,
        },
    );

    assert!(text.contains("mode: dry-run"));
    assert!(text.contains("requested: --dry-run"));
    assert!(text.contains("stale entries: 1"));
    assert!(text.contains("allow\\|stale"));
    assert!(text.contains("old \\| baseline entry"));
    assert!(text.contains("No files were changed"));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
}

#[test]
fn prune_human_styles_only_the_fixed_stale_marker() {
    let candidates = vec![PruneCandidate {
        id: "allow|stale",
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "baseline_debt",
        scope: "crates/parser/src/lib.rs",
        reason: "old | baseline entry",
    }];

    let text = render_prune_human_with_context_styled(
        &candidates,
        PruneModeContext {
            explicit_dry_run: true,
            write_requested: false,
            written_path: None,
            total_entries: 1,
        },
        InventoryContext::unknown_source_syntax(),
        Style::ANSI,
    );

    assert!(text.contains("\u{1b}[33mstale\u{1b}[0m entries: 1"));
    assert!(text.contains("allow\\|stale"));
    assert_eq!(text.matches('\u{1b}').count(), 2);
}

#[test]
fn prune_human_renderer_records_inventory_context() {
    let text = render_prune_human_with_context(
        &[],
        PruneModeContext {
            explicit_dry_run: false,
            write_requested: false,
            written_path: None,
            total_entries: 0,
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(49)),
    );

    assert!(
        text.contains("Inventory: source_tree/source_syntax via git_tracked; files scanned: 49")
    );
    assert!(text.contains("Source tree root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("No stale allow entries found."));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
}
