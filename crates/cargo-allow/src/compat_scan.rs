use allow_core::{AllowConfig, CargoAllowResult, Finding, FindingKind};
use allow_inventory::{InventoryOptions, inventory};
use std::path::Path;

use crate::InventoryFacts;

pub(super) fn scan_legacy_rust_compat(
    root: &Path,
    cfg: &AllowConfig,
    include_untracked: bool,
    kind: FindingKind,
) -> CargoAllowResult<(Vec<Finding>, InventoryFacts)> {
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let inventory = inventory(root, &opts)?;
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory);
    let rust_scan = allow_rust::scan_rust_files(root, &inventory.files)?;
    let mut findings = rust_scan.findings;
    findings.retain(|finding| finding.kind == kind);
    Ok((findings, inventory_facts))
}

pub(super) fn scan_non_rust_compat(
    root: &Path,
    include_untracked: bool,
) -> CargoAllowResult<(Vec<Finding>, InventoryFacts)> {
    let opts = InventoryOptions {
        include_untracked,
        ..InventoryOptions::default()
    };
    let inventory = inventory(root, &opts)?;
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory);
    let findings = allow_files::scan_files(&inventory.files)
        .into_iter()
        .filter(|finding| finding.kind == FindingKind::NonRustFile)
        .collect::<Vec<_>>();
    Ok((findings, inventory_facts))
}

#[cfg(test)]
mod tests {
    use allow_inventory::InventorySource;
    use std::fs;

    use super::*;
    use crate::compat_test_support::migrate_fixture_dir;

    #[test]
    fn legacy_rust_compat_scan_filters_to_requested_kind_and_honors_ignored_paths() {
        let dir = migrate_fixture_dir();
        let src_dir = dir.join("src");
        let ignored_dir = dir.join("ignored");
        fs::create_dir_all(&src_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        fs::create_dir_all(&ignored_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("ignored dir: {err}")));
        fs::write(
            src_dir.join("lib.rs"),
            "fn load(value: Option<u8>, ptr: *const u8) -> u8 {\n    let fallback = value.unwrap();\n    // SAFETY: fixture observes that unsafe findings are filtered away.\n    fallback + unsafe { core::ptr::read(ptr) }\n}\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));
        fs::write(
            ignored_dir.join("ignored.rs"),
            "fn ignored(value: Option<u8>) -> u8 {\n    value.unwrap()\n}\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("ignored fixture write: {err}")));

        let mut cfg = AllowConfig::empty();
        cfg.workspace.ignored.push("ignored/**".to_string());
        let (findings, inventory_facts) =
            scan_legacy_rust_compat(&dir, &cfg, false, FindingKind::Panic)
                .unwrap_or_else(|err| std::panic::panic_any(format!("legacy scan: {err}")));

        assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
        assert_eq!(inventory_facts.files_scanned, Some(1));
        assert!(!findings.is_empty());
        assert!(
            findings
                .iter()
                .all(|finding| finding.kind == FindingKind::Panic)
        );
        assert!(findings.iter().any(|finding| {
            finding.path == Path::new("src").join("lib.rs")
                && finding.family.as_deref() == Some("unwrap")
        }));
        assert!(
            findings
                .iter()
                .all(|finding| !finding.path.to_string_lossy().contains("ignored"))
        );
    }

    #[test]
    fn non_rust_compat_scan_reports_only_non_rust_inventory_findings() {
        let dir = migrate_fixture_dir();
        let docs_dir = dir.join("docs");
        let src_dir = dir.join("src");
        fs::create_dir_all(&docs_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
        fs::create_dir_all(&src_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        fs::write(docs_dir.join("guide.md"), "# Guide\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("docs fixture write: {err}")));
        fs::write(src_dir.join("lib.rs"), "pub fn load() {}\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

        let (findings, inventory_facts) = scan_non_rust_compat(&dir, false)
            .unwrap_or_else(|err| std::panic::panic_any(format!("non-rust scan: {err}")));

        assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
        assert_eq!(inventory_facts.files_scanned, Some(2));
        assert_eq!(findings.len(), 1);
        assert!(findings.iter().all(|finding| {
            finding.kind == FindingKind::NonRustFile
                && finding.path == Path::new("docs").join("guide.md")
        }));
    }
}
