# Generated writer-surface reconnaissance

> Investigation output for #3692. Every row still requires an explicit
> semantic owner and disposition. Regenerate from the exact source head;
> do not treat presence in this report as approval.

Rows: **612**

| Path | Line | Primitive/wrapper | Source marker |
| --- | ---: | --- | --- |
| `crates/allow-core/src/capped_read.rs` | 132 | `direct_fs_write` | `fs::write(&path, "hello\n")` |
| `crates/allow-core/src/capped_read.rs` | 138 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/allow-core/src/capped_read.rs` | 145 | `file_create` | `let mut file = File::create(&path).unwrap_or_else(\|err\| {` |
| `crates/allow-core/src/capped_read.rs` | 148 | `write_all` | `file.write_all(&vec![b'a'; (limit as usize) + 1])` |
| `crates/allow-core/src/capped_read.rs` | 154 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/allow-core/src/capped_read.rs` | 161 | `direct_fs_write` | `fs::write(&path, vec![b'b'; limit as usize])` |
| `crates/allow-core/src/capped_read.rs` | 167 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/allow-files/src/finding_generated_executable.rs` | 238 | `direct_fs_write` | `std::fs::write(&path, bytes)` |
| `crates/allow-files/src/finding_generated_executable.rs` | 248 | `remove_dir_all` | `let _ = std::fs::remove_dir_all(&root);` |
| `crates/allow-files/src/finding_generated_executable.rs` | 260 | `create_dir_all` | `std::fs::create_dir_all(&root)` |
| `crates/allow-files/src/finding_workflow.rs` | 218 | `create_dir_all` | `fs::create_dir_all(&workflows)` |
| `crates/allow-files/src/finding_workflow.rs` | 225 | `direct_fs_write` | `fs::write(&workflow_path, bytes)` |
| `crates/allow-files/src/finding_workflow.rs` | 235 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/allow-files/src/finding_workflow.rs` | 247 | `create_dir_all` | `fs::create_dir_all(&root)` |
| `crates/allow-inventory/src/git.rs` | 109 | `create_dir_all` | `fs::create_dir_all(&nested)?;` |
| `crates/allow-inventory/src/git.rs` | 121 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/allow-inventory/src/git.rs` | 127 | `direct_fs_write` | `fs::write(root.join(".git"), "gitdir: ../metadata\n")?;` |
| `crates/allow-inventory/src/git.rs` | 133 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/allow-inventory/src/git.rs` | 148 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/allow-inventory/src/root.rs` | 112 | `direct_fs_write` | `fs::write(&marker, "[workspace]\n")?;` |
| `crates/allow-inventory/src/root.rs` | 125 | `remove_dir_all` | `fs::remove_dir_all(root)?;` |
| `crates/allow-inventory/src/root.rs` | 134 | `create_dir_all` | `fs::create_dir_all(&nested)?;` |
| `crates/allow-inventory/src/root.rs` | 136 | `direct_fs_write` | `fs::write(&file, "pub fn demo() {}\n")?;` |
| `crates/allow-inventory/src/root.rs` | 151 | `remove_dir_all` | `fs::remove_dir_all(root)?;` |
| `crates/allow-inventory/src/root.rs` | 162 | `create_dir_all` | `fs::create_dir_all(&work_tree)?;` |
| `crates/allow-inventory/src/root.rs` | 163 | `create_dir_all` | `fs::create_dir_all(&unrelated_start)?;` |
| `crates/allow-inventory/src/root.rs` | 188 | `remove_dir_all` | `fs::remove_dir_all(root)?;` |
| `crates/allow-inventory/src/root.rs` | 200 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/allow-policy/src/evidence_tests/diagnostics.rs` | 15 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/allow-policy/src/evidence_tests/diagnostics.rs` | 17 | `direct_fs_write` | `fs::write(root.join("docs/safety.md"), "review notes")` |
| `crates/allow-policy/src/evidence_tests/diagnostics.rs` | 120 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/allow-policy/src/evidence_tests/diagnostics.rs` | 122 | `direct_fs_write` | `fs::write(root.join("docs/present.md"), "review notes")` |
| `crates/allow-policy/src/evidence_tests/diagnostics.rs` | 165 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/allow-policy/src/evidence_tests/diagnostics.rs` | 167 | `direct_fs_write` | `fs::write(root.join("docs/evidence.md"), "evidence")` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 11 | `create_dir_all` | `fs::create_dir_all(&root)` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 41 | `create_dir_all` | `fs::create_dir_all(&root)` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 71 | `create_dir_all` | `fs::create_dir_all(root.join("docs/safety"))` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 104 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 106 | `direct_fs_write` | `fs::write(root.join("docs/real.md"), "review notes")` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 141 | `create_dir_all` | `fs::create_dir_all(root.join("actual-docs"))` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 143 | `direct_fs_write` | `fs::write(root.join("actual-docs/safety.md"), "review notes")` |
| `crates/allow-policy/src/evidence_tests/local_rejections.rs` | 175 | `create_dir_all` | `fs::create_dir_all(&root)` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 12 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 14 | `direct_fs_write` | `fs::write(root.join("docs/safety.md"), "review notes")` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 43 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 45 | `direct_fs_write` | `fs::write(root.join("docs/safety.md"), "review notes")` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 89 | `create_dir_all` | `fs::create_dir_all(root.join("docs/evidence/unsafe-review")).unwrap_or_else(\|err\| {` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 92 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 135 | `create_dir_all` | `fs::create_dir_all(path.parent().unwrap_or_else(\|\| {` |
| `crates/allow-policy/src/evidence_tests/local_validation.rs` | 141 | `direct_fs_write` | `fs::write(&path, "{}").unwrap_or_else(\|err\| {` |
| `crates/allow-policy/src/federation/evaluate.rs` | 318 | `create_dir_all` | `fs::create_dir_all(root.join("policy"))` |
| `crates/allow-policy/src/federation/evaluate.rs` | 320 | `direct_fs_write` | `fs::write(root.join("policy/allow.toml"), "schema_version = \"1.0\"\n")` |
| `crates/allow-policy/src/federation/evaluate.rs` | 362 | `create_dir_all` | `fs::create_dir_all(root.join("policy"))` |
| `crates/allow-policy/src/federation/evaluate.rs` | 364 | `direct_fs_write` | `fs::write(root.join("policy/allow.toml"), "schema_version = \"1.0\"\n")` |
| `crates/allow-policy/src/federation/evaluate.rs` | 366 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy/src/federation/evaluate.rs` | 421 | `create_dir_all` | `fs::create_dir_all(root.join(".allow"))` |
| `crates/allow-policy/src/federation/evaluate.rs` | 423 | `direct_fs_write` | `fs::write(root.join(".allow/config.toml"), text)` |
| `crates/allow-policy/src/federation/evaluate.rs` | 437 | `remove_dir_all` | `fs::remove_dir_all(&dir)` |
| `crates/allow-policy/src/federation/evaluate.rs` | 440 | `create_dir_all` | `fs::create_dir_all(&dir)` |
| `crates/allow-policy/src/federation/evaluate.rs` | 446 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/allow-policy/src/import_roots/adapters/bespoke_ledger.rs` | 401 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/allow-policy/src/import_roots/adapters/bespoke_ledger.rs` | 476 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/allow-policy/src/import_roots/adapters/bespoke_ledger.rs` | 499 | `direct_fs_write` | `fs::write(&path, text)` |
| `crates/allow-policy-legacy/src/io.rs` | 35 | `direct_fs_write` | `fs::write(&path, "policy = \"non-rust-allowlist\"\n")` |
| `crates/allow-policy-legacy/src/io.rs` | 42 | `remove_file` | `fs::remove_file(&path)` |
| `crates/allow-policy-legacy/src/io.rs` | 75 | `direct_fs_write` | `fs::write(&path, bytes)` |
| `crates/allow-policy-legacy/src/io.rs` | 84 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/allow-policy-legacy/src/legacy_import_batch.rs` | 297 | `direct_fs_write` | `fs::write(dir.join(legacy_filename), text).unwrap_or_else(\|err\| {` |
| `crates/allow-policy-legacy/src/legacy_import_batch.rs` | 380 | `direct_fs_write` | `fs::write(&path, "policy = [\n")` |
| `crates/allow-policy-legacy/src/legacy_import_batch.rs` | 404 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/legacy_import_batch.rs` | 409 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/legacy_import_batch.rs` | 414 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/legacy_import_batch.rs` | 458 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/legacy_import_batch.rs` | 482 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/loader_policy_dir.rs` | 37 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/loader_policy_dir.rs` | 58 | `direct_fs_write` | `fs::write(undeclared.join("process-allowlist.toml"), without_status).unwrap_or_else(` |
| `crates/allow-policy-legacy/src/loader_policy_dir.rs` | 75 | `direct_fs_write` | `fs::write(&not_a_dir, "not a policy directory")` |
| `crates/allow-policy-legacy/src/loader_policy_dir.rs` | 99 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/loader_policy_dir.rs` | 104 | `direct_fs_write` | `fs::write(` |
| `crates/allow-policy-legacy/src/loader_policy_dir.rs` | 109 | `direct_fs_write` | `fs::write(dir.join("README.md"), "unsupported file")` |
| `crates/allow-policy-legacy/src/loader_policy_dir.rs` | 143 | `direct_fs_write` | `fs::write(dir.join("non-rust-allowlist.toml"), policy_fixture_text())` |
| `crates/allow-policy-legacy/src/loaders.rs` | 57 | `direct_fs_write` | `fs::write(&path, "policy = \"cargo-allow\"\nunknown_field = true\n")` |
| `crates/allow-policy-legacy/src/loaders.rs` | 84 | `direct_fs_write` | `fs::write(&path, source).map_err(\|err\| format!("write malformed legacy fixture: {err}"))?;` |
| `crates/allow-policy-legacy/src/loaders.rs` | 116 | `direct_fs_write` | `fs::write(&path, "policy = \"wrong-policy\"\n")` |
| `crates/allow-rust/src/scan_cache_store.rs` | 86 | `persist` | `let path = store.store_path();` |
| `crates/allow-rust/src/scan_cache_store.rs` | 95 | `persist` | `fn store_path(&self) -> PathBuf {` |
| `crates/allow-rust/src/scan_cache_store.rs` | 147 | `persist` | `self.flush_with_temp_path(None)` |
| `crates/allow-rust/src/scan_cache_store.rs` | 150 | `persist` | `fn flush_with_temp_path(&mut self, injected_temp: Option<&Path>) -> bool {` |
| `crates/allow-rust/src/scan_cache_store.rs` | 151 | `persist` | `self.flush_with_temp_path_and_wait_hook(injected_temp, None)` |
| `crates/allow-rust/src/scan_cache_store.rs` | 154 | `persist` | `fn flush_with_temp_path_and_wait_hook(` |
| `crates/allow-rust/src/scan_cache_store.rs` | 159 | `persist` | `self.flush_with_test_hooks(injected_temp, wait_hook, None)` |
| `crates/allow-rust/src/scan_cache_store.rs` | 162 | `persist` | `fn flush_with_test_hooks(` |
| `crates/allow-rust/src/scan_cache_store.rs` | 170 | `persist` | `\|\| path_is_unsafe(&self.store_path())` |
| `crates/allow-rust/src/scan_cache_store.rs` | 177 | `create_dir_all` | `if std::fs::create_dir_all(&self.dir).is_err() {` |
| `crates/allow-rust/src/scan_cache_store.rs` | 180 | `persist` | `if path_has_symlink_component(&self.dir) \|\| path_is_unsafe(&self.store_path()) {` |
| `crates/allow-rust/src/scan_cache_store.rs` | 191 | `persist` | `let dest = self.store_path();` |
| `crates/allow-rust/src/scan_cache_store.rs` | 195 | `open_options` | `let write = std::fs::OpenOptions::new()` |
| `crates/allow-rust/src/scan_cache_store.rs` | 200 | `write_all` | `file.write_all(&bytes)?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 204 | `remove_file` | `let _ = std::fs::remove_file(&tmp);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 214 | `remove_file` | `let _ = std::fs::remove_file(&tmp);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 217 | `rename` | `let mut moved = std::fs::rename(&tmp, &dest);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 219 | `remove_file` | `if path_is_unsafe(&dest) \|\| std::fs::remove_file(&dest).is_err() {` |
| `crates/allow-rust/src/scan_cache_store.rs` | 220 | `remove_file` | `let _ = std::fs::remove_file(&tmp);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 223 | `rename` | `moved = std::fs::rename(&tmp, &dest);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 226 | `remove_file` | `let _ = std::fs::remove_file(&tmp);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 323 | `open_options` | `let file = std::fs::OpenOptions::new()` |
| `crates/allow-rust/src/scan_cache_store.rs` | 367 | `remove_file` | `stale && std::fs::remove_file(path).is_ok()` |
| `crates/allow-rust/src/scan_cache_store.rs` | 807 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 809 | `file_create` | `let file = std::fs::File::create(&path).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 815 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 826 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 828 | `file_create` | `let file = std::fs::File::create(&stale).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 846 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 857 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 859 | `open_options` | `let held = std::fs::OpenOptions::new()` |
| `crates/allow-rust/src/scan_cache_store.rs` | 880 | `persist` | `let flushed = store.flush_with_temp_path_and_wait_hook(None, Some(&wait_hook));` |
| `crates/allow-rust/src/scan_cache_store.rs` | 893 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 904 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 932 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 938 | `persist` | `fn flush_fails_closed_when_cache_root_becomes_symlink() -> Result<(), String> {` |
| `crates/allow-rust/src/scan_cache_store.rs` | 951 | `remove_dir_all` | `let _ = std::fs::remove_dir_all(&root);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 952 | `create_dir_all` | `std::fs::create_dir_all(&cache_dir).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 953 | `create_dir_all` | `std::fs::create_dir_all(&outside).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 961 | `remove_dir_all` | `std::fs::remove_dir_all(&cache_dir).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 965 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 983 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 984 | `direct_fs_write` | `std::fs::write(&outside, b"sentinel").map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 995 | `persist` | `assert!(!store.flush_with_temp_path(Some(&temp)));` |
| `crates/allow-rust/src/scan_cache_store.rs` | 1000 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 1016 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 1027 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/allow-rust/src/scan_cache_store.rs` | 1044 | `remove_dir_all` | `let _ = std::fs::remove_dir_all(&self.0);` |
| `crates/allow-rust/src/scan_cache_store.rs` | 1048 | `create_dir_all` | `std::fs::create_dir_all(&outside).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/add.rs` | 354 | `emit_text` | `emit_text(args.summary_output.as_deref(), &rendered)?;` |
| `crates/cargo-allow/src/add.rs` | 363 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/add.rs` | 385 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/add_from_plan.rs` | 249 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/adoption.rs` | 106 | `emit_text` | `emit_text(output.as_deref(), &rendered)?;` |
| `crates/cargo-allow/src/adoption.rs` | 1089 | `create_dir_all` | `fs::create_dir_all(test_root.join(".github/workflows"))` |
| `crates/cargo-allow/src/adoption.rs` | 1092 | `direct_fs_write` | `fs::write(&workflow, "cargo-allow check --mode no-new")` |
| `crates/cargo-allow/src/adoption.rs` | 1095 | `direct_fs_write` | `fs::write(&unrelated, "readme").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/adoption.rs` | 1104 | `remove_dir_all` | `fs::remove_dir_all(test_root).map_err(\|error\| error.to_string())` |
| `crates/cargo-allow/src/adoption.rs` | 1170 | `create_dir_all` | `fs::create_dir_all(root.join("target/cargo-allow")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/adoption.rs` | 1187 | `remove_file` | `fs::remove_file(root.join(output)).map_err(\|error\| error.to_string())` |
| `crates/cargo-allow/src/capabilities.rs` | 500 | `emit_text` | `emit_text(args.output.as_deref(), &format!("{rendered}\n"))` |
| `crates/cargo-allow/src/capabilities.rs` | 824 | `remove_file` | `fs::remove_file(&human_path).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 845 | `remove_file` | `fs::remove_file(&excluded_path).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 869 | `remove_file` | `fs::remove_file(&finding_path).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 892 | `create_dir_all` | `fs::create_dir_all(&policy_dir).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 894 | `create_dir_all` | `fs::create_dir_all(&federation_dir).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 895 | `direct_fs_write` | `fs::write(federation_dir.join("config.toml"), "not = [valid")` |
| `crates/cargo-allow/src/capabilities.rs` | 898 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/capabilities.rs` | 921 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/capabilities.rs` | 991 | `remove_file` | `fs::remove_file(&human_path).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1031 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1039 | `create_dir_all` | `fs::create_dir_all(&policy_dir).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1041 | `direct_fs_write` | `fs::write(&policy, "not = [valid").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1057 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1065 | `create_dir_all` | `fs::create_dir_all(&policy_dir).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1067 | `direct_fs_write` | `fs::write(&policy, "not = [valid").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1083 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1090 | `create_dir_all` | `fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/capabilities.rs` | 1106 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/changie.rs` | 128 | `emit_text` | `emit_text(schema.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/changie.rs` | 223 | `emit_text` | `emit_text(lint.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/check.rs` | 109 | `remove_file` | `let _ = std::fs::remove_file(output);` |
| `crates/cargo-allow/src/check.rs` | 150 | `remove_file` | `let _ = std::fs::remove_file(output);` |
| `crates/cargo-allow/src/check.rs` | 354 | `write_file` | `write_file(path, &receipt)` |
| `crates/cargo-allow/src/check.rs` | 520 | `write_file` | `write_file(path, &receipt)` |
| `crates/cargo-allow/src/check.rs` | 567 | `write_file` | `write_file(path, &render_error_receipt(&err.to_string(), context))` |
| `crates/cargo-allow/src/command_support.rs` | 89 | `emit_text` | `pub(crate) fn emit_text(output: Option<&Path>, contents: &str) -> CargoAllowResult<()> {` |
| `crates/cargo-allow/src/command_support.rs` | 91 | `write_file` | `write_file(path, contents)` |
| `crates/cargo-allow/src/command_support.rs` | 135 | `write_file` | `write_file(path, contents)` |
| `crates/cargo-allow/src/command_support.rs` | 171 | `emit_text` | `let result = emit_text(Some(&output), "hello report\n");` |
| `crates/cargo-allow/src/command_support.rs` | 241 | `create_dir_all` | `fs::create_dir_all(&policy_dir).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/command_support.rs` | 243 | `direct_fs_write` | `fs::write(&policy, "original policy\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/command_support.rs` | 317 | `remove_dir_all` | `let _ = fs::remove_dir_all(&path);` |
| `crates/cargo-allow/src/command_support.rs` | 318 | `create_dir_all` | `fs::create_dir_all(&path)?;` |
| `crates/cargo-allow/src/command_support.rs` | 329 | `remove_dir_all` | `let _ = fs::remove_dir_all(&self.path);` |
| `crates/cargo-allow/src/compat_scan.rs` | 56 | `create_dir_all` | `fs::create_dir_all(&src_dir)` |
| `crates/cargo-allow/src/compat_scan.rs` | 58 | `create_dir_all` | `fs::create_dir_all(&ignored_dir)` |
| `crates/cargo-allow/src/compat_scan.rs` | 60 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/compat_scan.rs` | 65 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/compat_scan.rs` | 101 | `create_dir_all` | `fs::create_dir_all(&docs_dir)` |
| `crates/cargo-allow/src/compat_scan.rs` | 103 | `create_dir_all` | `fs::create_dir_all(&src_dir)` |
| `crates/cargo-allow/src/compat_scan.rs` | 105 | `direct_fs_write` | `fs::write(docs_dir.join("guide.md"), "# Guide\n")` |
| `crates/cargo-allow/src/compat_scan.rs` | 107 | `direct_fs_write` | `fs::write(src_dir.join("lib.rs"), "pub fn load() {}\n")` |
| `crates/cargo-allow/src/compat_test_support.rs` | 17 | `create_dir_all` | `fs::create_dir_all(&dir)` |
| `crates/cargo-allow/src/completions.rs` | 47 | `emit_text` | `emit_text(args.output.as_deref(), &render_completions(args.shell)?)?;` |
| `crates/cargo-allow/src/core_command_router.rs` | 85 | `emit_text` | `emit_text(args.output, &rendered)` |
| `crates/cargo-allow/src/diff.rs` | 416 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/diff.rs` | 425 | `write_file` | `write_file(path, &receipt)` |
| `crates/cargo-allow/src/diff.rs` | 1020 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|error\| {` |
| `crates/cargo-allow/src/diff.rs` | 1056 | `direct_fs_write` | `fs::write(&temporary, contents).map_err(\|error\| {` |
| `crates/cargo-allow/src/diff.rs` | 1065 | `rename` | `if let Err(error) = fs::rename(&temporary, &destination) {` |
| `crates/cargo-allow/src/diff.rs` | 1066 | `remove_file` | `let _ = fs::remove_file(&temporary);` |
| `crates/cargo-allow/src/doctor.rs` | 266 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/explain.rs` | 120 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 248 | `emit_text` | `emit_text(args.output.as_deref(), &rendered)?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1357 | `direct_fs_write` | `fs::write(path, contents).map_err(\|error\| {` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1420 | `direct_fs_write` | `fs::write(&path, format!("package:{package_name}").as_bytes())` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1438 | `direct_fs_write` | `fs::write(&build_path, b"build-artifact")` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1476 | `create_dir_all` | `fs::create_dir_all(&dir)` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1505 | `remove_dir_all` | `let _ = fs::remove_dir_all(path);` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1654 | `create_dir_all` | `fs::create_dir_all(&fixture).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1697 | `create_dir_all` | `fs::create_dir_all(` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1703 | `direct_fs_write` | `fs::write(root.join(tracked_path), "committed\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1762 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1764 | `direct_fs_write` | `fs::write(root.join(path), format!("{prepare} change\n"))` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1785 | `remove_file` | `fs::remove_file(root.join(path)).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1802 | `create_dir_all` | `fs::create_dir_all(root.join("watched/directory")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1803 | `create_dir_all` | `fs::create_dir_all(root.join("target")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1804 | `direct_fs_write` | `fs::write(root.join("watched/input.toml"), "source\n")` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1839 | `create_dir_all` | `fs::create_dir_all(root.join("watched/directory")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1865 | `remove_dir_all` | `let _ = fs::remove_dir_all(&self.0);` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1914 | `create_dir_all` | `fs::create_dir_all(&bin).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1933 | `create_dir_all` | `fs::create_dir_all(` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 1957 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2016 | `create_dir_all` | `fs::create_dir_all(` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2022 | `direct_fs_write` | `fs::write(&stale_receipt, "stale\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2033 | `direct_fs_write` | `fs::write(&outside, "{}\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2070 | `create_dir_all` | `fs::create_dir_all(` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2075 | `direct_fs_write` | `fs::write(path, "{}\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2306 | `create_dir_all` | `fs::create_dir_all(&dir).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2311 | `direct_fs_write` | `fs::write(&manifest, "{not-json\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2334 | `remove_file` | `fs::remove_file(dir.join("build-package.json")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2361 | `direct_fs_write` | `fs::write(dir.join(member), "{not-json\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2508 | `create_dir_all` | `fs::create_dir_all(&fixture).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2541 | `create_dir_all` | `fs::create_dir_all(&fixture).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2582 | `create_dir_all` | `fs::create_dir_all(&fixture).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_parity_command.rs` | 2618 | `create_dir_all` | `fs::create_dir_all(&fixture).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 43 | `remove_dir_all` | `let cleanup = fs::remove_dir_all(&workspace);` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 58 | `atomic_write` | `atomic_write_case(workspace),` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 82 | `create_dir_all` | `fs::create_dir_all(root.join("policy")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 83 | `create_dir_all` | `fs::create_dir_all(root.join("src")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 84 | `direct_fs_write` | `fs::write(root.join("src/lib.rs"), source).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 85 | `direct_fs_write` | `fs::write(root.join("policy/allow.toml"), initial).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 170 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 203 | `create_dir_all` | `fs::create_dir_all(root.join("policy")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 204 | `create_dir_all` | `fs::create_dir_all(root.join("docs")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 205 | `direct_fs_write` | `fs::write(root.join("docs/live.md"), "# live\n").map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 206 | `direct_fs_write` | `fs::write(root.join("policy/allow.toml"), &initial).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 220 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 286 | `create_dir_all` | `fs::create_dir_all(root.join("policy")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 287 | `create_dir_all` | `fs::create_dir_all(root.join("docs")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 288 | `direct_fs_write` | `fs::write(root.join("docs/new.md"), "# new\n").map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 345 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 374 | `create_dir_all` | `fs::create_dir_all(old_root.join("policy")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 375 | `create_dir_all` | `fs::create_dir_all(old_root.join("src")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 376 | `create_dir_all` | `fs::create_dir_all(new_root.join("policy")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 377 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 382 | `direct_fs_write` | `fs::write(&old_policy, &initial).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 394 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 467 | `create_dir_all` | `fs::create_dir_all(&old_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 468 | `create_dir_all` | `fs::create_dir_all(&new_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 470 | `direct_fs_write` | `fs::write(&old_source, &contents).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 477 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 501 | `create_dir_all` | `fs::create_dir_all(&old_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 502 | `create_dir_all` | `fs::create_dir_all(&new_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 511 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 535 | `create_dir_all` | `fs::create_dir_all(&old_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 536 | `create_dir_all` | `fs::create_dir_all(&new_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 561 | `atomic_write` | `fn atomic_write_case(workspace: &Path) -> CargoAllowResult<RepoEditParityCase> {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 566 | `write_file` | `let old_result = crate::command_support::write_file(&old_path, "[policy]\nvalue = 1\n");` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 567 | `write_file` | `let new_result = effortless_repo_edit::write_file(&new_path, "[policy]\nvalue = 1\n");` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 583 | `create_dir_all` | `fs::create_dir_all(root.join("policy")).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 594 | `apply_single_target` | `let new_success = apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 616 | `direct_fs_write` | `fs::write(root.join(target), "existing\n").map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 626 | `apply_single_target` | `let new_failure = apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 664 | `create_dir_all` | `fs::create_dir_all(&old_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 665 | `create_dir_all` | `fs::create_dir_all(&new_root).map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 666 | `direct_fs_write` | `fs::write(&old_path, "original\n").map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 667 | `direct_fs_write` | `fs::write(&new_path, "original\n").map_err(io_error)?;` |
| `crates/cargo-allow/src/extraction_repo_edit_runtime.rs` | 670 | `apply_single_target` | `let new_result = apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/hooks.rs` | 324 | `emit_text` | `emit_text(plan_args.output.as_deref(), &rendered)` |
| `crates/cargo-allow/src/hooks.rs` | 457 | `emit_text` | `emit_text(args.output.as_deref(), &rendered)?;` |
| `crates/cargo-allow/src/hooks.rs` | 720 | `emit_text` | `crate::emit_text(args.output.as_deref(), &rendered)` |
| `crates/cargo-allow/src/hooks.rs` | 896 | `remove_file` | `fs::remove_file(&hook_path).map_err(\|error\| {` |
| `crates/cargo-allow/src/hooks.rs` | 935 | `write_file` | `write_file(&hook_path, &retained)` |
| `crates/cargo-allow/src/hooks.rs` | 1331 | `write_file` | `write_file(path, &format!("{rendered}\n"))` |
| `crates/cargo-allow/src/hooks.rs` | 1449 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/hooks.rs` | 1987 | `remove_file` | `let _ = fs::remove_file(output);` |
| `crates/cargo-allow/src/hooks.rs` | 2063 | `direct_fs_write` | `fs::write(&failing, "#!/bin/sh\nexit 7\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/hooks.rs` | 2080 | `direct_fs_write` | `fs::write(&malformed, "#!/bin/sh\nprintf 'not-json\\n'\n")` |
| `crates/cargo-allow/src/hooks.rs` | 2154 | `direct_fs_write` | `fs::write(&path, "not json").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/hooks.rs` | 2162 | `direct_fs_write` | `fs::write(&invalid_hook, [0xff, 0xfe]).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/hooks.rs` | 2178 | `direct_fs_write` | `fs::write(&path, "#!/bin/sh\ncustom-hook\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/hooks.rs` | 2182 | `direct_fs_write` | `fs::write(&path, render_managed_hook(&plan)).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/hooks.rs` | 2186 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/hooks.rs` | 2206 | `remove_dir_all` | `fs::remove_dir_all(&path).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/hooks.rs` | 2208 | `create_dir_all` | `fs::create_dir_all(&path).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/hooks.rs` | 2215 | `remove_dir_all` | `let _ = fs::remove_dir_all(&self.path);` |
| `crates/cargo-allow/src/intent_delegate.rs` | 873 | `create_dir_all` | `std::fs::create_dir_all(&fixture_root).map_err(\|err\| err.to_string())?;` |
| `crates/cargo-allow/src/intent_delegate.rs` | 881 | `direct_fs_write` | `std::fs::write(&script, script_text).map_err(\|err\| err.to_string())?;` |
| `crates/cargo-allow/src/intent_delegate.rs` | 892 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|err\| err.to_string())?;` |
| `crates/cargo-allow/src/intent_delegate.rs` | 903 | `remove_dir_all` | `std::fs::remove_dir_all(fixture_root).map_err(\|err\| err.to_string())?;` |
| `crates/cargo-allow/src/intent_delegate.rs` | 1016 | `write_all` | `writer.write_all(bytes).map_err(\|err\| err.to_string())?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 391 | `create_dir_all` | `fs::create_dir_all(parent)?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 393 | `direct_fs_write` | `fs::write(path, b"#!/fake\n")` |
| `crates/cargo-allow/src/intent_provider.rs` | 399 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 412 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-allow/src/intent_provider.rs` | 419 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 423 | `create_dir_all` | `fs::create_dir_all(&config_dir)?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 424 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/intent_provider.rs` | 442 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-allow/src/intent_provider.rs` | 449 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 462 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-allow/src/intent_provider.rs` | 469 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 482 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-allow/src/intent_provider.rs` | 489 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-allow/src/intent_provider.rs` | 499 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-allow/src/list.rs` | 121 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/migrate_load.rs` | 183 | `direct_fs_write` | `fs::write(&from, bespoke_ledger_fixture_text())` |
| `crates/cargo-allow/src/migrate_load.rs` | 214 | `direct_fs_write` | `fs::write(&from, text).unwrap_or_else(\|err\| {` |
| `crates/cargo-allow/src/migrate_load.rs` | 267 | `direct_fs_write` | `fs::write(&from, render_policy(&canonical_policy_config()))` |
| `crates/cargo-allow/src/migrate_load.rs` | 302 | `create_dir_all` | `fs::create_dir_all(&policy_dir)` |
| `crates/cargo-allow/src/migrate_load.rs` | 304 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/migrate_load.rs` | 347 | `create_dir_all` | `fs::create_dir_all(&policy_dir)` |
| `crates/cargo-allow/src/migrate_load.rs` | 448 | `create_dir_all` | `fs::create_dir_all(&dir)` |
| `crates/cargo-allow/src/migrate_load.rs` | 454 | `remove_dir_all` | `let _ = fs::remove_dir_all(path);` |
| `crates/cargo-allow/src/migrate_render.rs` | 168 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/cargo-allow/src/migrate_render.rs` | 170 | `direct_fs_write` | `fs::write(root.join("docs/present.md"), "retained evidence")` |
| `crates/cargo-allow/src/migrate_render.rs` | 246 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/cargo-allow/src/migrate_render.rs` | 248 | `direct_fs_write` | `fs::write(root.join("docs/present.md"), "retained evidence")` |
| `crates/cargo-allow/src/migrate_render.rs` | 312 | `remove_dir_all` | `let _ = fs::remove_dir_all(path);` |
| `crates/cargo-allow/src/mutation_apply.rs` | 18 | `apply_single_target` | `let response = apply_single_target(request);` |
| `crates/cargo-allow/src/mutation_apply.rs` | 56 | `create_dir_all` | `fs::create_dir_all(target.parent().ok_or("missing target parent")?)` |
| `crates/cargo-allow/src/mutation_apply.rs` | 58 | `direct_fs_write` | `fs::write(&target, "existing\n").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/mutation_apply.rs` | 88 | `remove_dir_all` | `let _ = fs::remove_dir_all(&path);` |
| `crates/cargo-allow/src/mutation_apply.rs` | 89 | `create_dir_all` | `fs::create_dir_all(&path).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/mutation_apply.rs` | 100 | `remove_dir_all` | `let _ = fs::remove_dir_all(&self.path);` |
| `crates/cargo-allow/src/policy_config.rs` | 446 | `create_dir_all` | `fs::create_dir_all(package_config.parent().unwrap_or(&package_root)).unwrap_or_else(` |
| `crates/cargo-allow/src/policy_config.rs` | 449 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/policy_config.rs` | 454 | `direct_fs_write` | `fs::write(&package_config, render_policy(&valid_policy_config()))` |
| `crates/cargo-allow/src/policy_config.rs` | 459 | `create_dir_all` | `fs::create_dir_all(workspace_config.parent().unwrap_or(&workspace_root)).unwrap_or_else(` |
| `crates/cargo-allow/src/policy_config.rs` | 462 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/policy_config.rs` | 467 | `direct_fs_write` | `fs::write(&workspace_config, render_policy(&valid_policy_config()))` |
| `crates/cargo-allow/src/policy_config.rs` | 682 | `create_dir_all` | `fs::create_dir_all(&policy_dir)` |
| `crates/cargo-allow/src/policy_config.rs` | 685 | `direct_fs_write` | `fs::write(&policy_path, render_policy(&cfg))` |
| `crates/cargo-allow/src/policy_config.rs` | 697 | `create_dir_all` | `fs::create_dir_all(&dir)` |
| `crates/cargo-allow/src/policy_config.rs` | 703 | `remove_dir_all` | `let _ = fs::remove_dir_all(path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 399 | `emit_text` | `crate::emit_text(identity_args.output.as_deref(), &output)` |
| `crates/cargo-allow/src/precommit_tool.rs` | 468 | `remove_file` | `let _ = fs::remove_file(path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 469 | `direct_fs_write` | `fs::write(path, bytes)?;` |
| `crates/cargo-allow/src/precommit_tool.rs` | 517 | `remove_file` | `let _ = fs::remove_file(&missing);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 550 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 580 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 596 | `direct_fs_write` | `fs::write(&path, replacement)?;` |
| `crates/cargo-allow/src/precommit_tool.rs` | 598 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 611 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 648 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 675 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 701 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 727 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/precommit_tool.rs` | 754 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/cargo-allow/src/propose.rs` | 288 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/prune.rs` | 160 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/prune.rs` | 240 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/reference.rs` | 100 | `emit_text` | `emit_text(args.output.as_deref(), &rendered)` |
| `crates/cargo-allow/src/refresh.rs` | 110 | `apply_single_target` | `apply_single_target(SingleTargetApplyRequest {` |
| `crates/cargo-allow/src/refresh.rs` | 267 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/reporting.rs` | 242 | `emit_text` | `emit_text(args.output, &text)` |
| `crates/cargo-allow/src/reporting.rs` | 304 | `create_dir_all` | `fs::create_dir_all(&docs)` |
| `crates/cargo-allow/src/reporting.rs` | 306 | `direct_fs_write` | `fs::write(docs.join("present.md"), "present evidence")` |
| `crates/cargo-allow/src/reporting.rs` | 308 | `direct_fs_write` | `fs::write(docs.join("not-in-source-tree.md"), "untracked evidence")` |
| `crates/cargo-allow/src/reporting.rs` | 344 | `remove_dir_all` | `fs::remove_dir_all(&root)` |
| `crates/cargo-allow/src/reporting.rs` | 384 | `remove_dir_all` | `fs::remove_dir_all(&root)` |
| `crates/cargo-allow/src/reporting.rs` | 438 | `remove_dir_all` | `fs::remove_dir_all(&root)` |
| `crates/cargo-allow/src/reporting.rs` | 454 | `create_dir_all` | `fs::create_dir_all(&root)` |
| `crates/cargo-allow/src/spec_precommit.rs` | 236 | `emit_text` | `emit_text(args.output.as_deref(), &rendered)?;` |
| `crates/cargo-allow/src/spec_precommit.rs` | 239 | `write_file` | `write_file(` |
| `crates/cargo-allow/src/spec_precommit.rs` | 277 | `emit_text` | `emit_text(args.output.as_deref(), &rendered)?;` |
| `crates/cargo-allow/src/spec_precommit.rs` | 281 | `write_file` | `write_file(receipt, &json)` |
| `crates/cargo-allow/src/spec_precommit.rs` | 692 | `remove_file` | `let _ = fs::remove_file(&output);` |
| `crates/cargo-allow/src/spec_precommit.rs` | 693 | `remove_file` | `let _ = fs::remove_file(&receipt);` |
| `crates/cargo-allow/src/spec_precommit.rs` | 701 | `remove_file` | `let _ = fs::remove_file(&output);` |
| `crates/cargo-allow/src/spec_precommit.rs` | 702 | `remove_file` | `let _ = fs::remove_file(&receipt);` |
| `crates/cargo-allow/src/spec_precommit.rs` | 715 | `remove_file` | `let _ = fs::remove_file(&output);` |
| `crates/cargo-allow/src/spec_precommit.rs` | 719 | `remove_file` | `let _ = fs::remove_file(&output);` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 48 | `emit_text` | `emit_text(args.output, &rendered)?;` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 50 | `write_file` | `write_file(path, &render_spec_system_json(&report))` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 82 | `emit_text` | `emit_text(args.output, &rendered)` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 102 | `emit_text` | `emit_text(args.output, &rendered)` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 121 | `emit_text` | `emit_text(args.output, &rendered)?;` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 130 | `emit_text` | `emit_text(args.output, &rendered)` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 195 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|e\| {` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 231 | `direct_fs_write` | `fs::write(&path, file.contents).map_err(\|e\| {` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 268 | `create_dir_all` | `fs::create_dir_all(root.join(".allow/compatibility")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 273 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 293 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 307 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/cargo-allow/src/spec_system_commands.rs` | 340 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/cargo-allow/src/spec_system_report.rs` | 263 | `create_dir_all` | `fs::create_dir_all(root.join(".allow/profiles")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/spec_system_report.rs` | 264 | `create_dir_all` | `fs::create_dir_all(root.join("docs/status")).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/spec_system_report.rs` | 265 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/spec_system_report.rs` | 270 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 855 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 897 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 909 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 918 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 958 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 980 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1015 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1039 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1075 | `remove_dir_all` | `let _ = fs::remove_dir_all(&self.0);` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1139 | `create_dir_all` | `fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1149 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1151 | `direct_fs_write` | `fs::write(full, contents).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1500 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1529 | `direct_fs_write` | `fs::write(&requirement_path, &amended)` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1609 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/cargo-allow/src/spec_system_workspace.rs` | 1768 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/cargo-allow/src/support_bundle.rs` | 106 | `write_file` | `crate::write_file(output, &format!("{json}\n"))` |
| `crates/cargo-allow/src/support_bundle.rs` | 224 | `create_dir_all` | `fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-allow/src/support_bundle.rs` | 230 | `remove_file` | `let _ = fs::remove_file(&output);` |
| `crates/cargo-allow/src/vocabulary.rs` | 44 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/why.rs` | 127 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/why.rs` | 260 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/worklist.rs` | 148 | `emit_text` | `emit_text(args.output.as_deref(), &text)?;` |
| `crates/cargo-allow/src/world.rs` | 785 | `create_dir_all` | `fs::create_dir_all(root.join("src")).map_err(\|err\| format!("src dir: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 786 | `direct_fs_write` | `fs::write(root.join("src/lib.rs"), "fn disabled() {}\n")` |
| `crates/cargo-allow/src/world.rs` | 801 | `direct_fs_write` | `fs::write(&cache_file, sentinel).map_err(\|err\| format!("seed cache: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 804 | `remove_file` | `fs::remove_file(&lock_file).map_err(\|err\| format!("remove lock: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 813 | `remove_file` | `fs::remove_file(entry.path()).map_err(\|err\| format!("remove temp: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 839 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|err\| format!("remove fixture dir: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 846 | `create_dir_all` | `fs::create_dir_all(root.join("src")).map_err(\|err\| format!("src dir: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 847 | `create_dir_all` | `fs::create_dir_all(root.join("policy")).map_err(\|err\| format!("policy dir: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 848 | `direct_fs_write` | `fs::write(root.join("src/lib.rs"), "fn disabled() {}\n")` |
| `crates/cargo-allow/src/world.rs` | 850 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/world.rs` | 869 | `direct_fs_write` | `fs::write(&cache_file, sentinel).map_err(\|err\| format!("seed cache: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 872 | `remove_file` | `fs::remove_file(&lock_file).map_err(\|err\| format!("remove lock: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 881 | `remove_file` | `fs::remove_file(entry.path()).map_err(\|err\| format!("remove temp: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 908 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|err\| format!("remove fixture dir: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 915 | `create_dir_all` | `fs::create_dir_all(root.join("src")).map_err(\|err\| format!("src dir: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 916 | `direct_fs_write` | `fs::write(root.join("src/lib.rs"), "fn cached() {}\n")` |
| `crates/cargo-allow/src/world.rs` | 966 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|err\| format!("remove fixture: {err}"))?;` |
| `crates/cargo-allow/src/world.rs` | 989 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/cargo-allow/src/world.rs` | 1011 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/cargo-allow/src/world.rs` | 1038 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/cargo-allow/src/world.rs` | 1060 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/cargo-allow/src/world.rs` | 1093 | `create_dir_all` | `fs::create_dir_all(root.join("policy"))` |
| `crates/cargo-allow/src/world.rs` | 1095 | `direct_fs_write` | `fs::write(root.join("policy/allow.toml"), render_policy(&cfg))` |
| `crates/cargo-allow/src/world.rs` | 1097 | `direct_fs_write` | `fs::write(root.join("candidate.rs"), "fn candidate() {}\n")` |
| `crates/cargo-allow/src/world.rs` | 1114 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/cargo-allow/src/world.rs` | 1121 | `create_dir_all` | `fs::create_dir_all(root.join("policy"))` |
| `crates/cargo-allow/src/world.rs` | 1123 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/world.rs` | 1136 | `direct_fs_write` | `fs::write(root.join("invalid.rs"), [0xff_u8, 0xfe_u8])` |
| `crates/cargo-allow/src/world.rs` | 1138 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/world.rs` | 1183 | `remove_dir_all` | `fs::remove_dir_all(root)` |
| `crates/cargo-allow/src/world.rs` | 1188 | `create_dir_all` | `fs::create_dir_all(root.join("policy"))` |
| `crates/cargo-allow/src/world.rs` | 1190 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/cargo-allow/src/world.rs` | 1194 | `direct_fs_write` | `fs::write(root.join("policy/allow.toml"), render_policy(&cfg))` |
| `crates/cargo-allow/src/world.rs` | 1206 | `direct_fs_write` | `fs::write(root.join("docs/evidence.md"), "review notes")` |
| `crates/cargo-allow/src/world.rs` | 1211 | `create_dir_all` | `fs::create_dir_all(root.join("policy"))` |
| `crates/cargo-allow/src/world.rs` | 1213 | `create_dir_all` | `fs::create_dir_all(root.join("docs"))` |
| `crates/cargo-allow/src/world.rs` | 1217 | `direct_fs_write` | `fs::write(root.join("policy/allow.toml"), render_policy(&cfg))` |
| `crates/cargo-allow/src/world.rs` | 1229 | `direct_fs_write` | `fs::write(root.join("docs/rationale.md"), "review notes")` |
| `crates/cargo-allow/src/world.rs` | 1295 | `remove_dir_all` | `fs::remove_dir_all(&dir)` |
| `crates/cargo-allow/src/world.rs` | 1298 | `create_dir_all` | `fs::create_dir_all(&dir)` |
| `crates/cargo-allow/src/world.rs` | 1315 | `remove_dir_all` | `fs::remove_dir_all(dir)` |
| `crates/cargo-allow/src/world.rs` | 1318 | `create_dir_all` | `fs::create_dir_all(root.join("pkg/src"))` |
| `crates/cargo-allow/src/world.rs` | 1320 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/world.rs` | 1325 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-allow/src/world.rs` | 1330 | `create_dir_all` | `fs::create_dir_all(root.join("policy"))` |
| `crates/cargo-allow/src/world.rs` | 1332 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-intent/src/cli.rs` | 131 | `create_dir_all` | `std::fs::create_dir_all(parent)` |
| `crates/cargo-intent/src/cli.rs` | 136 | `direct_fs_write` | `std::fs::write(path, format!("{json}\n"))` |
| `crates/cargo-proof/src/cli.rs` | 467 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 483 | `direct_fs_write` | `std::fs::write(&plan, b"{}").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 484 | `direct_fs_write` | `std::fs::write(&receipts, b"{}").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 497 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 507 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 511 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/cli.rs` | 516 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/cli.rs` | 545 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 555 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 560 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/cli.rs` | 565 | `direct_fs_write` | `std::fs::write(&sentinel_path, b"must remain unchanged")` |
| `crates/cargo-proof/src/cli.rs` | 567 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/cli.rs` | 625 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 635 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/cli.rs` | 639 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/cli.rs` | 644 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/cli.rs` | 675 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/cli.rs` | 692 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/plan.rs` | 116 | `direct_fs_write` | `std::fs::write(&temporary, format!("{serialized}\n")).map_err(\|err\| PlanErrorV1 {` |
| `crates/cargo-proof/src/plan.rs` | 120 | `rename` | `if let Err(err) = std::fs::rename(&temporary, output_path) {` |
| `crates/cargo-proof/src/plan.rs` | 121 | `remove_file` | `let _ = std::fs::remove_file(&temporary);` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 283 | `create_dir_all` | `fs::create_dir_all(parent)?;` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 285 | `direct_fs_write` | `fs::write(path, b"#!/fake\n")` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 291 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 304 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 311 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 315 | `create_dir_all` | `fs::create_dir_all(&config_dir)?;` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 316 | `direct_fs_write` | `fs::write(` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 334 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 341 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 354 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 361 | `create_dir_all` | `fs::create_dir_all(&root)?;` |
| `crates/cargo-proof/src/providers/cargo_allow/discovery.rs` | 374 | `remove_dir_all` | `let _ = fs::remove_dir_all(root);` |
| `crates/cargo-proof/src/receipt_status.rs` | 681 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 685 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/receipt_status.rs` | 690 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/receipt_status.rs` | 725 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/receipt_status.rs` | 737 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 745 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 753 | `direct_fs_write` | `std::fs::write(&plan_path, b"{}").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 754 | `direct_fs_write` | `std::fs::write(&manifest_path, b"{}").map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 759 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 769 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 774 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/receipt_status.rs` | 779 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/receipt_status.rs` | 795 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 806 | `create_dir_all` | `std::fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/cargo-proof/src/receipt_status.rs` | 831 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/receipt_status.rs` | 836 | `direct_fs_write` | `std::fs::write(` |
| `crates/cargo-proof/src/receipt_status.rs` | 852 | `remove_dir_all` | `std::fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 13 | `write_file` | `pub fn write_file(path: impl AsRef<Path>, contents: &str) -> RepoEditResult<()> {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 16 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|e\| {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 31 | `write_all` | `if let Err(e) = file.write_all(contents.as_bytes()) {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 32 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 41 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 48 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 55 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 61 | `rename` | `if let Err(error) = fs::rename(&tmp, path) {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 62 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 81 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|e\| {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 89 | `rename` | `fs::rename(path, &bak).map_err(\|e\| {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 97 | `write_file` | `return match write_file(path, contents) {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 100 | `rename` | `if let Err(restore_error) = fs::rename(&bak, path) {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 113 | `open_options` | `let mut file = OpenOptions::new()` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 125 | `write_all` | `if let Err(e) = file.write_all(contents.as_bytes()) {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 126 | `remove_file` | `let _ = fs::remove_file(path);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 133 | `remove_file` | `let _ = fs::remove_file(path);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 137 | `remove_file` | `let _ = fs::remove_file(path);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 160 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|error\| {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 176 | `write_all` | `file.write_all(contents.as_bytes()).map_err(\|error\| {` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 196 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 200 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 206 | `remove_file` | `let _ = fs::remove_file(&tmp);` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 220 | `open_options` | `match OpenOptions::new()` |
| `crates/effortless-repo-edit/src/atomic_write.rs` | 244 | `open_options` | `let directory = OpenOptions::new()` |
| `crates/effortless-repo-edit/src/mutation_lock.rs` | 51 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|error\| {` |
| `crates/effortless-repo-edit/src/mutation_lock.rs` | 58 | `open_options` | `let file = OpenOptions::new()` |
| `crates/effortless-repo-edit/src/mutation_lock.rs` | 112 | `remove_file` | `let _ = fs::remove_file(&path);` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 68 | `apply_single_target` | `pub fn apply_single_target(request: SingleTargetApplyRequest<'_>) -> SingleTargetApplyResponse {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 289 | `write_file` | `SingleTargetApplyMode::AtomicReplace => write_file(write_path, request.contents),` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 416 | `apply_single_target` | `let create = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 432 | `apply_single_target` | `let replace = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 454 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 473 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 498 | `create_dir_all` | `fs::create_dir_all(parent)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 499 | `direct_fs_write` | `fs::write(&target, "existing\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 501 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 530 | `create_dir_all` | `fs::create_dir_all(parent)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 531 | `direct_fs_write` | `fs::write(&target, "before\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 533 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 560 | `create_dir_all` | `fs::create_dir_all(&target)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 562 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 587 | `direct_fs_write` | `fs::write(&parent, "sentinel\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 589 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 616 | `direct_fs_write` | `fs::write(&foreign, "foreign sentinel\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 619 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 646 | `create_dir_all` | `fs::create_dir_all(target.parent().ok_or("target needs a parent")?)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 647 | `direct_fs_write` | `fs::write(&foreign, "foreign sentinel\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 650 | `apply_single_target` | `let response = apply_single_target(SingleTargetApplyRequest {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 679 | `create_dir_all` | `fs::create_dir_all(&parent)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 680 | `direct_fs_write` | `fs::write(&target, "held A\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 681 | `create_dir_all` | `fs::create_dir_all(&retargeted)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 683 | `direct_fs_write` | `fs::write(&foreign, "foreign B sentinel\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 701 | `remove_dir_all` | `fs::remove_dir_all(&parent)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 735 | `create_dir_all` | `fs::create_dir_all(&parent)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 736 | `create_dir_all` | `fs::create_dir_all(&retargeted)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 738 | `direct_fs_write` | `fs::write(&sentinel, "create sentinel\n")?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 745 | `remove_dir_all` | `if let Err(error) = fs::remove_dir_all(&parent) {` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 790 | `remove_dir_all` | `let _ = fs::remove_dir_all(&path);` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 791 | `create_dir_all` | `fs::create_dir_all(&path)?;` |
| `crates/effortless-repo-edit/src/single_target_apply.rs` | 802 | `remove_dir_all` | `let _ = fs::remove_dir_all(&self.path);` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 365 | `create_dir_all` | `fs::create_dir_all(root.join("target")).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 366 | `direct_fs_write` | `fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\n")` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 368 | `direct_fs_write` | `fs::write(root.join("target/generated.rs"), "ignored")` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 380 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 390 | `direct_fs_write` | `fs::write(root.join("kept.rs"), "fn kept() {}\n").map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 391 | `direct_fs_write` | `fs::write(root.join("deleted.rs"), "fn deleted() {}\n")` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 394 | `remove_file` | `fs::remove_file(root.join("deleted.rs")).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 402 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 409 | `direct_fs_write` | `fs::write(root.join("kept.rs"), "fn kept() {}\n").map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 431 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 462 | `remove_dir_all` | `fs::remove_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 487 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 495 | `create_dir_all` | `fs::create_dir_all(root.join("nested")).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 496 | `direct_fs_write` | `fs::write(root.join("nested/z.rs"), "z\n").map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 497 | `direct_fs_write` | `fs::write(root.join("a.rs"), "a\n").map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 512 | `remove_dir_all` | `fs::remove_dir_all(root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/inventory.rs` | 539 | `create_dir_all` | `fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/staged_index.rs` | 1033 | `create_dir_all` | `fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/staged_index.rs` | 1044 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/staged_index.rs` | 1046 | `direct_fs_write` | `fs::write(full, contents).map_err(\|error\| error.to_string())` |
| `crates/effortless-repo-snapshot/src/staged_index.rs` | 1085 | `remove_dir_all` | `let _ = fs::remove_dir_all(&self.root);` |
| `crates/effortless-repo-snapshot/src/staged_index.rs` | 1350 | `write_all` | `.write_all(index_info.as_bytes())` |
| `crates/effortless-repo-snapshot/src/staged_index.rs` | 1396 | `direct_fs_write` | `fs::write(repo.root.join(&raw_name), b"bytes\n").map_err(\|error\| error.to_string())?;` |
| `crates/effortless-repo-snapshot/src/util.rs` | 160 | `direct_fs_write` | `std::fs::write(&path, b"hello").map_err(\|error\| format!("write fixture: {error}"))?;` |
| `crates/effortless-repo-snapshot/src/util.rs` | 165 | `direct_fs_write` | `std::fs::write(&path, vec![b'x'; (SOURCE_FILE_READ_MAX_BYTES as usize) + 1])` |
| `crates/effortless-repo-snapshot/src/util.rs` | 170 | `remove_file` | `let _ = std::fs::remove_file(path);` |
| `crates/effortless-repo-snapshot/src/util.rs` | 220 | `direct_fs_write` | `std::fs::write(` |
| `crates/effortless-repo-snapshot/src/util.rs` | 229 | `remove_file` | `let _ = std::fs::remove_file(link);` |
| `crates/effortless-repo-snapshot/src/util.rs` | 230 | `remove_file` | `let _ = std::fs::remove_file(target);` |
| `crates/intent-engine/src/workspace/source_views.rs` | 68 | `create_dir_all` | `fs::create_dir_all(&root).map_err(\|error\| error.to_string())?;` |
| `crates/intent-engine/src/workspace/source_views.rs` | 79 | `create_dir_all` | `fs::create_dir_all(parent).map_err(\|error\| error.to_string())?;` |
| `crates/intent-engine/src/workspace/source_views.rs` | 81 | `direct_fs_write` | `fs::write(&full, format!("authority bytes for {path}"))` |
| `crates/intent-engine/src/workspace/source_views.rs` | 94 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
| `crates/intent-engine/src/workspace/source_views.rs` | 107 | `remove_dir_all` | `let _ = fs::remove_dir_all(&root);` |
