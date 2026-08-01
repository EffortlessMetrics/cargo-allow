use super::*;
use allow_core::{CargoAllowError, CargoAllowErrorKind};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn git_cat_file_batch_parser_preserves_blob_bytes_and_boundaries() {
    let first = "a".repeat(40);
    let second = "b".repeat(40);
    let output = format!("{first} blob 17\nfirst line\nsecond\n{second} blob 0\n\n");

    let blobs = revision_git::parse_git_cat_file_batch_for_test(output.as_bytes())
        .unwrap_or_else(|err| std::panic::panic_any(format!("batch parse: {err}")));

    assert_eq!(
        blobs.get(&first).map(String::as_str),
        Some("first line\nsecond")
    );
    assert_eq!(blobs.get(&second).map(String::as_str), Some(""));
}

#[test]
fn git_cat_file_batch_parser_rejects_missing_and_truncated_blob_records() {
    let oid = "c".repeat(40);
    for output in [format!("{oid} missing\n"), format!("{oid} blob 4\nno\n")] {
        let err = revision_git::parse_git_cat_file_batch_for_test(output.as_bytes())
            .err()
            .unwrap_or_else(|| std::panic::panic_any("malformed batch output should fail"));
        assert_eq!(err.kind(), CargoAllowErrorKind::Inventory);
    }
}

#[test]
fn git_cat_file_batch_mapping_rejects_missing_requested_blob() {
    let path = PathBuf::from("src/lib.rs");
    let mut paths = BTreeMap::new();
    paths.insert(path.clone(), "A".repeat(40));
    let err = revision_git::map_blob_texts_by_path_for_test(paths, BTreeMap::new())
        .err()
        .unwrap_or_else(|| std::panic::panic_any("missing blob mapping should fail"));

    assert_eq!(err.kind(), CargoAllowErrorKind::Inventory);
    assert!(err.to_string().contains("did not return blob"));
}

#[test]
fn git_tree_revision_parser_skips_symlinks_and_preserves_newlines() {
    // #1826: symlinks (mode 120000) and gitlinks/submodules (mode 160000)
    // are excluded because their blob content is a target path or commit
    // reference, not parseable source. Only regular files (100644) and
    // executables (100755) carry source content. Paths with embedded
    // newlines are preserved (the -z / NUL-delimited output from #1918
    // ensures the newline inside `line\nbreak.rs` is one path, not two).
    let output = b"100644 blob abc123\tsrc/lib.rs\0\
120000 blob def456\tsrc/link.rs\0\
160000 commit 123456\tvendor/submodule\0\
100644 blob fedcba\tfixtures/line\nbreak.rs\0";

    let files = revision_git::parse_git_ls_tree_z(output);

    assert_eq!(
        files,
        vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("fixtures/line\nbreak.rs")
        ]
    );
}

#[test]
fn git_tree_revision_parser_preserves_executable_modes() {
    let output = b"100644 blob abc123\tREADME.md\0\
100755 blob def456\tscripts/package-proof.sh\0\
120000 blob fedcba\tscripts/link.sh\0";

    let files = revision_git::parse_git_ls_tree_file_entries_z(output);

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].mode, "100644");
    assert_eq!(files[0].path, PathBuf::from("README.md"));
    assert_eq!(files[1].mode, "100755");
    assert_eq!(files[1].path, PathBuf::from("scripts/package-proof.sh"));
}

#[test]
fn git_tree_revision_parser_filters_malformed_and_non_file_records() {
    let output = b"record without separator\0\
\tpath-without-mode\0\
040000 tree abc123\tsrc\0\
100644 blob abc123\tvalid.txt\0";

    let files = revision_git::parse_git_ls_tree_file_entries_z(output);

    assert_eq!(
        files,
        vec![revision_git::GitTreeFile {
            mode: "100644".to_string(),
            object_oid: "abc123".to_string(),
            path: PathBuf::from("valid.txt"),
            raw_path: b"valid.txt".to_vec(),
        },]
    );
}

