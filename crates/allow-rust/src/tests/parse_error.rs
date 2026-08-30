use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{scan_rust_files, scan_rust_source_with_completeness};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn temp_root(label: &str) -> TestResult<PathBuf> {
    temp_root_under(&std::env::temp_dir(), label)
}

fn temp_root_under(base: &Path, label: &str) -> TestResult<PathBuf> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = base.join(format!(
        "cargo-allow-rust-parse-error-{label}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;
    Ok(root)
}

#[test]
fn scan_rust_source_reports_tree_sitter_parse_errors() {
    let source = "fn broken( {\n    let _ = Some(1).unwrap();\n}\n";
    let scan = scan_rust_source_with_completeness("src/broken.rs", source);
    assert!(
        scan.has_parse_error,
        "syntax-error source must not fail open silently"
    );
}

#[test]
fn scan_rust_files_counts_parse_errors_without_aborting() -> TestResult {
    let root = temp_root("workspace-parse-error")?;
    let src = root.join("src");
    fs::create_dir_all(&src)?;
    fs::write(src.join("ok.rs"), "fn ok() { let _ = Some(1).unwrap(); }\n")?;
    fs::write(
        src.join("broken.rs"),
        "fn broken( {\n    let _ = Some(1).unwrap();\n}\n",
    )?;

    let mixed = scan_rust_files(
        &root,
        &[PathBuf::from("src/ok.rs"), PathBuf::from("src/broken.rs")],
    )?;

    assert!(
        mixed.findings.iter().any(|f| f.path.ends_with("ok.rs")),
        "parse-error sibling must not abort the scan"
    );
    assert_eq!(
        mixed.files_with_parse_errors, 1,
        "broken file must be counted as a partial parse"
    );
    assert!(
        mixed.has_parse_errors(),
        "scan result must expose parse-error completeness"
    );
    assert_eq!(mixed.files_skipped, 0, "readable files must not be skipped");

    fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn temp_root_preserves_typed_io_errors() -> TestResult {
    let root = temp_root("typed-error-control")?;
    let blocker = root.join("not-a-directory");
    fs::write(&blocker, b"file")?;

    let error = match temp_root_under(&blocker, "child") {
        Ok(path) => {
            return Err(io::Error::other(format!(
                "temp root unexpectedly succeeded at {}",
                path.display()
            ))
            .into());
        }
        Err(error) => error,
    };
    let Some(io_error) = error.downcast_ref::<io::Error>() else {
        return Err(io::Error::other(format!(
            "expected io::Error, received {error:?}"
        ))
        .into());
    };

    assert_eq!(io_error.kind(), io::ErrorKind::NotADirectory);
    fs::remove_dir_all(&root)?;
    Ok(())
}
