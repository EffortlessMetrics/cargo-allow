use super::*;
use allow_core::{AllowConfig, AllowEntry};
use std::fs;
use std::path::PathBuf;

#[test]
fn legacy_migration_preserves_evidence_matrix() {
    let cases = evidence_matrix_cases();

    for case in cases {
        let migrated = load_legacy_or_canonical(write_policy(case.file_name, case.policy_text));
        assert!(
            migrated.is_ok(),
            "{} policy should migrate: {:?}",
            case.label,
            migrated.as_ref().err()
        );
        let cfg = match migrated {
            Ok(cfg) => cfg,
            Err(_) => continue,
        };
        let entry = find_entry(&cfg, case.entry_id, case.family);
        assert!(
            entry.is_some(),
            "expected migrated entry {} with family {:?}",
            case.entry_id,
            case.family
        );
        let Some(entry) = entry else {
            continue;
        };

        assert_evidence_contains(case.label, entry, case.expected_evidence);
        assert_evidence_contains(case.label, entry, case.expected_derived_evidence);
        assert_links_contains(case.label, entry, case.expected_links);
    }
}

struct EvidenceMatrixCase {
    label: &'static str,
    file_name: &'static str,
    policy_text: &'static str,
    entry_id: &'static str,
    family: Option<&'static str>,
    expected_evidence: &'static [&'static str],
    expected_derived_evidence: &'static [&'static str],
    expected_links: &'static [&'static str],
}