#[test]
fn parse_git_ls_tree_record_call_presence_observer() {
    assert_eq!(
        revision_git::parse_git_ls_tree_record_for_test(b"record without separator"),
        None
    );
    assert_eq!(
        revision_git::parse_git_ls_tree_record_for_test(b"\tpath-without-mode"),
        None
    );
    assert_eq!(
        revision_git::parse_git_ls_tree_record_for_test(b"040000 tree abc123\tsrc"),
        None
    );

    let non_utf8 = b"100644 blob abc123\tinvalid-\xff.txt";
    #[cfg(unix)]
    {
        let entry = revision_git::parse_git_ls_tree_record_for_test(non_utf8)
            .unwrap_or_else(|| std::panic::panic_any("unix should preserve non-UTF-8 path bytes"));
        assert_eq!(entry.mode, "100644");
        assert_eq!(entry.raw_path, b"invalid-\xff.txt");
        assert_eq!(entry.object_oid, "abc123");
    }
    #[cfg(windows)]
    {
        assert_eq!(
            revision_git::parse_git_ls_tree_record_for_test(non_utf8),
            None,
            "Windows must not invent a lossy PathBuf for non-UTF-8 Git paths"
        );
        assert_eq!(
            revision_git::parse_git_tree_record_outcome_for_test(non_utf8),
            Some(("unsupported", b"invalid-\xff.txt".to_vec()))
        );
    }
}

#[test]
fn parse_git_ls_tree_record_return_value_discriminator() {
    let entry =
        revision_git::parse_git_ls_tree_record_for_test(b"100755 blob def456\tscripts/run.sh")
            .unwrap_or_else(|| std::panic::panic_any("executable file record should parse"));

    assert_eq!(entry.mode, "100755");
    assert_eq!(entry.path, PathBuf::from("scripts/run.sh"));
    assert_eq!(entry.object_oid, "def456");
    assert_eq!(entry.raw_path, b"scripts/run.sh");
}

#[test]
fn parse_git_tree_record_outcome_distinguishes_entry_and_malformed() {
    assert_eq!(
        revision_git::parse_git_tree_record_outcome_for_test(b"100644 blob abc123\tnotes/ok.txt"),
        Some(("entry", b"notes/ok.txt".to_vec()))
    );
    assert_eq!(
        revision_git::parse_git_tree_record_outcome_for_test(b"record without separator"),
        None
    );
}

#[test]
fn parse_git_ls_tree_record_preserves_colon_and_literal_backslash_path_bytes() {
    let colon = revision_git::parse_git_ls_tree_record_for_test(
        b"100644 blob abc123\tnotes/file:with:colons.txt",
    )
    .unwrap_or_else(|| std::panic::panic_any("colon path should parse"));
    assert_eq!(colon.raw_path, b"notes/file:with:colons.txt");
    assert_eq!(colon.path, PathBuf::from("notes/file:with:colons.txt"));

    let backslash_record = b"100644 blob def456\tweird\\name.txt";
    #[cfg(unix)]
    {
        let backslash = revision_git::parse_git_ls_tree_record_for_test(backslash_record)
            .unwrap_or_else(|| std::panic::panic_any("literal backslash path should parse"));
        assert_eq!(backslash.raw_path, b"weird\\name.txt");
        assert_eq!(backslash.path, PathBuf::from("weird\\name.txt"));
    }
    #[cfg(windows)]
    {
        assert_eq!(
            revision_git::parse_git_ls_tree_record_for_test(backslash_record),
            None,
            "Windows must not reinterpret literal backslash Git path bytes as separators"
        );
        assert_eq!(
            revision_git::parse_git_tree_record_outcome_for_test(backslash_record),
            Some(("unsupported", b"weird\\name.txt".to_vec()))
        );
    }
}

#[test]
fn parse_changed_files_z_preserves_embedded_newline_path() {
    // #1918: NUL-delimited git diff output must preserve paths with embedded
    // newlines (legal on many filesystems, storable in git). The old
    // newline-split parsing would corrupt such a path into two entries.
    let stdout = b"src/lib.rs\0weird\nname.rs\0README.md\0";
    let files = revision_git::parse_changed_files_z(stdout);
    assert_eq!(files.len(), 3, "three NUL-delimited paths");
    assert_eq!(files[0], PathBuf::from("src/lib.rs"));
    assert_eq!(
        files[1],
        PathBuf::from("weird\nname.rs"),
        "embedded-newline path must be preserved as one entry"
    );
    assert_eq!(files[2], PathBuf::from("README.md"));
}

