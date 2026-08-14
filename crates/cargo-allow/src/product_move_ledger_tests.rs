use allow_policy::extraction_parity::parse_extraction_parity_registry_at;
use allow_policy::product_move::{
    parse_product_move_ledger_at, render_product_move_map, validate_product_move_ledger_at,
};
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
    assert_eq!(report.entry_count, 108);
    assert_eq!(report.target_ratified_count, 98);
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

#[test]
fn extraction_parity_cases_bind_to_their_canonical_cutover_stage() -> Result<(), String> {
    let root = repo_root();
    let registry_path = root.join("policy/extraction-parity.toml");
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let registry_text = std::fs::read_to_string(&registry_path)
        .map_err(|error| format!("read extraction parity registry: {error}"))?;
    let ledger_text = std::fs::read_to_string(&ledger_path)
        .map_err(|error| format!("read product move ledger: {error}"))?;
    let registry = parse_extraction_parity_registry_at(Some(&registry_path), &registry_text)
        .map_err(|error| format!("parse extraction parity registry: {error}"))?;
    let ledger = parse_product_move_ledger_at(Some(&ledger_path), &ledger_text)
        .map_err(|error| format!("parse product move ledger: {error}"))?;

    let mut snapshot_count = 0;
    let mut edit_count = 0;
    for case in registry
        .case
        .iter()
        .filter(|case| matches!(case.stage.as_str(), "RepoSnapshot" | "RepoEdit"))
    {
        let entry = ledger
            .entry
            .iter()
            .find(|entry| entry.id == case.move_ledger_entry)
            .ok_or_else(|| {
                format!(
                    "parity case `{}` references missing move entry `{}`",
                    case.id, case.move_ledger_entry
                )
            })?;
        if entry.cutover_stage != case.stage.as_str() {
            return Err(format!(
                "parity case `{}` stage `{}` disagrees with ledger stage `{}` for `{}`",
                case.id,
                case.stage.as_str(),
                entry.cutover_stage,
                entry.id
            ));
        }
        let expected_receipt = match case.stage.as_str() {
            "RepoSnapshot" => {
                snapshot_count += 1;
                "CUTOVER-REPO-SNAPSHOT"
            }
            "RepoEdit" => {
                edit_count += 1;
                "CUTOVER-REPO-EDIT"
            }
            _ => return Err(format!("unexpected parity stage `{}`", case.stage.as_str())),
        };
        if entry.expected_cutover_receipt != expected_receipt {
            return Err(format!(
                "ledger entry `{}` has receipt `{}`; expected `{expected_receipt}`",
                entry.id, entry.expected_cutover_receipt
            ));
        }
    }
    if snapshot_count != 2 || edit_count != 11 {
        return Err(format!(
            "unexpected live parity stage counts: RepoSnapshot={snapshot_count}, RepoEdit={edit_count}"
        ));
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
