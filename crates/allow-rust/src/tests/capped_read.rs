use allow_core::{SOURCE_FILE_READ_MAX_BYTES, read_text_file_capped};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{RustFileScanOutcome, ScanCache, scan_rust_files, scan_rust_files_cached};

fn temp_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-rust-capped-{label}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

#[test]
fn scan_rust_files_skips_oversized_sources_without_aborting() {
    let root = temp_root("oversized-skip");
    let src = root.join("src");
    fs::create_dir_all(&src)
        .unwrap_or_else(|err| std::panic::panic_any(format!("mkdir src: {err}")));
    fs::write(src.join("ok.rs"), "fn ok() { let _ = Some(1).unwrap(); }\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write ok.rs: {err}")));

    let oversized_path = src.join("huge.rs");
    // One byte over the documented production ceiling (#1916).
    let oversized_len = (SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1);
    fs::write(&oversized_path, vec![b'a'; oversized_len])
        .unwrap_or_else(|err| std::panic::panic_any(format!("write huge.rs: {err}")));

    let err = read_text_file_capped(&oversized_path).unwrap_err();
    assert!(
        err.is_oversized(),
        "oversized source must fail closed: {err}"
    );

    let mixed = scan_rust_files(
        &root,
        &[PathBuf::from("src/ok.rs"), PathBuf::from("src/huge.rs")],
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("scan mixed: {err}")));
    assert!(
        !mixed.findings.is_empty(),
        "oversized sibling must not abort the scan"
    );
    assert!(
        mixed.findings.iter().any(|f| f.path.ends_with("ok.rs")),
        "findings should come from the readable sibling"
    );
    assert!(
        mixed.findings.iter().all(|f| !f.path.ends_with("huge.rs")),
        "oversized file must not contribute findings"
    );
    assert_eq!(
        mixed.files_skipped, 1,
        "oversized file must be counted as skipped"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cached_scan_exposes_typed_status_for_each_rust_file() -> Result<(), String> {
    let root = temp_root("cached-status");
    let src = root.join("src");
    fs::create_dir_all(&src).map_err(|err| format!("mkdir src: {err}"))?;
    fs::write(src.join("ok.rs"), "fn ok() { let _ = Some(1).unwrap(); }\n")
        .map_err(|err| format!("write ok.rs: {err}"))?;
    let oversized_path = src.join("huge.rs");
    let oversized_len = (SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1);
    fs::write(&oversized_path, vec![b'a'; oversized_len])
        .map_err(|err| format!("write huge.rs: {err}"))?;

    let mut cache = ScanCache::new();
    let result = scan_rust_files_cached(
        &root,
        &[PathBuf::from("src/ok.rs"), PathBuf::from("src/huge.rs")],
        &mut cache,
    )
    .map_err(|err| format!("cached scan: {err}"))?;

    if result.status_for(std::path::Path::new("src/ok.rs")) != Some(&RustFileScanOutcome::Scanned)
        || !matches!(
            result.status_for(std::path::Path::new("src/huge.rs")),
            Some(RustFileScanOutcome::Skipped { .. })
        )
    {
        return Err("cached scan did not expose typed per-file statuses".to_string());
    }
    let _ = fs::remove_dir_all(&root);
    Ok(())
}