#[test]
fn revision_git_commands_report_changed_tracked_and_missing_files() {
    let repo = TempGitRepo::new("revision-git-commands");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    repo.write("README.md", "initial readme\n");
    repo.write("src/lib.rs", "pub fn version() -> u8 { 1 }\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "initial"]);
    let base = repo.git_stdout(&["rev-parse", "HEAD"]);

    repo.write("README.md", "updated readme\n");
    repo.write("src/new.rs", "pub const NEW: bool = true;\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "update"]);

    let changed = revision_git::changed_files(repo.path(), &base, Some("HEAD"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("changed files read: {err}")));
    assert_eq!(
        changed,
        vec![PathBuf::from("README.md"), PathBuf::from("src/new.rs")]
    );

    let tracked = revision_git::git_tracked_files_at_revision(repo.path(), &base)
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked files read: {err}")));
    assert_eq!(
        tracked,
        vec![PathBuf::from("README.md"), PathBuf::from("src/lib.rs")]
    );

    let readme = revision_git::read_file_at_revision(repo.path(), &base, "README.md")
        .unwrap_or_else(|err| std::panic::panic_any(format!("readme read: {err}")));
    assert_eq!(readme.as_deref(), Some("initial readme\n"));

    // On Windows, host paths may use `\`; source_tree_path_bytes maps separators
    // to Git's `/` form. On Unix, `\` is a literal filename byte, so use `/`.
    #[cfg(windows)]
    let lib_path = PathBuf::from("src\\lib.rs");
    #[cfg(not(windows))]
    let lib_path = PathBuf::from("src/lib.rs");
    let lib = revision_git::read_file_at_revision(repo.path(), &base, lib_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("lib read: {err}")));
    assert_eq!(lib.as_deref(), Some("pub fn version() -> u8 { 1 }\n"));

    let missing = revision_git::read_file_at_revision(repo.path(), &base, "src/new.rs")
        .unwrap_or_else(|err| std::panic::panic_any(format!("missing file handled: {err}")));
    assert_eq!(missing, None);
}

#[test]
fn read_file_at_revision_treats_leading_dash_space_and_unicode_paths_literally() {
    let repo = TempGitRepo::new("revision-git-literal-paths");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    repo.write("-leading.txt", "leading dash\n");
    repo.write("notes/hello world.txt", "space\n");
    repo.write("notes/naïve.txt", "unicode\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "literal paths"]);

    for (path, expected) in [
        ("-leading.txt", "leading dash\n"),
        ("notes/hello world.txt", "space\n"),
        ("notes/naïve.txt", "unicode\n"),
    ] {
        let value = revision_git::read_file_at_revision(repo.path(), "HEAD", path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read {path}: {err}")));
        assert_eq!(value.as_deref(), Some(expected), "{path}");
    }
}

#[test]
fn option_like_revision_is_rejected_before_git_can_create_output() {
    let repo = TempGitRepo::new("revision-git-option-like");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    repo.write("README.md", "fixture\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "fixture"]);

    let side_effect = repo.path().join("git-option-side-effect.txt");
    let revision = format!("--output={}", side_effect.display());
    let err = revision_git::changed_files(repo.path(), &revision, Some("HEAD"))
        .err()
        .unwrap_or_else(|| std::panic::panic_any("option-like revision should fail"));

    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidConfig);
    assert_diagnostic_code(&err, "invalid_revision_input");
    assert!(
        !side_effect.exists(),
        "option-like revision must not create {}",
        side_effect.display()
    );
}

#[test]
fn revision_git_commands_report_unresolved_revision_without_clean_fallback() {
    let repo = TempGitRepo::new("revision-git-failures");
    repo.git(&["init"]);

    let changed_err = revision_git::changed_files(repo.path(), "missing-revision", None)
        .err()
        .unwrap_or_else(|| std::panic::panic_any("missing diff base should fail"));
    assert_revision_not_found(&changed_err);

    let tree_err = revision_git::git_tree_files_at_revision(repo.path(), "missing-revision")
        .err()
        .unwrap_or_else(|| std::panic::panic_any("missing tree revision should fail"));
    assert_revision_not_found(&tree_err);

    let read_err =
        revision_git::read_file_at_revision(repo.path(), "missing-revision", "README.md")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("missing revision read should fail"));
    assert_revision_not_found(&read_err);
}

fn assert_revision_not_found(err: &CargoAllowError) {
    assert_eq!(err.kind(), CargoAllowErrorKind::Inventory);
    assert!(
        err.to_string()
            .contains("could not be resolved to a commit"),
        "unexpected error: {err}"
    );
    assert_diagnostic_code(err, "revision_not_found");
}

fn assert_diagnostic_code(err: &CargoAllowError, code: &str) {
    assert!(
        err.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "expected diagnostic code `{code}` in {:?}",
        err.diagnostics()
    );
}

#[cfg(unix)]
#[test]
fn read_file_at_revision_uses_blob_oid_for_colon_paths() {
    let repo = TempGitRepo::new("revision-git-colon-blob");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    // Create the colon path via plumbing so the OS filesystem does not need to
    // accept `:` in a filename. Windows Git rejects these paths in the index.
    let blob = repo.hash_blob("colon-blob-content\n");
    let cacheinfo = format!("100644,{blob},notes/file:with:colons.txt");
    repo.git(&["update-index", "--add", "--cacheinfo", &cacheinfo]);
    let tree = repo.git_stdout(&["write-tree"]);
    let commit = repo.git_stdout(&["commit-tree", &tree, "-m", "colon path via plumbing"]);
    repo.git(&["update-ref", "HEAD", &commit]);

    let value =
        revision_git::read_file_at_revision(repo.path(), "HEAD", "notes/file:with:colons.txt")
            .unwrap_or_else(|err| std::panic::panic_any(format!("colon path read: {err}")));
    assert_eq!(value.as_deref(), Some("colon-blob-content\n"));

    let tracked = revision_git::git_tracked_files_at_revision(repo.path(), "HEAD")
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked colon path: {err}")));
    assert_eq!(tracked, vec![PathBuf::from("notes/file:with:colons.txt")]);
}

#[test]
fn read_file_at_revision_no_longer_rejects_colon_in_normalize() {
    // Even when the host Git index cannot store colon paths, caller path
    // validation must not reject `:` merely because `commit:path` syntax once
    // required disambiguation. A missing tree entry returns None.
    let repo = TempGitRepo::new("revision-git-colon-normalize");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    repo.write("README.md", "fixture\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "fixture"]);

    let missing =
        revision_git::read_file_at_revision(repo.path(), "HEAD", "notes/file:with:colons.txt")
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!(
                    "colon characters must not fail path validation: {err}"
                ))
            });
    assert_eq!(missing, None);
}

