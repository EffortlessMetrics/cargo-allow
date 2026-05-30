use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

pub(crate) fn run_cargo_allow(args: &[&str]) -> std::process::Output {
    let output = cargo_allow_command()
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow: {err}")));
    let command = format!("cargo-allow {}", args.join(" "));
    assert_status(&command, &output, true);
    assert_stdout_empty(
        &command,
        &output,
        "should not write stdout when output files are set",
    );
    assert_stderr_empty(
        &command,
        &output,
        "should not write stderr when output files are set",
    );
    output
}

pub(crate) fn assert_source_syntax_artifact(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
) -> serde_json::Value {
    assert_source_syntax_artifact_with_inventory(
        path,
        expected_schema_id,
        expected_command,
        "filesystem_fallback",
    )
}

pub(crate) fn assert_source_syntax_artifact_with_inventory(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
    expected_source: &str,
) -> serde_json::Value {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX,
        expected_source,
    );
    value
}

pub(crate) fn assert_policy_migration_artifact(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
) {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_POLICY_MIGRATION,
        allow_report::INVENTORY_SOURCE_UNKNOWN,
    );
}

fn assert_inventory(value: &serde_json::Value, expected_scanner: &str, expected_source: &str) {
    assert_eq!(
        value
            .pointer("/inventory/scanner")
            .and_then(serde_json::Value::as_str),
        Some(expected_scanner),
        "inventory scanner"
    );
    assert_eq!(
        value
            .pointer("/inventory/source")
            .and_then(serde_json::Value::as_str),
        Some(expected_source),
        "inventory source"
    );
}

pub(crate) fn assert_policy_output(path: &Path) {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read policy output {}: {err}", path.display()))
    });
    assert!(
        text.contains("schema_version = \"0.1\""),
        "{} should be policy TOML",
        path.display()
    );
    assert!(
        !text.contains("\"schema_id\""),
        "{} should not contain summary JSON",
        path.display()
    );
}

pub(crate) fn path_arg(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| std::panic::panic_any(format!("non-UTF-8 path: {}", path.display())))
}

pub(crate) fn commit_fixture_base(root: &Path) {
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

pub(crate) struct SourceTreeFixture {
    pub(crate) root: PathBuf,
    root_arg: String,
}

impl SourceTreeFixture {
    pub(crate) fn new(prefix: &str) -> Self {
        let root = temp_root(prefix);
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture: {err}")));
        let root_arg = root
            .to_str()
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("non-UTF-8 fixture path: {}", root.display()))
            })
            .to_string();
        Self { root, root_arg }
    }

    pub(crate) fn root_str(&self) -> &str {
        &self.root_arg
    }

    pub(crate) fn write_minimal_policy(&self) {
        fs::write(
            self.root.join("policy/allow.toml"),
            r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = ["policy/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    pub(crate) fn write_panic_source(&self) {
        fs::create_dir_all(self.root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
        fs::write(
            self.root.join("src/lib.rs"),
            "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
    }

    pub(crate) fn append_saved_artifact_allow_entries(&self) {
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-panic-fixture"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps explain/check saved artifact output covered."
evidence = ["test:saved_json_outputs_keep_source_tree_contracts"]
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"

[[allow]]
id = "allow-stale-fixture"
kind = "non_rust_file"
family = "documentation"
path = "docs/missing.md"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps prune saved artifact output covered."
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/missing.md"
target_fingerprint = "md"
glob = "docs/missing.md"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    pub(crate) fn write_policy_with_broken_evidence(&self) {
        self.write_policy_with_evidence(
            "allow-broken-evidence",
            "Fixture exercises broken evidence worklist output.",
            "doc:docs/missing-evidence.md",
        );
    }

    pub(crate) fn write_policy_with_invalid_evidence_scope(&self) {
        self.write_policy_with_evidence(
            "allow-invalid-evidence-scope",
            "Fixture exercises invalid evidence scope worklist output.",
            "doc:../outside.md",
        );
    }

    pub(crate) fn write_policy_with_missing_evidence_entry(&self) {
        self.write_minimal_policy();
        fs::create_dir_all(self.root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        fs::write(
            self.root.join("docs/policy.md"),
            "# Policy\n\nFixture documentation surface.\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write docs fixture: {err}")));
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-missing-evidence"
kind = "non_rust_file"
family = "documentation"
path = "docs/policy.md"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps missing-evidence worklist saved artifact output covered."
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/policy.md"
target_fingerprint = "md"
glob = "docs/policy.md"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    pub(crate) fn write_policy_with_baseline_debt_entry(&self) {
        self.write_minimal_policy();
        self.write_panic_source();
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-baseline-debt"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "unowned"
classification = "baseline_debt"
reason = "Generated by cargo-allow propose; requires human review."
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    pub(crate) fn write_policy_with_weak_evidence(&self) {
        self.write_policy_with_evidence(
            "allow-weak-evidence",
            "Fixture exercises weak evidence worklist output.",
            "spreadsheet:manual-review",
        );
    }

    pub(crate) fn write_policy_with_present_and_traceability_evidence(&self) {
        self.write_minimal_policy();
        fs::create_dir_all(self.root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
        fs::write(
            self.root.join("src/lib.rs"),
            "pub fn load(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write unsafe source: {err}")));
        fs::create_dir_all(self.root.join("docs/evidence"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create evidence dir: {err}")));
        fs::write(
            self.root.join("docs/evidence/safety.md"),
            "# Safety evidence\n\nFixture evidence artifact.\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence fixture: {err}")));
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-evidence-diagnostics"
kind = "unsafe"
family = "unsafe_block"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps explain evidence diagnostics covered."
evidence = [
  "doc:docs/evidence/safety.md",
  "test:saved_explain_output_reports_present_and_traceability_evidence",
]
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "unsafe_block"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    fn write_policy_with_evidence(&self, id: &str, reason: &str, evidence: &str) {
        self.write_minimal_policy();
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(&format!(
            r#"

[[allow]]
id = "{id}"
kind = "unsafe"
family = "unsafe_block"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "{reason}"
evidence = ["{evidence}"]
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "unsafe_block"
"#,
        ));
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }
}

impl Drop for SourceTreeFixture {
    fn drop(&mut self) {
        remove_temp_root(self.root.clone());
    }
}
