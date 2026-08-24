use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[test]
fn cargo_allow_0_2_campaign_skill_contract_is_wired() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;

    // Packaged cargo-allow tests run without the repository guidance tree. The
    // source checkout is the authority for this repository-local contract.
    if !root.join(".git").exists() {
        return Ok(());
    }

    let skill_path = ".agents/skills/cargo-allow-0.2-campaign/SKILL.md";
    let review_skill_path = ".agents/skills/review-current-head/SKILL.md";

    let skill = read(&root, skill_path)?;
    let review_skill = read(&root, review_skill_path)?;
    let agents = read(&root, "AGENTS.md")?;
    let gemini = read(&root, "GEMINI.md")?;
    let campaign = read(&root, "docs/campaigns/cargo-allow-0.2.0.md")?;

    let expected_frontmatter = concat!(
        "name: cargo-allow-0.2-campaign\n",
        "description: Implement the currently selected cargo-allow 0.2.0 campaign issue from exact live repository and issue state, stay within one semantic owner and PR lane, validate proportionally, and hand the resulting exact head to the independent review skill."
    );
    require(
        frontmatter(&skill)? == expected_frontmatter,
        "campaign skill must retain its exact canonical frontmatter",
    )?;

    for marker in [
        "ReversibleImplementation",
        "ReadOnlyReview",
        "ExternalObservation",
        "RootDecision",
        "IrreversibleOperation",
        "BlockedOrStale",
        "AGENTS.md",
        "GEMINI.md",
        "CLAUDE.md",
        "#3768",
        "selected issue",
        "current `main`",
        "open pull requests",
        "current CI",
        "local branch, status, diff, worktrees",
        "active viable pull request",
        "one semantic owner",
        "one writer",
        "full current diff",
        "exact base/head pair",
        "fresh review",
        ".agents/skills/review-current-head/SKILL.md",
        "create, move, or delete release tags",
        "publish or yank packages",
        "change live repository controls",
        "mutate an external pilot repository",
        "final release authorization",
        "never delete, recreate, or move",
        "never publish another package row",
        "never treat a tag push",
        "never continue or recover a partial release from moving",
        "never reuse RC.1 package bytes",
        "#2501",
        "#3760",
        "merged-main/current evidence",
        "cargo-intent",
        "cargo-proof",
        "model discovery",
        "Claim boundary",
    ] {
        require_contains(&skill, marker, "campaign skill")?;
    }

    let review_frontmatter = frontmatter(&review_skill)?;
    require(
        review_frontmatter.contains("name: review-current-head"),
        "independent review skill must retain its own identity",
    )?;
    require(
        frontmatter(&skill)? != review_frontmatter,
        "implementation and independent review must not collapse into one skill contract",
    )?;
    require_contains(
        &skill,
        "It does not perform independent review",
        "campaign skill",
    )?;
    require_contains(
        &review_skill,
        "independent",
        "independent review skill",
    )?;

    for (path, source) in [
        ("AGENTS.md", &agents),
        ("GEMINI.md", &gemini),
        ("docs/campaigns/cargo-allow-0.2.0.md", &campaign),
    ] {
        require_contains(
            source,
            skill_path,
            &format!("{path} implementation routing"),
        )?;
        require_contains(
            source,
            review_skill_path,
            &format!("{path} review routing"),
        )?;
    }

    for forbidden in ['\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}'] {
        require(
            !skill.contains(forbidden),
            "campaign skill contains a zero-width formatting character",
        )?;
    }

    Ok(())
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}

fn read(root: &Path, relative: &str) -> Result<String, Box<dyn Error>> {
    let path = root.join(relative);
    fs::read_to_string(&path)
        .map(|text| text.replace("\r\n", "\n"))
        .map_err(|error| {
            io::Error::other(format!("failed to read {}: {error}", path.display())).into()
        })
}

fn frontmatter(source: &str) -> Result<&str, Box<dyn Error>> {
    let rest = source
        .strip_prefix("---\n")
        .ok_or_else(|| io::Error::other("skill must start with YAML frontmatter"))?;
    rest.split_once("\n---\n")
        .map(|(frontmatter, _)| frontmatter)
        .ok_or_else(|| io::Error::other("skill frontmatter must have a closing delimiter").into())
}

fn require_contains(haystack: &str, needle: &str, owner: &str) -> Result<(), Box<dyn Error>> {
    let normalized_haystack = normalize_whitespace(haystack);
    let normalized_needle = normalize_whitespace(needle);
    require(
        normalized_haystack.contains(&normalized_needle),
        &format!("{owner} is missing required campaign-contract marker: {needle}"),
    )
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn require(condition: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message.to_string()).into())
    }
}
