use allow_core::AllowConfig;

use crate::{parse_policy, render_policy};

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
    assert!(reparsed.requirements.unsafe_safety_comment_required);
}
