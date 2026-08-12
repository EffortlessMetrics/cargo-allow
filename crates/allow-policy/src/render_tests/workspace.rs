use allow_core::{AllowConfig, FileFamilyRule};

use crate::render_sections::{render_policy_header, render_requirements, render_workspace};
use crate::{parse_policy, render_policy};

#[test]
fn render_policy_header_call_presence_observer() {
    let mut cfg = AllowConfig::empty();
    cfg.schema_version = "0.1".to_string();
    cfg.policy = "cargo-allow".to_string();
    cfg.owner = Some("core/\"policy\"".to_string());
    cfg.status = Some("shadow\\mode".to_string());
    let mut rendered = String::new();

    render_policy_header(&mut rendered, &cfg);

    assert_eq!(
        rendered,
        "schema_version = \"0.1\"\n\
policy = \"cargo-allow\"\n\
owner = \"core/\\\"policy\\\"\"\n\
status = \"shadow\\\\mode\"\n\n"
    );
}

#[test]
fn render_workspace_call_presence_observer() {
    let mut cfg = AllowConfig::empty();
    cfg.workspace.root = "fixtures/source tree".to_string();
    cfg.workspace.inventory = "git-tracked".to_string();
    cfg.workspace.default_mode = "no-new".to_string();
    cfg.workspace.ignored = vec!["target/**".to_string(), "vendor/\"old\"/**".to_string()];
    cfg.workspace.generated = vec!["generated/**".to_string(), "snapshots\\tmp/**".to_string()];
    let mut rendered = String::new();

    render_workspace(&mut rendered, &cfg.workspace);

    assert_eq!(
        rendered,
        "[workspace]\n\
root = \"fixtures/source tree\"\n\
inventory = \"git-tracked\"\n\
default_mode = \"no-new\"\n\
ignored = [\"target/**\", \"vendor/\\\"old\\\"/**\"]\n\
generated = [\"generated/**\", \"snapshots\\\\tmp/**\"]\n\n"
    );
}

#[test]
fn render_workspace_emits_custom_file_family_rules() {
    let mut cfg = AllowConfig::empty();
    cfg.workspace.file_families.push(FileFamilyRule {
        id: "model-artifact".to_string(),
        family: "ml_model".to_string(),
        glob: "models/**/*.onnx".to_string(),
        reason: "Govern model artifacts with a stable family.".to_string(),
    });

    let rendered = render_policy(&cfg);
    for expected in [
        "[[workspace.file_family]]",
        "id = \"model-artifact\"",
        "family = \"ml_model\"",
        "glob = \"models/**/*.onnx\"",
        "reason = \"Govern model artifacts with a stable family.\"",
    ] {
        assert!(
            rendered.contains(expected),
            "missing `{expected}`:\n{rendered}"
        );
    }

    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert_eq!(
        reparsed.workspace.file_families,
        cfg.workspace.file_families
    );
}

#[test]
fn render_requirements_call_presence_observer() {
    let mut cfg = AllowConfig::empty();
    cfg.requirements.owner_required = true;
    cfg.requirements.reason_required = true;
    cfg.requirements.classification_required = true;
    cfg.requirements.evidence_required = true;
    cfg.requirements.expires_or_review_after_required = true;
    cfg.requirements.allow_bare_allow_attributes = true;
    cfg.requirements.lint_policy_id_required = true;
    cfg.requirements.stale_entries_fail = true;
    cfg.requirements.unsafe_evidence_required = true;
    cfg.requirements.unsafe_verified_evidence_required = true;
    cfg.requirements.unsafe_safety_comment_required = true;
    let mut rendered = String::new();

    render_requirements(&mut rendered, &cfg.requirements);

    assert_eq!(
        rendered,
        "[requirements]\n\
owner_required = true\n\
reason_required = true\n\
classification_required = true\n\
evidence_required = true\n\
expires_or_review_after_required = true\n\
allow_bare_allow_attributes = true\n\
lint_policy_id_required = true\n\
stale_entries_fail = true\n\n\
[requirements.unsafe]\n\
evidence_required = true\n\
verified_evidence_required = true\n\
safety_comment_required = true\n"
    );
}

#[test]
fn renders_and_parses_general_evidence_requirement() {
    let mut cfg = AllowConfig::empty();
    cfg.requirements.evidence_required = true;

    let rendered = render_policy(&cfg);
    assert!(rendered.contains("evidence_required = true"));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert!(reparsed.requirements.evidence_required);
}

#[test]
fn renders_and_parses_source_tree_settings() {
    let mut cfg = AllowConfig::empty();
    cfg.owner = Some("core/policy".to_string());
    cfg.status = Some("advisory".to_string());
    cfg.workspace.root = "fixtures/source-tree".to_string();
    cfg.workspace.inventory = "git-tracked".to_string();
    cfg.workspace.default_mode = "strict".to_string();
    cfg.workspace.ignored = vec![".git/**".to_string(), "target/**".to_string()];
    cfg.workspace.generated = vec!["target/**".to_string(), "vendor/**".to_string()];
    cfg.requirements.allow_bare_allow_attributes = true;
    cfg.requirements.lint_policy_id_required = true;
    cfg.requirements.stale_entries_fail = true;
    cfg.requirements.unsafe_verified_evidence_required = true;
    cfg.requirements.unsafe_safety_comment_required = true;

    let rendered = render_policy(&cfg);
    for expected in [
        "owner = \"core/policy\"",
        "status = \"advisory\"",
        "root = \"fixtures/source-tree\"",
        "inventory = \"git-tracked\"",
        "default_mode = \"strict\"",
        "ignored = [\".git/**\", \"target/**\"]",
        "generated = [\"target/**\", \"vendor/**\"]",
        "allow_bare_allow_attributes = true",
        "lint_policy_id_required = true",
        "stale_entries_fail = true",
        "[requirements.unsafe]",
        "verified_evidence_required = true",
        "safety_comment_required = true",
    ] {
        assert!(
            rendered.contains(expected),
            "rendered policy should contain `{expected}`:\n{rendered}"
        );
    }

    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert_eq!(reparsed.owner.as_deref(), Some("core/policy"));
    assert_eq!(reparsed.status.as_deref(), Some("advisory"));
    assert_eq!(reparsed.workspace.root, "fixtures/source-tree");
    assert_eq!(reparsed.workspace.inventory, "git-tracked");
    assert_eq!(reparsed.workspace.default_mode, "strict");
    assert_eq!(reparsed.workspace.ignored, [".git/**", "target/**"]);
    assert_eq!(reparsed.workspace.generated, ["target/**", "vendor/**"]);
    assert!(reparsed.requirements.allow_bare_allow_attributes);
    assert!(reparsed.requirements.lint_policy_id_required);
    assert!(reparsed.requirements.stale_entries_fail);
    assert!(reparsed.requirements.unsafe_evidence_required);
    assert!(reparsed.requirements.unsafe_verified_evidence_required);
    assert!(reparsed.requirements.unsafe_safety_comment_required);
}