#[test]
fn read_file_at_revision_distinguishes_symlink_directory_and_missing_paths() {
    let repo = TempGitRepo::new("revision-git-mode-discrimination");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);

    let blob = repo.hash_blob("regular\n");
    repo.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("100644,{blob},regular.txt"),
    ]);
    let link_blob = repo.hash_blob("regular.txt");
    repo.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("120000,{link_blob},link.txt"),
    ]);
    // Nested tree entry for a directory-shaped path: stage a blob under nested/
    // and also attempt to read the directory path itself.
    repo.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("100644,{blob},nested/child.txt"),
    ]);
    let tree = repo.git_stdout(&["write-tree"]);
    let commit = repo.git_stdout(&["commit-tree", &tree, "-m", "modes"]);
    repo.git(&["update-ref", "HEAD", &commit]);

    let regular = revision_git::read_file_at_revision(repo.path(), "HEAD", "regular.txt")
        .unwrap_or_else(|err| std::panic::panic_any(format!("regular read: {err}")));
    assert_eq!(regular.as_deref(), Some("regular\n"));

    let link = revision_git::read_file_at_revision(repo.path(), "HEAD", "link.txt")
        .unwrap_or_else(|err| std::panic::panic_any(format!("symlink read: {err}")));
    assert_eq!(link, None, "symlinks are not regular source-tree files");

    let nested_dir = revision_git::read_file_at_revision(repo.path(), "HEAD", "nested")
        .unwrap_or_else(|err| std::panic::panic_any(format!("directory read: {err}")));
    assert_eq!(
        nested_dir, None,
        "directories are not regular source-tree files"
    );

    let missing = revision_git::read_file_at_revision(repo.path(), "HEAD", "absent.txt")
        .unwrap_or_else(|err| std::panic::panic_any(format!("missing read: {err}")));
    assert_eq!(missing, None);
}

