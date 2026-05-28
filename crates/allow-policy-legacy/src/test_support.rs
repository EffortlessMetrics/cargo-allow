use super::*;
use allow_core::{
    Finding, FindingKind, Lifecycle, Span, StructuralIdentity, normalize_snippet, stable_hash_hex,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) fn assert_current_baseline_window(lifecycle: &Lifecycle) {
    let created = lifecycle
        .created
        .as_deref()
        .and_then(SimpleDate::parse)
        .unwrap_or_else(|| std::panic::panic_any("baseline should have valid created date"));
    let expires = lifecycle
        .expires
        .as_deref()
        .and_then(SimpleDate::parse)
        .unwrap_or_else(|| std::panic::panic_any("baseline should have valid expires date"));
    let today = SimpleDate::today_utc_approx();

    assert!(
        today.add_days(-1) <= created && created <= today.add_days(1),
        "baseline created date should track the current UTC day"
    );
    assert_eq!(created.days_until(expires), BASELINE_DEBT_DEFAULT_DAYS);
}

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

pub(super) fn policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("non-rust-allowlist.toml");
    fs::write(&path, policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn non_rust_policy_with_entry(entry: &str) -> PathBuf {
    let path = fixture_dir().join("non-rust-allowlist.toml");
    let text = format!(
        r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
{entry}
"#
    );
    fs::write(&path, text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn generated_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("generated-allowlist.toml");
    fs::write(&path, generated_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn no_panic_baseline_fixture_path() -> PathBuf {
    let path = fixture_dir().join("no-panic-baseline.toml");
    fs::write(&path, no_panic_baseline_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn no_panic_allowlist_fixture_path() -> PathBuf {
    let path = fixture_dir().join("no-panic-allowlist.toml");
    fs::write(&path, no_panic_allowlist_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn clippy_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("clippy-exceptions.toml");
    fs::write(&path, clippy_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn unsafe_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("unsafe-allowlist.toml");
    fs::write(&path, unsafe_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn executable_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("executable-allowlist.toml");
    fs::write(&path, executable_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn workflow_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("workflow-allowlist.toml");
    fs::write(&path, workflow_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn dependency_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("dependency-surface-allowlist.toml");
    fs::write(&path, dependency_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn process_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("process-allowlist.toml");
    fs::write(&path, process_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn malformed_process_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("process-allowlist.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-missing"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
owner = "release/ci"
reason = "Intentionally incomplete fixture."
created = "2026-05-09"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn network_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("network-allowlist.toml");
    fs::write(&path, network_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn malformed_network_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("network-allowlist.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-missing"
destination = "crates.io"
lane = "build"
owner = "release"
reason = "Intentionally incomplete fixture."
created = "2026-05-09"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn generated_fixture_root() -> PathBuf {
    let dir = fixture_dir();
    fs::write(
            dir.join(".gitattributes"),
            "# generated files\npolicy/no-panic-baseline.toml text linguist-generated=true\nREADME.md text\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("gitattributes write: {err}")));
    dir
}

pub(super) fn workflow_fixture_root() -> PathBuf {
    let dir = fixture_dir();
    let workflows = dir.join(".github").join("workflows");
    fs::create_dir_all(&workflows)
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
    fs::write(
            workflows.join("ci.yml"),
            "name: ci\njobs:\n  test:\n    steps:\n      - uses: actions/checkout@v6.0.2\n      - uses: Swatinem/rust-cache@v2 # cache\n      # - uses: ignored/comment@v1\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow write: {err}")));
    dir
}

pub(super) fn fixture_dir() -> PathBuf {
    let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-policy-legacy-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

pub(super) fn policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "non-rust-docs"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
broad_glob_reason = "Docs are intentionally tree-shaped."
created = "2026-05-09"
expires = "permanent"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "non-rust-github-meta"
glob = ".github/**"
category = "ci_meta"
owner = "release/meta"
reason = "GitHub metadata."
broad_glob_reason = "Covers ancillary GitHub configuration."
created = "2026-05-09"
expires = "permanent"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "non-rust-github-workflows"
glob = ".github/workflows/*.yml"
category = "ci_declarative"
owner = "release/ci"
reason = "GitHub Actions workflows."
broad_glob_reason = "Workflow detail lives in a companion ledger."
created = "2026-05-09"
expires = "permanent"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "non-rust-ripr-config"
path = "ripr.toml"
category = "policy_config"
owner = "policy"
reason = "ripr configuration."
created = "2026-05-09"
expires = "permanent"
"#,
    );
    text
}

pub(super) fn generated_policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "generated-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "generated-no-panic-baseline"
path = "policy/no-panic-baseline.toml"
generator = "cargo xtask no-panic baseline --reset"
regenerate_command = "cargo xtask no-panic baseline --reset"
owner = "policy"
reason = "Generated by the no-panic classifier."
created = "2026-05-10"
expires = "permanent"
"#,
    );
    text
}

pub(super) fn no_panic_baseline_fixture_text() -> String {
    let unwrap_snippet = ["let value = maybe.", "unwrap();"].concat();
    let panic_snippet = ["panic!", "(\"bad\");"].concat();
    format!(
        r#"schema_version = 1
policy = "no-panic-baseline"
owner = "EffortlessMetrics"
status = "advisory"

[policy_config]
mode = "no-new-debt"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "{unwrap_snippet}"
count = 2

[[entry]]
path = "src/lib.rs"
family = "panic"
selector_kind = "macro-call"
selector_callee = "panic"
snippet = '{panic_snippet}'
count = 1
"#,
    )
}

pub(super) fn no_panic_allowlist_fixture_text() -> String {
    r#"schema_version = 1
policy = "no-panic-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "no-panic-unwrap"
path = "src/lib.rs"
family = "unwrap"
owner = "parser"
classification = "reviewed_panic_exception"
explanation = "Parser validates the optional value."
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "Option/Result::unwrap"
container = "load"
line_hint = 7

[allow.last_seen]
line = 7
column = 12

[[allow]]
path = "src/lib.rs"
family = "panic"

[allow.selector]
kind = "macro-call"
callee = "panic"
"#
    .to_string()
}

pub(super) fn clippy_policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "clippy-exceptions"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "clippy-unwrap-policy"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
family = "expect"
owner = "lint"
classification = "reviewed_lint_exception"
reason = "Fixture keeps an explicit lint suppression linked to policy."
policy_id = "clippy-unwrap-policy"
created = "2026-05-09"
review_after = "2026-09-09"
"#,
    );
    text
}

pub(super) fn unsafe_policy_fixture_text() -> String {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe_block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller validates pointer before read."
evidence = ["unsafe-review:docs/evidence/unsafe/read.json"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-block"
container = "read"
line_hint = 7

[allow.last_seen]
line = 7
column = 12

[[allow]]
path = "src/lib.rs"
family = "unsafe_fn"

[allow.selector]
kind = "unsafe-fn"
"#
    .to_string()
}

pub(super) fn executable_policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "executable-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "exec-package-proof"
path = "scripts/package-proof.sh"
interpreter = "bash"
owner = "release"
reason = "Release preflight aggregator."
created = "2026-05-09"
expires = "permanent"
"#,
    );
    text
}

pub(super) fn workflow_policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    text.push_str("[[entry]]\n");
    text.push_str(
        r#"path = ".github/workflows/ci.yml"
owner = "release/ci"
reason = "Primary PR correctness gate."
permissions = ["contents:read"]
secrets_used = []
external_actions = [
  "actions/checkout@v6.0.2",
  "Swatinem/rust-cache@v2",
]
created = "2026-05-09"
expires = "permanent"
"#,
    );
    text
}

pub(super) fn dependency_policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "dep-workspace-cargo-toml"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block."
dep_count_at_baseline = 22
created = "2026-05-09"
expires = "permanent"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "dep-crate-cargo-toml"
path = "crates/*/Cargo.toml"
surface = "crate_manifest"
owner = "release"
reason = "Per-crate manifests."
broad_glob_reason = "Per-crate enumeration would duplicate the workspace member list."
created = "2026-05-09"
expires = "permanent"
"#,
    );
    text
}

pub(super) fn process_policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "proc-cargo-install-cargo-deny"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
network_reach = true
called_by = [".github/workflows/ci.yml"]
owner = "release/ci"
reason = "Installs cargo-deny in the deny job."
created = "2026-05-09"
review_after = "2026-09-09"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "proc-bash-package-proof"
binary = "bash"
argv_shape = ["scripts/package-proof.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release preflight package proof; pure local checks."
created = "2026-05-09"
expires = "permanent"
"#,
    );
    text
}

pub(super) fn network_policy_fixture_text() -> String {
    let mut text = String::from(
        r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"

"#,
    );
    push_allow(
        &mut text,
        r#"id = "net-github-api"
destination = "api.github.com"
auth_required = true
auth_secret = "GITHUB_TOKEN"
lane = "release"
owner = "release/ci"
reason = "Release uploads through the GitHub API."
created = "2026-05-09"
expires = "permanent"
"#,
    );
    text
}

pub(super) fn push_allow(text: &mut String, body: &str) {
    text.push_str("[[");
    text.push_str("allow]]\n");
    text.push_str(body);
}

pub(super) fn process_policy_finding(path: &str, symbol: &str) -> Finding {
    let mut identity = StructuralIdentity::new("policy", "process_spawn");
    identity.symbol = Some(symbol.to_string());
    identity.target_fingerprint = Some(format!("process:{symbol}"));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("process_spawn".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn network_policy_finding(symbol: &str) -> Finding {
    let mut identity = StructuralIdentity::new("policy", "network_destination");
    identity.symbol = Some(symbol.to_string());
    identity.target_fingerprint = Some(format!("network:{symbol}"));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("network_destination".to_string()),
        path: PathBuf::from("policy/network-allowlist.toml"),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn panic_finding(
    path: &str,
    family: &str,
    ast_kind: &str,
    callee: Option<&str>,
    macro_name: Option<&str>,
    snippet: &str,
) -> Finding {
    let mut identity = StructuralIdentity::new("rust", ast_kind);
    identity.callee = callee.map(str::to_string);
    identity.macro_name = macro_name.map(str::to_string);
    identity.normalized_snippet_hash = Some(stable_hash_hex(&normalize_snippet(snippet)));
    Finding {
        kind: FindingKind::Panic,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn lint_finding(
    path: &str,
    family: &str,
    lint: &str,
    policy_id: Option<&str>,
) -> Finding {
    let mut identity = StructuralIdentity::new("rust", "attribute");
    identity.lint = Some(lint.to_string());
    identity.symbol = Some(format!(
        "#[expect({lint}, reason = \"policy:{}\")]",
        policy_id.unwrap_or("unlinked")
    ));
    identity.target_fingerprint = policy_id.map(|id| format!("policy:{id}"));
    Finding {
        kind: FindingKind::LintException,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn unsafe_finding(path: &str, family: &str, container: Option<&str>) -> Finding {
    let mut identity = StructuralIdentity::new("rust", family);
    identity.container = container.map(str::to_string);
    Finding {
        kind: FindingKind::Unsafe,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn finding(path: &str, ast_kind: &str) -> Finding {
    Finding {
        kind: FindingKind::NonRustFile,
        family: Some("configuration".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: String::new(),
    }
}
