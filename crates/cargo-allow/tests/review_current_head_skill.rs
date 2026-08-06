use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[test]
fn current_head_review_skill_contract_is_wired() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;

    // Packaged cargo-allow tests run without the repository guidance tree. The
    // source checkout is the authority for this repository-local contract.
    if !root.join(".git").exists() {
        return Ok(());
    }

    let skill = read(&root, ".agents/skills/review-current-head/SKILL.md")?;
    require(
        skill.starts_with("---\nname: review-current-head\n"),
        "review-current-head skill must retain its canonical front matter",
    )?;
    for marker in [
        "description: Review or re-review an open pull request's exact current base/head pair",
        "## Trigger",
        "## Independence and Correlated Failure",
        "## Inputs",
        "## Review Procedure",
        "## Review Passes",
        "## CI and External Evidence",
        "## Finding Contract",
        "## Posting Discipline",
        "## Re-review After Repairs",
        "## Merge Readiness",
        "## Failure Conditions",
        "## Output",
        "## Claim Boundary",
        "Reviewer identity alone does not create independence.",
        "Independence is designed into fresh evidence and controls",
        "If the live head changes during the review, stop.",
        "If the base SHA or merge-base changes",
        "When a provider evaluates a synthetic merge or queue commit",
        "Re-fetch those identities immediately before submitting the review.",
        "DuplicateOrSuperseded",
        "StaleAfterHeadChange",
        "StaleAfterBaseChange",
        "Green CI alone is not merge readiness.",
    ] {
        require_contains(&skill, marker, "canonical review skill")?;
    }

    let agents = read(&root, "AGENTS.md")?;
    require_contains(
        &agents,
        ".agents/skills/review-current-head/SKILL.md",
        "AGENTS.md",
    )?;
    require_contains(
        &agents,
        "If the reviewer pushes a repair, the prior review is stale",
        "AGENTS.md",
    )?;
    require_contains(
        &agents,
        "Green CI alone is not merge readiness.",
        "AGENTS.md",
    )?;

    let contributing = read(&root, "CONTRIBUTING.md")?;
    require_contains(
        &contributing,
        "## Current-Head Review and Merge Readiness",
        "CONTRIBUTING.md",
    )?;
    require_contains(
        &contributing,
        ".agents/skills/review-current-head/SKILL.md",
        "CONTRIBUTING.md",
    )?;
    require_contains(
        &contributing,
        "Any author or repair commit makes the affected review evidence stale.",
        "CONTRIBUTING.md",
    )?;

    let template = read(&root, ".github/PULL_REQUEST_TEMPLATE.md")?;
    for marker in [
        "## Controlling authority and reviewer focus",
        "Exact head intended for review",
        "Highest-risk invariants or failure modes",
        "## Review readiness",
        "Final reviewed base and merge-base:",
        "Final reviewed head:",
        "Final review source and independence posture:",
        "Required-check and unresolved-thread disposition:",
    ] {
        require_contains(&template, marker, "pull request template")?;
    }

    let operating_model = read(&root, "docs/source-of-truth/agent-operating-model.md")?;
    require_contains(
        &operating_model,
        "## Current-Head Review",
        "agent operating model",
    )?;
    require_contains(
        &operating_model,
        ".agents/skills/review-current-head/SKILL.md",
        "agent operating model",
    )?;

    let posture = read(&root, "docs/how-to/review-pr-posture.md")?;
    require_contains(
        &posture,
        "This guide supplies the source-exception posture dimension only.",
        "PR posture guide",
    )?;

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
    fs::read_to_string(&path).map_err(|error| {
        io::Error::other(format!("failed to read {}: {error}", path.display())).into()
    })
}

fn require_contains(haystack: &str, needle: &str, owner: &str) -> Result<(), Box<dyn Error>> {
    require(
        haystack.contains(needle),
        &format!("{owner} is missing required review-contract marker: {needle}"),
    )
}

fn require(condition: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message.to_string()).into())
    }
}