#[test]
fn read_file_at_revision_rejects_parent_and_absolute_paths() {
    let repo = TempGitRepo::new("revision-git-invalid-paths");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    repo.write("README.md", "fixture\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "fixture"]);

    for path in ["../escape.txt", "/abs.txt", ""] {
        let err = revision_git::read_file_at_revision(repo.path(), "HEAD", path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any(format!("path `{path}` should fail")));
        assert_eq!(err.kind(), CargoAllowErrorKind::InvalidConfig);
        assert_diagnostic_code(&err, "invalid_source_tree_path");
    }
}

#[test]
fn source_tree_path_bytes_maps_ordinary_relative_paths_to_git_form() {
    let slash = revision_git::source_tree_path_bytes_for_test(Path::new("src/lib.rs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("slash path: {err}")));
    assert_eq!(slash, b"src/lib.rs");

    let nested =
        revision_git::source_tree_path_bytes_for_test(Path::new("nested/dir/file name.rs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("nested path: {err}")));
    assert_eq!(nested, b"nested/dir/file name.rs");

    #[cfg(windows)]
    {
        let backslash = revision_git::source_tree_path_bytes_for_test(Path::new(r"src\lib.rs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("backslash path: {err}")));
        assert_eq!(backslash, b"src/lib.rs");
        let spaced =
            revision_git::source_tree_path_bytes_for_test(Path::new(r"nested\dir\file name.rs"))
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("spaced backslash path: {err}"))
                });
        assert_eq!(spaced, b"nested/dir/file name.rs");
    }
}

#[cfg(windows)]
#[test]
fn source_tree_path_bytes_rejects_windows_drive_unc_and_rooted_host_paths() {
    for path in [
        PathBuf::from(r"C:\repo\file.rs"),
        PathBuf::from(r"C:file.rs"),
        PathBuf::from(r"\\server\share\file.rs"),
        PathBuf::from(r"\\?\C:\repo\file.rs"),
        PathBuf::from(r"\rooted\file.rs"),
        PathBuf::from(r"/host-rooted/file.rs"),
    ] {
        let err = revision_git::source_tree_path_bytes_for_test(&path)
            .err()
            .unwrap_or_else(|| {
                std::panic::panic_any(format!(
                    "host path `{}` must be rejected before Git lookup",
                    path.display()
                ))
            });
        assert_eq!(err.kind(), CargoAllowErrorKind::InvalidConfig);
        assert_diagnostic_code(&err, "invalid_source_tree_path");
    }
}

#[cfg(windows)]
#[test]
fn read_file_at_revision_rejects_windows_drive_prefixed_caller_paths() {
    let repo = TempGitRepo::new("revision-git-windows-drive");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);
    repo.write("README.md", "fixture\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "fixture"]);

    let abs = PathBuf::from(r"C:\repo\README.md");
    let err = revision_git::read_file_at_revision(repo.path(), "HEAD", &abs)
        .err()
        .unwrap_or_else(|| std::panic::panic_any("drive path must fail closed"));
    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidConfig);
    assert_diagnostic_code(&err, "invalid_source_tree_path");
}

#[test]
fn read_file_at_revision_keeps_literal_pathspec_from_selecting_neighbors() {
    let repo = TempGitRepo::new("revision-git-literal-pathspec");
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "cargo-allow@example.invalid"]);
    repo.git(&["config", "user.name", "cargo-allow"]);

    // Use plumbing so hosts that cannot materialize `*` / `[` filenames still
    // prove exact tree selection under `--literal-pathspecs`. On Windows Git,
    // disable protectNTFS only for these plumbing index writes.
    let protect_off = [("core.protectNTFS", "false")];
    let bracket = repo.hash_blob("bracket-content\n");
    let plain = repo.hash_blob("plain-content\n");
    let star = repo.hash_blob("star-content\n");
    repo.git_with_config(
        &protect_off,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{bracket},literal[1].txt"),
        ],
    );
    repo.git_with_config(
        &protect_off,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{plain},literal1.txt"),
        ],
    );
    repo.git_with_config(
        &protect_off,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{star},literal*.txt"),
        ],
    );
    let tree = repo.git_stdout(&["write-tree"]);
    let commit = repo.git_stdout(&["commit-tree", &tree, "-m", "literal pathspec collision"]);
    repo.git(&["update-ref", "HEAD", &commit]);

    let bracket_text = revision_git::read_file_at_revision(repo.path(), "HEAD", "literal[1].txt")
        .unwrap_or_else(|err| std::panic::panic_any(format!("bracket path: {err}")));
    assert_eq!(bracket_text.as_deref(), Some("bracket-content\n"));

    let plain_text = revision_git::read_file_at_revision(repo.path(), "HEAD", "literal1.txt")
        .unwrap_or_else(|err| std::panic::panic_any(format!("plain path: {err}")));
    assert_eq!(plain_text.as_deref(), Some("plain-content\n"));

    let star_text = revision_git::read_file_at_revision(repo.path(), "HEAD", "literal*.txt")
        .unwrap_or_else(|err| std::panic::panic_any(format!("star path: {err}")));
    assert_eq!(star_text.as_deref(), Some("star-content\n"));

    let tracked = revision_git::git_tracked_files_at_revision(repo.path(), "HEAD")
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked collision paths: {err}")));
    assert_eq!(
        tracked,
        vec![
            PathBuf::from("literal*.txt"),
            PathBuf::from("literal1.txt"),
            PathBuf::from("literal[1].txt"),
        ]
    );
}

