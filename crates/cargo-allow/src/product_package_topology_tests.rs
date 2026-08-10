use allow_policy::product_crates::current_architecture_receipt_at;
use std::path::PathBuf;

#[test]
fn current_v2_architecture_receipt_drives_package_candidate() -> Result<(), String> {
    let root = repo_root();
    let receipt = current_architecture_receipt_at(&root)
        .map_err(|err| format!("build current V2 architecture receipt: {err}"))?;
    if receipt.workspace_package_count != 22
        || receipt.architecture_identity_count != 22
        || receipt.topology_package_count != 22
    {
        return Err(format!(
            "expected three complete 22-package denominators, got workspace={} architecture={} topology={}",
            receipt.workspace_package_count,
            receipt.architecture_identity_count,
            receipt.topology_package_count
        ));
    }
    if receipt.candidate_identity != "cargo-allow-0.2" || receipt.candidate_package_count != 13 {
        return Err(format!(
            "expected derived cargo-allow-0.2 candidate with 13 rows, got `{}` and {} rows",
            receipt.candidate_identity, receipt.candidate_package_count
        ));
    }
    if receipt.workspace_packages.len() != 22 || receipt.candidate_packages.len() != 13 {
        return Err("receipt row vectors do not match their derived denominators".to_string());
    }
    let candidate_names: Vec<_> = receipt
        .candidate_packages
        .iter()
        .map(|row| row.cargo_package_name.as_str())
        .collect();
    for name in [
        "allow-core",
        "allow-policy",
        "allow-inventory",
        "allow-files",
        "allow-rust",
        "allow-match",
        "allow-report",
        "allow-policy-legacy",
        "allow-diff",
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
        "effortless-repo-edit",
        "cargo-allow",
    ] {
        if !candidate_names.contains(&name) {
            return Err(format!("derived candidate omitted `{name}`"));
        }
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
