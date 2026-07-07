use super::*;
use allow_core::{AllowConfig, AllowEntry};
use std::fs;
use std::path::PathBuf;

#[test]
fn legacy_migration_preserves_metadata_matrix() {
    for case in metadata_matrix_cases() {
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

        assert_eq!(entry.owner, case.expected_owner, "{} owner", case.label);
        assert_eq!(
            entry.classification, case.expected_classification,
            "{} classification",
            case.label
        );
        assert!(
            entry.reason.contains(case.expected_reason),
            "{} should preserve reason fragment `{}` in `{}`",
            case.label,
            case.expected_reason,
            entry.reason
        );
        assert_eq!(
            entry.lifecycle.created.as_deref(),
            case.expected_created,
            "{} created",
            case.label
        );
        assert_eq!(
            entry.lifecycle.review_after.as_deref(),
            case.expected_review_after,
            "{} review_after",
            case.label
        );
        assert_eq!(
            entry.lifecycle.expires.as_deref(),
            case.expected_expires,
            "{} expires",
            case.label
        );
    }
}

struct MetadataMatrixCase {
    label: &'static str,
    file_name: &'static str,
    policy_text: &'static str,
    entry_id: &'static str,
    family: Option<&'static str>,
    expected_owner: &'static str,
    expected_classification: &'static str,
    expected_reason: &'static str,
    expected_created: Option<&'static str>,
    expected_review_after: Option<&'static str>,
    expected_expires: Option<&'static str>,
}

fn metadata_matrix_cases() -> Vec<MetadataMatrixCase> {
    vec![
        MetadataMatrixCase {
            label: "generated file metadata",
            file_name: "generated-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "generated-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "metadata-generated"
path = "docs/generated/schema.json"
generator = "cargo xtask schema"
owner = "policy"
reason = "Generated schema fixture."
created = "2026-05-10"
expires = "permanent"
"#,
            entry_id: "metadata-generated",
            family: Some("generated_code"),
            expected_owner: "policy",
            expected_classification: "generated_code",
            expected_reason: "Generated schema fixture.",
            expected_created: Some("2026-05-10"),
            expected_review_after: Some("2026-05-10"),
            expected_expires: Some("never"),
        },
        MetadataMatrixCase {
            label: "workflow action metadata",
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
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"
"#,
            entry_id: "workflow-action-github-workflows-release-yml-2702d30def7815f5--actions-checkout-v4",
            family: Some("workflow_external_action"),
            expected_owner: "release/ci",
            expected_classification: "workflow_external_action",
            expected_reason: "Release workflow fixture.",
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-09-09"),
            expected_expires: Some("never"),
        },
        MetadataMatrixCase {
            label: "executable file metadata",
            file_name: "executable-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "executable-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "metadata-executable"
path = "scripts/release.sh"
interpreter = "bash"
owner = "release"
reason = "Release helper fixture."
created = "2026-05-09"
review_after = "2026-08-09"
expires = "permanent"
"#,
            entry_id: "metadata-executable",
            family: Some("executable_file"),
            expected_owner: "release",
            expected_classification: "executable_file",
            expected_reason: "Release helper fixture.",
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-08-09"),
            expected_expires: Some("never"),
        },
        MetadataMatrixCase {
            label: "dependency metadata",
            file_name: "dependency-surface-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "metadata-dependency"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block fixture."
broad_glob_reason = "Workspace manifest is the dependency boundary."
dep_count_at_baseline = 22
created = "2026-05-09"
review_after = "2026-08-09"
expires = "permanent"
"#,
            entry_id: "metadata-dependency",
            family: Some("dependency_surface"),
            expected_owner: "release",
            expected_classification: "workspace_manifest",
            expected_reason: "Workspace dependency block fixture.",
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-08-09"),
            expected_expires: Some("never"),
        },
        MetadataMatrixCase {
            label: "process metadata",
            file_name: "process-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "metadata-process"
binary = "bash"
argv_shape = ["scripts/release.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release helper fixture."
created = "2026-05-09"
review_after = "2026-08-09"
expires = "permanent"
"#,
            entry_id: "metadata-process",
            family: Some("process_spawn"),
            expected_owner: "release",
            expected_classification: "local_process",
            expected_reason: "Release helper fixture.",
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-08-09"),
            expected_expires: Some("never"),
        },
        MetadataMatrixCase {
            label: "network metadata",
            file_name: "network-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "metadata-network"
destination = "api.github.com"
auth_required = true
auth_secret = "GITHUB_TOKEN"
lane = "release"
owner = "release/ci"
reason = "Release API fixture."
created = "2026-05-09"
review_after = "2026-08-09"
expires = "permanent"
"#,
            entry_id: "metadata-network",
            family: Some("network_destination"),
            expected_owner: "release/ci",
            expected_classification: "authenticated_network",
            expected_reason: "Release API fixture.",
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-08-09"),
            expected_expires: Some("never"),
        },
        MetadataMatrixCase {
            label: "unsafe metadata",
            file_name: "unsafe-allowlist.toml",
            policy_text: r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "metadata-unsafe"
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
            entry_id: "metadata-unsafe",
            family: Some("unsafe_block"),
            expected_owner: "runtime",
            expected_classification: "reviewed_unsafe_boundary",
            expected_reason: "Caller validates pointer before read.",
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-09-09"),
            expected_expires: None,
        },
        MetadataMatrixCase {
            label: "no-panic baseline metadata",
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
owner = "parser"
reason = "Counted unwrap baseline after parser hardening."
created = "2026-05-09"
review_after = "2026-06-09"
expires = "2026-06-09"
"#,
            entry_id: "panic-baseline-0001",
            family: Some("unwrap"),
            expected_owner: "parser",
            expected_classification: "baseline_debt",
            expected_reason: "Counted unwrap baseline after parser hardening.",
            expected_created: Some("2026-05-09"),
            expected_review_after: Some("2026-06-09"),
            expected_expires: Some("2026-06-09"),
        },
    ]
}

fn write_policy(file_name: &str, text: &str) -> PathBuf {
    let path = test_support::fixture_dir().join(file_name);
    fs::write(&path, text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("matrix fixture write: {err}")));
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
