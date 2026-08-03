use super::*;
use std::path::Path;

#[test]
fn render_prune_stale_preview_is_dry_run_first() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        scope: "src/lib.rs".to_string(),
        reason: "The old exception is gone.".to_string(),
    }];

    let removed_toml_blocks =
        vec!["[[allow]]\nid = \"allow-stale\"\nkind = \"panic\"\nfamily = \"unwrap\"".to_string()];

    let text = render_prune_stale_result(
        &candidates,
        &removed_toml_blocks,
        true,
        false,
        None,
        5,
        test_prune_context(),
    );

    assert!(
        text.contains("Inventory: source_tree/source_syntax via git_tracked; files scanned: 49")
    );
    assert!(text.contains("Source tree root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("mode: dry-run"));
    assert!(text.contains("requested: --dry-run"));
    assert!(text.contains("stale entries: 1"));
    assert!(text.contains("allow-stale"));
    assert!(text.contains("No files were changed"));
    assert!(text.contains("TOML removal preview:"));
    assert!(text.contains("```diff"));
    assert!(text.contains("- [[allow]]"));
    assert!(text.contains("- id = \"allow-stale\""));
}

#[test]
fn render_prune_stale_json_records_context_and_candidates() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "baseline_debt".to_string(),
        scope: "crates/parser/src/lib.rs".to_string(),
        reason: "old baseline entry".to_string(),
    }];

    let json = render_prune_stale_json(
        &candidates,
        true,
        false,
        None,
        5,
        &["[[allow]]\nid = \"allow-stale\"".to_string()],
        PruneContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(49),
            ),
            mutation_receipt: allow_report::MutationReceipt {
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
                next_commands: vec!["cargo-allow check --mode no-new".to_string()],
            },
        },
    );

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::PRUNE_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"prune\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
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
fn render_prune_stale_result_reports_written_policy() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        scope: "src/lib.rs".to_string(),
        reason: "The old exception is gone.".to_string(),
    }];

    let text = render_prune_stale_result(
        &candidates,
        &[],
        false,
        true,
        Some(Path::new("policy/allow.toml")),
        3,
        test_prune_context(),
    );

    assert!(text.contains("mode: write"));
    assert!(text.contains("Removed 1 stale entry from `policy/allow.toml`. 2 entries remain."));
    assert!(!text.contains("No files were changed"));
}

#[test]
fn render_prune_stale_result_reports_write_mode_with_no_candidates() {
    let text = render_prune_stale_result(&[], &[], false, true, None, 0, test_prune_context());

    assert!(text.contains("mode: write"));
    assert!(text.contains("No stale allow entries found."));
    assert!(!text.contains("TOML removal preview:"));
}

#[test]
fn test_prune_context_return_value_observer() {
    let context = test_prune_context();
    assert_eq!(context.inventory.scope, "source_tree");
    assert_eq!(context.inventory.scanner, "source_syntax");
    assert_eq!(context.inventory.source, "git_tracked");
    assert_eq!(context.inventory.root, Some("H:/Code/Rust/cargo-allow"));
    assert_eq!(context.inventory.files_scanned, Some(49));
}

fn test_prune_context() -> PruneContext<'static> {
    PruneContext {
        inventory: allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(49),
        ),
        mutation_receipt: allow_report::MutationReceipt {
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
            next_commands: vec!["cargo-allow check --mode no-new".to_string()],
        },
    }
}