fn evidence_matrix_cases() -> Vec<EvidenceMatrixCase> {
    vec![
        EvidenceMatrixCase {
            label: "non-rust evidence",
            file_name: "non-rust-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-non-rust"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
broad_glob_reason = "Docs are intentionally tree-shaped."
evidence = ["doc:docs/source-exception-ledger.md", "issue:#123"]
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-non-rust",
            family: None,
            expected_evidence: &["doc:docs/source-exception-ledger.md", "issue:#123"],
            expected_derived_evidence: &[],
            expected_links: &[],
        },
        EvidenceMatrixCase {
            label: "non-rust covered_by",
            file_name: "non-rust-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-non-rust-covered"
glob = ".github/workflows/*.yml"
category = "ci_declarative"
owner = "release/ci"
reason = "GitHub workflow files are reviewed CI policy."
broad_glob_reason = "Workflow files are all declarative CI surfaces."
covered_by = "doc:docs/ci.md"
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-non-rust-covered",
            family: None,
            expected_evidence: &["doc:docs/ci.md"],
            expected_derived_evidence: &[],
            expected_links: &[],
        },
        EvidenceMatrixCase {
            label: "generated covered_by",
            file_name: "generated-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "generated-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-generated"
path = "docs/generated/schema.json"
generator = "cargo xtask schema"
regenerate_command = "cargo xtask schema"
owner = "policy"
reason = "Generated schema fixture."
covered_by = "doc:docs/schemas/README.md"
created = "2026-05-10"
expires = "permanent"
"#,
            entry_id: "matrix-generated",
            family: Some("generated_code"),
            expected_evidence: &["doc:docs/schemas/README.md"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-generated",
                "generator:cargo xtask schema",
                "cargo:cargo xtask schema",
            ],
            expected_links: &["legacy-policy:matrix-generated"],
        },
        EvidenceMatrixCase {
            label: "generated evidence",
            file_name: "generated-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "generated-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-generated-evidence"
path = "docs/generated/api.json"
generator = "cargo xtask api-schema"
regenerate_command = "cargo xtask api-schema"
owner = "policy"
reason = "Generated API schema fixture."
evidence = ["doc:docs/api-schema.md", "issue:#567"]
created = "2026-05-10"
expires = "permanent"
"#,
            entry_id: "matrix-generated-evidence",
            family: Some("generated_code"),
            expected_evidence: &["doc:docs/api-schema.md", "issue:#567"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-generated-evidence",
                "generator:cargo xtask api-schema",
                "cargo:cargo xtask api-schema",
            ],
            expected_links: &["legacy-policy:matrix-generated-evidence"],
        },
        EvidenceMatrixCase {
            label: "executable covered_by",
            file_name: "executable-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "executable-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-executable"
path = "scripts/release.sh"
interpreter = "bash"
owner = "release"
reason = "Release helper fixture."
covered_by = "doc:docs/release/README.md"
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-executable",
            family: Some("executable_file"),
            expected_evidence: &["doc:docs/release/README.md"],
            expected_derived_evidence: &["legacy-policy:matrix-executable", "interpreter:bash"],
            expected_links: &["legacy-policy:matrix-executable"],
        },
        EvidenceMatrixCase {
            label: "executable evidence",
            file_name: "executable-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "executable-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-executable-evidence"
path = "scripts/package.sh"
interpreter = "bash"
owner = "release"
reason = "Package helper fixture."
evidence = ["doc:docs/release/package.md", "issue:#678"]
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-executable-evidence",
            family: Some("executable_file"),
            expected_evidence: &["doc:docs/release/package.md", "issue:#678"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-executable-evidence",
                "interpreter:bash",
            ],
            expected_links: &["legacy-policy:matrix-executable-evidence"],
        },
        EvidenceMatrixCase {
            label: "workflow evidence",
            file_name: "workflow-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = ".github/workflows/release.yml"
owner = "release/ci"
reason = "Release workflow fixture."
permissions = ["contents:read"]
secrets_used = ["RELEASE_TOKEN"]
external_actions = ["actions/checkout@v4"]
evidence = ["doc:docs/ci.md", "issue:#456"]
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "workflow-file-github-workflows-release-yml-2702d30def7815f5",
            family: Some("github_workflow"),
            expected_evidence: &["doc:docs/ci.md", "issue:#456"],
            expected_derived_evidence: &[
                "legacy-policy:workflow:.github/workflows/release.yml",
                "permission:contents:read",
                "secret:RELEASE_TOKEN",
            ],
            expected_links: &["legacy-policy:workflow:.github/workflows/release.yml"],
        },
        EvidenceMatrixCase {
            label: "workflow action evidence",
            file_name: "workflow-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = ".github/workflows/release.yml"
owner = "release/ci"
reason = "Release workflow fixture."
permissions = ["contents:read"]
secrets_used = []
external_actions = ["actions/checkout@v4"]
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "workflow-action-github-workflows-release-yml-2702d30def7815f5--actions-checkout-v4",
            family: Some("workflow_external_action"),
            expected_evidence: &["doc:docs/ci.md"],
            expected_derived_evidence: &[
                "legacy-policy:workflow:.github/workflows/release.yml",
                "external_action:actions/checkout@v4",
            ],
            expected_links: &["legacy-policy:workflow:.github/workflows/release.yml"],
        },
        EvidenceMatrixCase {
            label: "workflow covered_by",
            file_name: "workflow-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = ".github/workflows/release.yml"
owner = "release/ci"
reason = "Release workflow fixture."
permissions = ["contents:read"]
secrets_used = ["RELEASE_TOKEN"]
external_actions = ["actions/checkout@v4"]
covered_by = "doc:docs/workflows/release.md"
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "workflow-file-github-workflows-release-yml-2702d30def7815f5",
            family: Some("github_workflow"),
            expected_evidence: &["doc:docs/workflows/release.md"],
            expected_derived_evidence: &[
                "legacy-policy:workflow:.github/workflows/release.yml",
                "permission:contents:read",
                "secret:RELEASE_TOKEN",
            ],
            expected_links: &["legacy-policy:workflow:.github/workflows/release.yml"],
        },
        EvidenceMatrixCase {
            label: "workflow action covered_by",
            file_name: "workflow-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = ".github/workflows/release.yml"
owner = "release/ci"
reason = "Release workflow fixture."
permissions = []
secrets_used = []
external_actions = ["actions/checkout@v4"]
covered_by = "doc:docs/workflows/actions.md"
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "workflow-action-github-workflows-release-yml-2702d30def7815f5--actions-checkout-v4",
            family: Some("workflow_external_action"),
            expected_evidence: &["doc:docs/workflows/actions.md"],
            expected_derived_evidence: &[
                "legacy-policy:workflow:.github/workflows/release.yml",
                "external_action:actions/checkout@v4",
            ],
            expected_links: &["legacy-policy:workflow:.github/workflows/release.yml"],
        },
        EvidenceMatrixCase {
            label: "dependency covered_by",
            file_name: "dependency-surface-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-dependency"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block fixture."
dep_count_at_baseline = 22
covered_by = "doc:docs/dependencies.md"
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-dependency",
            family: Some("dependency_surface"),
            expected_evidence: &["doc:docs/dependencies.md"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-dependency",
                "surface:workspace_manifest",
                "dep_count_at_baseline:22",
            ],
            expected_links: &["legacy-policy:matrix-dependency"],
        },
        EvidenceMatrixCase {
            label: "dependency evidence",
            file_name: "dependency-surface-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-dependency-evidence"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block fixture."
dep_count_at_baseline = 22
evidence = ["doc:docs/dependencies.md", "issue:#234"]
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-dependency-evidence",
            family: Some("dependency_surface"),
            expected_evidence: &["doc:docs/dependencies.md", "issue:#234"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-dependency-evidence",
                "surface:workspace_manifest",
                "dep_count_at_baseline:22",
            ],
            expected_links: &["legacy-policy:matrix-dependency-evidence"],
        },
        EvidenceMatrixCase {
            label: "process evidence",
            file_name: "process-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-process"
binary = "bash"
argv_shape = ["scripts/release.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release helper fixture."
evidence = ["doc:docs/release/README.md", "issue:#789"]
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-process",
            family: Some("process_spawn"),
            expected_evidence: &["doc:docs/release/README.md", "issue:#789"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-process",
                "binary:bash",
                "argv_shape:scripts/release.sh",
                "network_reach:false",
                "called_by:.github/workflows/release.yml",
            ],
            expected_links: &["legacy-policy:matrix-process"],
        },
        EvidenceMatrixCase {
            label: "process covered_by",
            file_name: "process-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-process-covered"
binary = "bash"
argv_shape = ["scripts/release.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release helper fixture."
covered_by = "doc:docs/release/process.md"
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-process-covered",
            family: Some("process_spawn"),
            expected_evidence: &["doc:docs/release/process.md"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-process-covered",
                "binary:bash",
                "argv_shape:scripts/release.sh",
                "network_reach:false",
                "called_by:.github/workflows/release.yml",
            ],
            expected_links: &["legacy-policy:matrix-process-covered"],
        },
        EvidenceMatrixCase {
            label: "network covered_by",
            file_name: "network-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-network"
destination = "api.github.com"
auth_required = true
auth_secret = "GITHUB_TOKEN"
lane = "release"
owner = "release/ci"
reason = "Release API fixture."
covered_by = "doc:docs/ci.md"
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-network",
            family: Some("network_destination"),
            expected_evidence: &["doc:docs/ci.md"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-network",
                "destination:api.github.com",
                "lane:release",
                "auth_required:true",
                "auth_secret:GITHUB_TOKEN",
            ],
            expected_links: &["legacy-policy:matrix-network"],
        },
        EvidenceMatrixCase {
            label: "network evidence",
            file_name: "network-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-network-evidence"
destination = "api.github.com"
auth_required = true
auth_secret = "GITHUB_TOKEN"
lane = "release"
owner = "release/ci"
reason = "Release API fixture."
evidence = ["doc:docs/network.md", "issue:#345"]
created = "2026-05-09"
expires = "permanent"
"#,
            entry_id: "matrix-network-evidence",
            family: Some("network_destination"),
            expected_evidence: &["doc:docs/network.md", "issue:#345"],
            expected_derived_evidence: &[
                "legacy-policy:matrix-network-evidence",
                "destination:api.github.com",
                "lane:release",
                "auth_required:true",
                "auth_secret:GITHUB_TOKEN",
            ],
            expected_links: &["legacy-policy:matrix-network-evidence"],
        },
        EvidenceMatrixCase {
            label: "clippy evidence",
            file_name: "clippy-exceptions.toml",
            policy_text: r#"schema_version = 1
policy = "clippy-exceptions"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-clippy-evidence"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
evidence = ["test:lint_policy_is_linked", "issue:#123"]
"#,
            entry_id: "matrix-clippy-evidence",
            family: Some("expect_attribute"),
            expected_evidence: &["test:lint_policy_is_linked", "issue:#123"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:matrix-clippy-evidence"],
        },
        EvidenceMatrixCase {
            label: "clippy covered_by",
            file_name: "clippy-exceptions.toml",
            policy_text: r#"schema_version = 1
policy = "clippy-exceptions"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-clippy"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
covered_by = "test:lint_policy_covered"
"#,
            entry_id: "matrix-clippy",
            family: Some("expect_attribute"),
            expected_evidence: &["test:lint_policy_covered"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:matrix-clippy"],
        },
        EvidenceMatrixCase {
            label: "no-panic evidence",
            file_name: "no-panic-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "no-panic-allowlist"

[[allow]]
id = "matrix-no-panic"
path = "src/lib.rs"
family = "panic"
explanation = "Crash path is unreachable after argument validation."
evidence = ["test:panic_path_unreachable"]

[allow.selector]
kind = "macro-call"
callee = "panic"
"#,
            entry_id: "matrix-no-panic",
            family: Some("panic_macro"),
            expected_evidence: &["test:panic_path_unreachable"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:no-panic-allowlist"],
        },
        EvidenceMatrixCase {
            label: "no-panic covered_by",
            file_name: "no-panic-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "no-panic-allowlist"

[[allow]]
id = "matrix-no-panic-covered"
path = "src/lib.rs"
family = "unwrap"
reason = "Parser validates optional value."
covered_by = "test:parser_validates_optional_value"

[allow.selector]
kind = "method-call"
callee = "unwrap"
"#,
            entry_id: "matrix-no-panic-covered",
            family: Some("unwrap"),
            expected_evidence: &["test:parser_validates_optional_value"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:no-panic-allowlist"],
        },
        EvidenceMatrixCase {
            label: "no-panic baseline evidence",
            file_name: "no-panic-baseline.toml",
            policy_text: r#"schema_version = 1
policy = "no-panic-baseline"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "unwrap"
snippet = "value.unwrap()"
count = 2
evidence = ["test:parser_validates_optional_value", "issue:#123"]
"#,
            entry_id: "panic-baseline-0001",
            family: Some("unwrap"),
            expected_evidence: &["test:parser_validates_optional_value", "issue:#123"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:no-panic-baseline"],
        },
        EvidenceMatrixCase {
            label: "no-panic baseline covered_by",
            file_name: "no-panic-baseline.toml",
            policy_text: r#"schema_version = 1
policy = "no-panic-baseline"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "unwrap"
snippet = "value.unwrap()"
count = 2
covered_by = "test:baseline_unwrap_counted"
"#,
            entry_id: "panic-baseline-0001",
            family: Some("unwrap"),
            expected_evidence: &["test:baseline_unwrap_counted"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:no-panic-baseline"],
        },
        EvidenceMatrixCase {
            label: "unsafe evidence",
            file_name: "unsafe-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-unsafe"
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
"#,
            entry_id: "matrix-unsafe",
            family: Some("unsafe_block"),
            expected_evidence: &["unsafe-review:docs/evidence/unsafe/read.json"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:matrix-unsafe"],
        },
        EvidenceMatrixCase {
            label: "unsafe covered_by",
            file_name: "unsafe-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "matrix-unsafe-covered"
path = "src/lib.rs"
family = "unsafe_fn"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller validates FFI boundary before call."
covered_by = "unsafe-review:docs/evidence/unsafe/ffi.json"
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-fn"
container = "ffi_call"
"#,
            entry_id: "matrix-unsafe-covered",
            family: Some("unsafe_fn"),
            expected_evidence: &["unsafe-review:docs/evidence/unsafe/ffi.json"],
            expected_derived_evidence: &[],
            expected_links: &["legacy-policy:matrix-unsafe-covered"],
        },
    ]
}

fn write_policy(file_name: &str, text: &str) -> PathBuf {
    let path = test_support::fixture_dir().join(file_name);
    let written = fs::write(&path, text);
    assert!(
        written.is_ok(),
        "matrix fixture should write {}",
        path.display()
    );
    path
}

fn find_entry<'a>(
    cfg: &'a AllowConfig,
    entry_id: &str,
    family: Option<&str>,
) -> Option<&'a AllowEntry> {
    cfg.allow.iter().find(|entry| {
        entry.id == entry_id && family.is_none_or(|family| entry.family.as_deref() == Some(family))
    })
}

fn assert_evidence_contains(label: &str, entry: &AllowEntry, expected: &[&str]) {
    for value in expected {
        assert!(
            entry.evidence.iter().any(|item| item == value),
            "{label} should preserve evidence `{value}` in {:?}",
            entry.evidence
        );
    }
}

fn assert_links_contains(label: &str, entry: &AllowEntry, expected: &[&str]) {
    for value in expected {
        assert!(
            entry.links.iter().any(|item| item == value),
            "{label} should preserve link `{value}` in {:?}",
            entry.links
        );
    }
}
