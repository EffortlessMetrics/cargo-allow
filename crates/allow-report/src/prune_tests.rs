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
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(49)),
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
        },
    );

    assert!(text.contains("mode: dry-run"));
    assert!(text.contains("requested: --dry-run"));
    assert!(text.contains("stale entries: 1"));
    assert!(text.contains("allow\\|stale"));
    assert!(text.contains("old \\| baseline entry"));
    assert!(text.contains("No files were changed"));
}
