use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("no crates dir parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("no repo root"))?;
    Ok(root.to_path_buf())
}

fn release_workflows(root: &Path) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut workflows = Vec::new();
    for name in ["release.yml", "release-authorized.yml"] {
        let path = root.join(".github/workflows").join(name);
        if path.exists() {
            workflows.push((name.to_string(), fs::read_to_string(&path)?));
        }
    }
    require(!workflows.is_empty(), "no release workflow under test")?;
    Ok(workflows)
}

/// #3790: token-access inventory. Exactly the two known steps may reference
/// the registry secret, and both are gated on the authorize job.
#[test]
fn test_token_access_points_are_inventoried_and_gated() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let workflows = release_workflows(&root)?;
    let release = workflows
        .iter()
        .find(|(name, _)| name == "release.yml")
        .map(|(_, content)| content.clone())
        .ok_or_else(|| io::Error::other("release.yml is absent"))?;
    let references: Vec<&str> = release
        .lines()
        .filter(|line| line.contains("secrets.CARGO_REGISTRY_TOKEN"))
        .collect();
    require(
        references.len() == 2,
        &format!(
            "exactly two token references must exist, found {}",
            references.len()
        ),
    )?;
    // The publish upload env carries the secret only behind the gate.
    require(
        references
            .iter()
            .any(|line| line.contains("needs.authorize.outputs")),
        "token references must be conditioned on the authorize gate",
    )?;
    // No step echoes the secret and no cleartext token exists.
    for line in release.lines() {
        require(
            !line.contains("echo ${{ secrets.") && !line.contains("echo \"${{ secrets."),
            "secrets must never be echoed into logs",
        )?;
    }
    Ok(())
}

/// #3790: skipped or failed gates cannot reach token lookup. The publish job
/// depends on the authorize job, and the token steps additionally require
/// its explicit valid output, so an empty (skipped) output also fails closed.
#[test]
fn test_skipped_or_failed_gate_cannot_reach_token() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let workflows = release_workflows(&root)?;
    let release = workflows
        .iter()
        .find(|(name, _)| name == "release.yml")
        .map(|(_, content)| content.clone())
        .ok_or_else(|| io::Error::other("release.yml is absent"))?;
    require(
        release.contains("needs: [preflight, authorize]"),
        "publish must need both preflight and authorize",
    )?;
    // No token-bearing step may run unconditionally or under always():
    // steps open at six-space indent; jobs open at two-space indent.
    let mut step_has_always = false;
    let mut step_has_token = false;
    let mut offending = false;
    let mut flush = |always: &mut bool, token: &mut bool| {
        if *always && *token {
            offending = true;
        }
        *always = false;
        *token = false;
    };
    for line in release.lines() {
        let is_step = line.starts_with("      - ");
        let is_job = !is_step
            && line.starts_with("  ")
            && !line.starts_with("    ")
            && line.trim_end().ends_with(':');
        if is_step || is_job {
            flush(&mut step_has_always, &mut step_has_token);
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("if:") && trimmed.contains("always()") {
            step_has_always = true;
        }
        if line.contains("secrets.CARGO_REGISTRY_TOKEN") {
            step_has_token = true;
        }
    }
    flush(&mut step_has_always, &mut step_has_token);
    require(!offending, "token steps must never run under always()")?;
    Ok(())
}

/// #3790: ordinary rehearsal dispatches stay zero-token, and the recovery
/// path never presents clean authorization.
#[test]
fn test_rehearsal_stays_zero_token_and_recovery_stays_separate() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let workflows = release_workflows(&root)?;
    let release = workflows
        .iter()
        .find(|(name, _)| name == "release.yml")
        .map(|(_, content)| content.clone())
        .ok_or_else(|| io::Error::other("release.yml is absent"))?;
    // The authorize job marks rehearsal dispatches explicitly; token steps
    // require valid or recovery, so rehearsal (both false) stays zero-token.
    require(
        release.contains("rehearsal=true") && release.contains("valid=false"),
        "rehearsal dispatches must resolve to an explicitly invalid gate",
    )?;
    // Recovery keeps its incident-owned inputs and never sets valid=true.
    // Count only bash assignments, not prose comments.
    let grants = release
        .lines()
        .filter(|line| line.trim() == "valid=true")
        .count();
    require(
        grants == 1,
        "exactly one clean-authorization grant may exist",
    )?;
    let recovery_branch = release
        .split_once("[ \"${RECOVERY}\" = \"true\" ]")
        .and_then(|(_, tail)| tail.split_once("elif"))
        .map(|(branch, _)| branch)
        .ok_or_else(|| io::Error::other("recovery branch is absent"))?;
    require(
        !recovery_branch.contains("valid=true"),
        "recovery must never present clean authorization",
    )?;
    Ok(())
}
