use allow_policy::product_move::{render_product_move_map, validate_product_move_ledger_at};
use std::path::PathBuf;

#[test]
fn product_move_ledger_repository_inventory_is_valid() -> Result<(), String> {
    let root = repo_root();
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let (validated, diagnostics, report) = validate_product_move_ledger_at(&root, &ledger_path)
        .map_err(|error| format!("validate move ledger: {error}"))?;

    if !validated.valid {
        return Err(format!("move ledger diagnostics: {diagnostics:?}"));
    }
    assert_eq!(report.entry_count, 101);
    assert_eq!(report.target_ratified_count, 100);
    assert_eq!(report.decision_required_count, 1);
    assert_eq!(validated.ledger.controlling_issue, 2598);
    assert_eq!(validated.ledger.topology_issue, 2612);
    assert_eq!(validated.ledger.ledger_id, "CARGO-ALLOW-MOVE-LEDGER-0001");

    let map_text = std::fs::read_to_string(root.join(&validated.ledger.projection))
        .map_err(|error| format!("product move map readable: {error}"))?;
    assert_eq!(
        map_text.replace("\r\n", "\n"),
        render_product_move_map(&validated.ledger)
    );

    for entry in &validated.ledger.entry {
        assert!(map_text.contains(&format!("### `{}`", entry.id)));
        assert!(map_text.contains(&format!("- Disposition: `{}`", entry.disposition)));
        assert!(map_text.contains(&format!("- Removal: {}", entry.removal_issue_or_condition)));
        assert!(map_text.contains(&format!("- Deletion output: {}", entry.deletion_output)));
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
