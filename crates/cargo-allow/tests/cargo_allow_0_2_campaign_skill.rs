use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("cargo-allow crate should be nested below the workspace root")
        .to_path_buf()
}

fn read_workspace_file(path: &str) -> String {
    let full_path = workspace_root().join(path);
    fs::read_to_string(&full_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", full_path.display()))
}

fn frontmatter(source: &str) -> &str {
    let rest = source
        .strip_prefix("---\n")
        .expect("skill must start with YAML frontmatter");
    rest.split_once("\n---\n")
        .map(|(frontmatter, _)| frontmatter)
        .expect("skill frontmatter must have a closing delimiter")
}

#[test]
fn cargo_allow_0_2_campaign_skill_contract_is_wired() {
    let skill_path = ".agents/skills/cargo-allow-0.2-campaign/SKILL.md";
    let review_skill_path = ".agents/skills/review-current-head/SKILL.md";

    let skill = read_workspace_file(skill_path);
    let review_skill = read_workspace_file(review_skill_path);
    let agents = read_workspace_file("AGENTS.md");
    let gemini = read_workspace_file("GEMINI.md");
    let campaign = read_workspace_file("docs/campaigns/cargo-allow-0.2.0.md");

    let expected_frontmatter = concat!(
        "name: cargo-allow-0.2-campaign\n",
        "description: Implement the currently selected cargo-allow 0.2.0 campaign issue from exact live repository and issue state, stay within one semantic owner and PR lane, validate proportionally, and hand the resulting exact head to the independent review skill."
    );
    assert_eq!(frontmatter(&skill), expected_frontmatter);

    for marker in [
        "ReversibleImplementation",
        "ReadOnlyReview",
        "ExternalObservation",
        "RootDecision",
        "IrreversibleOperation",
        "AGENTS.md",
        "GEMINI.md",
        "#3768",
        "selected issue",
        "current `main`",
        "open pull requests",
        "one semantic owner",
        "one writer",
        "full current diff",
        "exact base/head pair",
        "fresh review",
        "current CI",
        ".agents/skills/review-current-head/SKILL.md",
        "create, move, or delete release tags",
        "publish or yank packages",
        "change live repository controls",
        "mutate an external pilot repository",
        "final release authorization",
        "Claim boundary",
    ] {
        assert!(
            skill.contains(marker),
            "campaign skill is missing required contract marker: {marker}"
        );
    }

    let review_frontmatter = frontmatter(&review_skill);
    assert!(
        review_frontmatter.contains("name: review-current-head"),
        "independent review skill must retain its own identity"
    );
    assert_ne!(
        frontmatter(&skill),
        review_frontmatter,
        "implementation and independent review must not collapse into one skill contract"
    );
    assert!(
        skill.contains("It does not perform independent review"),
        "implementation skill must disclaim independent review authority"
    );
    assert!(
        review_skill.contains("independent"),
        "review skill must remain explicitly independent"
    );

    for (path, source) in [
        ("AGENTS.md", &agents),
        ("GEMINI.md", &gemini),
        ("docs/campaigns/cargo-allow-0.2.0.md", &campaign),
    ] {
        assert!(
            source.contains(skill_path),
            "{path} must route implementation work through the campaign skill"
        );
        assert!(
            source.contains(review_skill_path),
            "{path} must keep the independent review handoff visible"
        );
    }

    for forbidden in ['\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}'] {
        assert!(
            !skill.contains(forbidden),
            "campaign skill contains a zero-width formatting character"
        );
    }
}
