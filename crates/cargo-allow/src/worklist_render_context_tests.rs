use super::*;

#[test]
fn worklist_renderers_include_inventory_context() {
    let items = Vec::new();
    let context = WorklistContext {
        inventory: allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(46),
        ),
        filters: WorklistFilters::default(),
    };

    let json = render_worklist_json_with_context(&items, context);
    let human = render_worklist_human_with_context(&items, context);

    assert!(json.contains("\"scope\": \"source_tree\""));
    assert!(json.contains("\"scanner\": \"source_syntax\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 46"));
    assert!(json.contains("\"filters\""));
    assert!(json.contains("\"risk\": null"));
    assert!(
        human.contains("Inventory: source_tree/source_syntax via git_tracked; files scanned: 46")
    );
    assert!(human.contains("Source tree root: H:/Code/Rust/cargo-allow"));
    assert!(human.contains("Filters: none"));
}

#[test]
fn worklist_renderers_include_applied_filters() {
    let items = Vec::new();
    let context = WorklistContext {
        inventory: allow_report::InventoryContext::source_syntax("git_tracked", None, Some(46)),
        filters: WorklistFilters {
            kind: Some("unsafe"),
            family: Some("unsafe_fn"),
            item_kind: Some("baseline_debt"),
            status: Some("baseline_debt"),
            allow_id: Some("allow-0001"),
            path: Some("crates/allow-core"),
            source_package: Some("allow-core"),
            owner: Some("runtime"),
            classification: Some("baseline_debt"),
            baseline_debt: true,
            broad_scope: true,
            risk: Some("high"),
            difficulty: Some("medium"),
            missing_evidence: true,
            broken_evidence: true,
            weak_evidence: true,
        },
    };

    let json = render_worklist_json_with_context(&items, context);
    let human = render_worklist_human_with_context(&items, context);

    assert!(json.contains("\"filters\""));
    assert!(json.contains("\"kind\": \"unsafe\""));
    assert!(json.contains("\"family\": \"unsafe_fn\""));
    assert!(json.contains("\"item_kind\": \"baseline_debt\""));
    assert!(json.contains("\"status\": \"baseline_debt\""));
    assert!(json.contains("\"allow_id\": \"allow-0001\""));
    assert!(json.contains("\"path\": \"crates/allow-core\""));
    assert!(json.contains("\"source_package\": \"allow-core\""));
    assert!(json.contains("\"owner\": \"runtime\""));
    assert!(json.contains("\"classification\": \"baseline_debt\""));
    assert!(json.contains("\"baseline_debt\": true"));
    assert!(json.contains("\"broad_scope\": true"));
    assert!(json.contains("\"risk\": \"high\""));
    assert!(json.contains("\"difficulty\": \"medium\""));
    assert!(json.contains("\"missing_evidence\": true"));
    assert!(json.contains("\"broken_evidence\": true"));
    assert!(json.contains("\"weak_evidence\": true"));
    assert!(human.contains(
            "Filters: kind=unsafe, family=unsafe_fn, item_kind=baseline_debt, status=baseline_debt, allow_id=allow-0001, path=crates/allow-core, source_package=allow-core, owner=runtime, classification=baseline_debt, baseline_debt=true, broad_scope=true, risk=high, difficulty=medium, missing_evidence=true, broken_evidence=true, weak_evidence=true"
        ));
}
