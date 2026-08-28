use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn rust_sources_do_not_spawn_cargo_or_compiler_tools() {
    let root = workspace_root();
    let mut violations = Vec::new();

    for path in rust_files(&root) {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
        let compact_text = compact_for_token_scan(&text);
        for tool in forbidden_tool_literals() {
            let forbidden = format!("Command::new({tool}");
            if compact_text.contains(&forbidden) {
                violations.push(format!(
                    "{} contains forbidden process invocation {}",
                    source_tree_path(&root, &path),
                    forbidden
                ));
            }
        }
        for token in [
            ["cargo", "_metadata::"].concat(),
            ["Metadata", "Command"].concat(),
        ] {
            if text.contains(&token) {
                violations.push(format!(
                    "{} contains forbidden Cargo metadata token `{token}`",
                    source_tree_path(&root, &path)
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "cargo-allow scans source trees directly and must not invoke Cargo/rustc tooling:\n{}",
        violations.join("\n")
    );
}

#[test]
fn forbidden_invocation_scan_catches_whitespace_variants() {
    let text = "std::process::Command::new ( \"cargo\" ).arg(\"metadata\")";

    assert!(compact_for_token_scan(text).contains("Command::new(\"cargo\""));
}

#[test]
fn forbidden_tool_literals_cover_windows_executable_names() {
    assert!(forbidden_tool_literals().contains(&"\"cargo.exe\""));
    assert!(forbidden_tool_literals().contains(&"\"rustc.exe\""));
    assert!(forbidden_tool_literals().contains(&"\"cargo-clippy.exe\""));
    assert!(forbidden_tool_literals().contains(&"\"cargo-geiger.exe\""));
    assert!(forbidden_tool_literals().contains(&"\"cargo-llvm-cov.exe\""));
    assert!(forbidden_tool_literals().contains(&"\"tarpaulin.exe\""));
}

fn forbidden_tool_literals() -> &'static [&'static str] {
    &[
        "\"cargo\"",
        "\"cargo.exe\"",
        "\"rustc\"",
        "\"rustc.exe\"",
        "\"clippy\"",
        "\"clippy.exe\"",
        "\"clippy-driver\"",
        "\"clippy-driver.exe\"",
        "\"cargo-clippy\"",
        "\"cargo-clippy.exe\"",
        "\"cargo-deny\"",
        "\"cargo-deny.exe\"",
        "\"cargo-vet\"",
        "\"cargo-vet.exe\"",
        "\"cargo-geiger\"",
        "\"cargo-geiger.exe\"",
        "\"ripr\"",
        "\"ripr.exe\"",
        "\"unsafe-review\"",
        "\"unsafe-review.exe\"",
        "\"cargo-llvm-cov\"",
        "\"cargo-llvm-cov.exe\"",
        "\"llvm-cov\"",
        "\"llvm-cov.exe\"",
        "\"grcov\"",
        "\"grcov.exe\"",
        "\"tarpaulin\"",
        "\"tarpaulin.exe\"",
        "\"cargo-tarpaulin\"",
        "\"cargo-tarpaulin.exe\"",
    ]
}

#[test]
fn cargo_dependency_files_do_not_add_cargo_metadata_dependencies() {
    let root = workspace_root();
    let mut violations = Vec::new();

    for path in cargo_dependency_files(&root) {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
        for token in [
            ["cargo", "_metadata"].concat(),
            ["cargo", "-metadata"].concat(),
        ] {
            if text.contains(&token) {
                violations.push(format!(
                    "{} contains forbidden Cargo metadata dependency token `{token}`",
                    source_tree_path(&root, &path)
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "cargo-allow must not require Cargo metadata to scan source trees:\n{}",
        violations.join("\n")
    );
}

#[test]
fn cargo_dependency_file_scan_includes_lockfile() {
    let root = workspace_root();
    let files = cargo_dependency_files(&root);

    assert!(
        files
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == "Cargo.lock")),
        "Cargo.lock should be checked for resolved Cargo metadata dependencies"
    );
}

#[test]
fn published_library_crates_document_source_tree_boundary() {
    let root = workspace_root();
    let mut violations = Vec::new();

    for path in library_crate_roots(&root) {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
        let docs = crate_level_docs(&text);
        let relative = source_tree_path(&root, &path);
        if docs.is_empty() {
            violations.push(format!("{relative} is missing crate-level Rustdoc"));
            continue;
        }
        if !docs.contains("cargo-allow") {
            violations.push(format!("{relative} crate docs should mention cargo-allow"));
        }
        if !documents_source_tree_boundary(&docs) {
            violations.push(format!(
                "{relative} crate docs should preserve source-tree/no-execution boundary"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "published library crate docs should describe their cargo-allow source-tree boundary:\n{}",
        violations.join("\n")
    );
}

fn compact_for_token_scan(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| std::panic::panic_any("cargo-allow manifest should be under crates/"))
        .to_path_buf()
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, &mut files, |path| {
        path.extension().is_some_and(|ext| ext == "rs")
    });
    files
}

fn cargo_dependency_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, &mut files, |path| {
        path.file_name()
            .is_some_and(|name| name == "Cargo.toml" || name == "Cargo.lock")
    });
    files
}

fn library_crate_roots(root: &Path) -> Vec<PathBuf> {
    let crates_dir = root.join("crates");
    let mut files = Vec::new();
    let entries = fs::read_dir(&crates_dir).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read {}: {err}", crates_dir.display()))
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            std::panic::panic_any(format!("read entry under {}: {err}", crates_dir.display()))
        });
        let lib = entry.path().join("src/lib.rs");
        if lib.is_file() {
            files.push(lib);
        }
    }
    files.sort();
    files
}

fn crate_level_docs(text: &str) -> String {
    text.lines()
        .take_while(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim())
        .collect::<Vec<_>>()
        .join("\n")
}

fn documents_source_tree_boundary(docs: &str) -> bool {
    let docs = docs.to_ascii_lowercase();
    [
        "source-tree",
        "source-syntax",
        "without invoking",
        "does not invoke",
        "does not call",
        "does not require",
        "without executing",
        "does not execute",
        "repository code",
    ]
    .iter()
    .any(|needle| docs.contains(needle))
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>, include: impl Fn(&Path) -> bool + Copy) {
    let entries = fs::read_dir(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", root.display())));
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            std::panic::panic_any(format!("read entry under {}: {err}", root.display()))
        });
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        let file_type = entry.file_type().unwrap_or_else(|err| {
            std::panic::panic_any(format!("read file type {}: {err}", path.display()))
        });
        if file_type.is_dir() {
            collect_files(&path, files, include);
        } else if file_type.is_file() && include(&path) {
            files.push(path);
        }
    }
}

fn source_tree_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