#[test]
fn parse_git_ls_tree_record_preserves_embedded_newline_raw_path() {
    let entry = revision_git::parse_git_ls_tree_record_for_test(
        b"100644 blob abc123\tfixtures/line\nbreak.rs",
    )
    .unwrap_or_else(|| std::panic::panic_any("newline path should parse"));
    assert_eq!(entry.raw_path, b"fixtures/line\nbreak.rs");
    assert_eq!(entry.path, PathBuf::from("fixtures/line\nbreak.rs"));
}

struct TempGitRepo {
    path: PathBuf,
}

impl TempGitRepo {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("system clock before epoch: {err}"))
            })
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("temp repo created: {err}")));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| {
                std::panic::panic_any(format!("test parent directory created: {err}"))
            });
        }
        fs::write(&path, contents)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test file written: {err}")));
    }

    fn hash_blob(&self, contents: &str) -> String {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("hash-object")
            .arg("-w")
            .arg("--stdin")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|err| std::panic::panic_any(format!("hash-object starts: {err}")));
        {
            let stdin = child
                .stdin
                .as_mut()
                .unwrap_or_else(|| std::panic::panic_any("hash-object stdin"));
            use std::io::Write;
            stdin
                .write_all(contents.as_bytes())
                .unwrap_or_else(|err| std::panic::panic_any(format!("hash-object write: {err}")));
        }
        let output = child
            .wait_with_output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("hash-object waits: {err}")));
        if !output.status.success() {
            std::panic::panic_any(format!(
                "hash-object failed: stdout=`{}` stderr=`{}`",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn git(&self, args: &[&str]) {
        let output = self.git_output(args);
        if !output.status.success() {
            std::panic::panic_any(format!(
                "git {args:?} failed: stdout=`{}` stderr=`{}`",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    /// Run git with temporary `-c` overrides (for plumbing names Windows protects).
    fn git_with_config(&self, config: &[(&str, &str)], args: &[&str]) {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.path);
        for (key, value) in config {
            command.arg("-c").arg(format!("{key}={value}"));
        }
        command.args(args);
        let output = command
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git process starts: {err}")));
        if !output.status.success() {
            std::panic::panic_any(format!(
                "git -c {config:?} {args:?} failed: stdout=`{}` stderr=`{}`",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    fn git_stdout(&self, args: &[&str]) -> String {
        let output = self.git_output(args);
        if !output.status.success() {
            std::panic::panic_any(format!(
                "git {args:?} failed: stdout=`{}` stderr=`{}`",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn git_output(&self, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git process starts: {err}")))
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
