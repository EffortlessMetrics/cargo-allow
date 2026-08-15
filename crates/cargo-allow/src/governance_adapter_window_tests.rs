//! Governance compatibility-adapter window guards (#2942 step 6 / #3542).
//!
//! allow-policy keeps V1 governance parsing/validation as a bounded parity
//! adapter while the intent-side authority (#3327-#3329 DTOs + engine
//! operations, #3540 receipt) becomes canonical. These tests pin the
//! window: every remaining allow-policy governance export carries a
//! MOVE-GOV ledger row with a bounded duplicate-authority class, a parity
//! case, and a deletion condition; and the intent-model compat parsing
//! reads the same authority rows (no duplicate current authority).

use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn governance_compat_parsing_matches_allow_policy_authority() -> Result<(), String> {
    let root = repo_root();
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(root.join(rel)).map_err(|err| format!("read {rel}: {err}"))
    };

    // Crate identities: V2 manifest (allow-policy) vs governance_v2 compat
    // (intent-model) must agree row for row.
    let identity_text = read("policy/product-crates-v2.toml")?;
    let v2_manifest =
        allow_policy::product_crates::v2::parse_architecture_manifest_v2(&identity_text)
            .map_err(|err| format!("allow-policy V2 parse: {err}"))?;
    let compat_identities = intent_model::parse_crate_identities_v1(&identity_text)?;
    let v2_rows: BTreeSet<(String, String, String)> = v2_manifest
        .crate_identity
        .iter()
        .map(|row| {
            (
                row.logical_id.clone(),
                row.cargo_package_name.clone(),
                row.rust_library_name.clone(),
            )
        })
        .collect();
    let compat_rows: BTreeSet<(String, String, String)> = compat_identities
        .iter()
        .map(|identity| {
            (
                identity.logical_id.clone(),
                identity.cargo_package_name.clone(),
                identity.rust_library_name.clone(),
            )
        })
        .collect();
    if v2_rows != compat_rows {
        return Err(format!(
            "crate identity parity drift between allow-policy V2 and intent-model compat: only-in-allow-policy {:?}, only-in-intent-model {:?}",
            v2_rows.difference(&compat_rows).count(),
            compat_rows.difference(&v2_rows).count(),
        ));
    }

    // Dependency law: V1 manifest (allow-policy) vs governance_v2 law
    // (intent-model) must agree on forbidden and required edges.
    let law_text = read("policy/product-crates.toml")?;
    let v1_manifest = allow_policy::product_crates::parse_architecture_manifest(&law_text)
        .map_err(|err| format!("allow-policy V1 parse: {err}"))?;
    let (compat_forbidden, compat_required) = intent_model::parse_dependency_law_v1(&law_text)?;
    let v1_forbidden: BTreeSet<(String, String)> = v1_manifest
        .forbidden_crate_dependency
        .iter()
        .map(|edge| (edge.from.clone(), edge.to.clone()))
        .collect();
    let compat_forbidden_set: BTreeSet<(String, String)> = compat_forbidden
        .iter()
        .map(|edge| (edge.from_logical_id.clone(), edge.to_logical_id.clone()))
        .collect();
    if v1_forbidden != compat_forbidden_set {
        return Err("forbidden-edge parity drift between allow-policy and intent-model".into());
    }
    let v1_required: BTreeSet<(String, String)> = v1_manifest
        .required_crate_dependency
        .iter()
        .map(|edge| (edge.from.clone(), edge.to.clone()))
        .collect();
    let compat_required_set: BTreeSet<(String, String)> = compat_required
        .iter()
        .map(|edge| (edge.from_logical_id.clone(), edge.to_logical_id.clone()))
        .collect();
    if v1_required != compat_required_set {
        return Err("required-edge parity drift between allow-policy and intent-model".into());
    }

    // Shims: registry ids and statuses must agree.
    let shim_text = read("policy/extraction-shims.toml")?;
    let shim_registry = allow_policy::extraction_shims::parse_extraction_shim_registry(&shim_text)
        .map_err(|err| format!("allow-policy shim parse: {err}"))?;
    let (compat_shims, compat_expiries) = intent_model::parse_shim_references_v1(&shim_text)?;
    let v1_shims: BTreeSet<(String, &'static str)> = shim_registry
        .shim
        .iter()
        .map(|shim| (shim.id.clone(), shim.status.as_str()))
        .collect();
    let compat_shim_set: BTreeSet<(String, &'static str)> = compat_shims
        .iter()
        .map(|shim| (shim.shim_id.clone(), shim.status.as_str()))
        .collect();
    if v1_shims != compat_shim_set {
        return Err("shim registry parity drift between allow-policy and intent-model".into());
    }
    let v1_expiry_ids: BTreeSet<&str> = shim_registry
        .shim
        .iter()
        .map(|shim| shim.id.as_str())
        .collect();
    let compat_expiry_ids: BTreeSet<&str> = compat_expiries
        .iter()
        .map(|expiry| expiry.component_id.as_str())
        .collect();
    if v1_expiry_ids != compat_expiry_ids {
        return Err("shim expiry parity drift between allow-policy and intent-model".into());
    }

    // Parity case ids must agree.
    let parity_text = read("policy/extraction-parity.toml")?;
    let parity_registry =
        allow_policy::extraction_parity::parse_extraction_parity_registry(&parity_text)
            .map_err(|err| format!("allow-policy parity parse: {err}"))?;
    let compat_parity = intent_model::parse_parity_references_v1(&parity_text)?;
    let v1_case_ids: BTreeSet<&str> = parity_registry
        .case
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    let compat_case_ids: BTreeSet<&str> = compat_parity
        .iter()
        .map(|case| case.case_id.as_str())
        .collect();
    if v1_case_ids != compat_case_ids {
        return Err("parity case parity drift between allow-policy and intent-model".into());
    }

    // Move ledger entry ids must agree.
    let ledger_text = read("policy/product-move-ledger.toml")?;
    let ledger = allow_policy::product_move::parse_product_move_ledger(&ledger_text)
        .map_err(|err| format!("allow-policy ledger parse: {err}"))?;
    let compat_moves = intent_model::parse_move_references_v1(&ledger_text)?;
    let v1_entry_ids: BTreeSet<&str> = ledger.entry.iter().map(|e| e.id.as_str()).collect();
    let compat_entry_ids: BTreeSet<&str> =
        compat_moves.iter().map(|r| r.entry_id.as_str()).collect();
    if v1_entry_ids != compat_entry_ids {
        return Err("move ledger parity drift between allow-policy and intent-model".into());
    }

    Ok(())
}

#[test]
fn allow_policy_governance_window_is_explicit() -> Result<(), String> {
    let root = repo_root();
    let ledger_text = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("read ledger: {err}"))?;
    let ledger = allow_policy::product_move::parse_product_move_ledger(&ledger_text)
        .map_err(|err| format!("parse ledger: {err}"))?;

    let parity_text = std::fs::read_to_string(root.join("policy/extraction-parity.toml"))
        .map_err(|err| format!("read parity registry: {err}"))?;
    let parity_registry =
        allow_policy::extraction_parity::parse_extraction_parity_registry(&parity_text)
            .map_err(|err| format!("parse parity registry: {err}"))?;
    let known_case_ids: BTreeSet<&str> = parity_registry
        .case
        .iter()
        .map(|case| case.id.as_str())
        .collect();

    let gov_entries: Vec<_> = ledger
        .entry
        .iter()
        .filter(|entry| entry.id.starts_with("MOVE-GOV-"))
        .collect();
    if gov_entries.len() != 6 {
        return Err(format!(
            "expected 6 MOVE-GOV window rows, got {}",
            gov_entries.len()
        ));
    }

    // Every window row is bounded, removable, parity-linked, and inside the
    // cutover window.
    let mut covered_paths: BTreeSet<String> = BTreeSet::new();
    for entry in &gov_entries {
        if entry.duplicate_authority_class != "BoundedParityOnly" {
            return Err(format!(
                "`{}` must be BoundedParityOnly, got `{}`",
                entry.id, entry.duplicate_authority_class
            ));
        }
        if entry.removal_issue_or_condition.trim().is_empty() {
            return Err(format!("`{}` needs a deletion condition", entry.id));
        }
        let closed = entry.old_path_reachability_disposition == "Deleted";
        if closed {
            // Deleted rows record the completed cutover.
            if entry.status != "CutoverCurrent" {
                return Err(format!(
                    "`{}` has a Deleted old path but does not record CutoverCurrent",
                    entry.id
                ));
            }
        } else if entry.status != "CutoverOutstanding" {
            return Err(format!(
                "`{}` must record CutoverOutstanding while the adapter window is open",
                entry.id
            ));
        }
        if entry.parity_case_ids.is_empty() {
            return Err(format!("`{}` needs a parity case", entry.id));
        }
        for case_id in &entry.parity_case_ids {
            if !known_case_ids.contains(case_id.as_str()) {
                return Err(format!(
                    "`{}` references unknown parity case `{case_id}`",
                    entry.id
                ));
            }
        }
        if entry.target_crate != "intent-model" && entry.target_crate != "intent-engine" {
            return Err(format!(
                "`{}` must target the cargo-intent family, got `{}`",
                entry.id, entry.target_crate
            ));
        }
        for path in &entry.current_paths {
            if !root.join(path).is_file() {
                return Err(format!("`{}` path missing: {path}", entry.id));
            }
            covered_paths.insert(path.clone());
        }
    }

    // The canonical files the intent-side compat parsing supersedes must be
    // inside the MOVE-GOV window specifically (not merely registered).
    for canonical in [
        "crates/allow-policy/src/product_crates/config.rs",
        "crates/allow-policy/src/product_move/config.rs",
        "crates/allow-policy/src/extraction_parity/config.rs",
        "crates/allow-policy/src/extraction_shims/config.rs",
    ] {
        if !covered_paths.contains(canonical) {
            return Err(format!(
                "canonical governance file `{canonical}` must carry a MOVE-GOV window row"
            ));
        }
    }

    // Every allow-policy governance module file is registered somewhere in
    // the ledger: canonical parsers/validators carry MOVE-GOV window rows;
    // migration-control, receipt, and test surfaces carry their own entries
    // (e.g. REMAIN-MOVE-LEDGER-VALIDATOR owns the product_move directory).
    // A registered directory covers the files beneath it.
    let registered: Vec<&str> = ledger
        .entry
        .iter()
        .flat_map(|entry| entry.current_paths.iter().map(String::as_str))
        .collect();
    let is_registered = |rel: &str| -> bool {
        registered
            .iter()
            .any(|candidate| *candidate == rel || rel.starts_with(&format!("{candidate}/")))
    };
    let module_dirs = [
        "crates/allow-policy/src/product_crates",
        "crates/allow-policy/src/product_move",
        "crates/allow-policy/src/extraction_parity",
        "crates/allow-policy/src/extraction_shims",
    ];
    let mut uncovered: Vec<String> = Vec::new();
    for dir in module_dirs {
        for entry in std::fs::read_dir(root.join(dir))
            .map_err(|err| format!("read dir {dir}: {err}"))?
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .map_err(|err| err.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let file = rel.rsplit('/').next().unwrap_or_default();
            // Structural re-export surfaces and test fixtures are not
            // canonical authority files.
            if file == "mod.rs" || file == "tests.rs" || file.ends_with("_tests.rs") {
                continue;
            }
            if !is_registered(&rel) {
                uncovered.push(rel);
            }
        }
    }
    if !uncovered.is_empty() {
        return Err(format!(
            "allow-policy governance files with no ledger disposition: {uncovered:?}"
        ));
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
