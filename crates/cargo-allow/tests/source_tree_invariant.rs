use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn rust_sources_do_not_spawn_cargo_or_compiler_tools() {
    let root = workspace_root();
    let mut violations = Vec::new();

    for path in rust_files(&root.join("crates")) {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
        for (call, tool) in [
            ("Command::new(", "\"cargo\""),
            ("Command::new(", "\"rustc\""),
            ("Command::new(", "\"clippy\""),
            ("Command::new(", "\"cargo-clippy\""),
            ("Command::new(", "\"cargo-deny\""),
            ("Command::new(", "\"cargo-vet\""),
            ("Command::new(", "\"ripr\""),
            ("Command::new(", "\"unsafe-review\""),
        ] {
            if text.contains(call) && text.contains(&format!("{call}{tool}")) {
                violations.push(format!(
                    "{} contains forbidden process invocation {call}{tool}",
                    source_tree_path(&root, &path)
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
fn manifests_do_not_add_cargo_metadata_dependencies() {
    let root = workspace_root();
    let mut violations = Vec::new();

    for path in manifest_files(&root) {
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

fn manifest_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, &mut files, |path| {
        path.file_name().is_some_and(|name| name == "Cargo.toml")
    });
    files
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
