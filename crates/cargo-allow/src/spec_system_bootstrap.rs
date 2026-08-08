use super::*;

pub(super) struct SpecSystemBootstrapFile {
    pub(super) path: PathBuf,
    pub(super) contents: String,
}

pub(super) fn spec_system_bootstrap_files(
    config_path: &Path,
    legacy_compatibility: bool,
) -> Vec<SpecSystemBootstrapFile> {
    let mut files = vec![
        bootstrap_file(
            config_path,
            spec_system_config_template(legacy_compatibility),
        ),
        bootstrap_file(
            Path::new(DEFAULT_OWNED_ARTIFACT_LEDGER),
            doc_artifacts_template(),
        ),
        bootstrap_file(
            Path::new("docs/proposals/README.md"),
            artifact_root_readme("Proposals", "why work exists and what user value it serves"),
        ),
        bootstrap_file(
            Path::new("docs/specs/README.md"),
            artifact_root_readme("Specs", "required behavior, evidence, and claim boundaries"),
        ),
        bootstrap_file(
            Path::new("docs/adr/README.md"),
            artifact_root_readme("ADRs", "durable architecture decisions"),
        ),
        bootstrap_file(
            Path::new("plans/README.md"),
            artifact_root_readme("Plans", "PR-sized execution sequences and rollback notes"),
        ),
        bootstrap_file(Path::new(".allow/imports/README.md"), imports_root_readme()),
        bootstrap_file(
            Path::new("docs/status/SUPPORT_TIERS.md"),
            support_tiers_template(),
        ),
    ];

    files.extend(
        EXPECTED_TEMPLATE_FILES
            .iter()
            .map(|path| bootstrap_file(Path::new(path), template_contents(path))),
    );

    if legacy_compatibility {
        files.splice(
            6..6,
            [
                bootstrap_file(
                    Path::new(".allow/goals/README.md"),
                    artifact_root_readme(
                        "Legacy Active Goals",
                        "historical compatibility metadata only; not current work authority",
                    ),
                ),
                bootstrap_file(
                    Path::new(".allow/goals/active.toml"),
                    active_goal_template(),
                ),
                bootstrap_file(Path::new(".allow/goals/archive/.gitkeep"), String::new()),
            ],
        );
    }
    files
}

fn bootstrap_file(path: &Path, contents: String) -> SpecSystemBootstrapFile {
    SpecSystemBootstrapFile {
        path: path.to_path_buf(),
        contents,
    }
}

fn spec_system_config_template(legacy_compatibility: bool) -> String {
    if legacy_compatibility {
        return r#"schema_version = "1.0"
profile = "spec-system"
mode = "advisory"
generation = "legacy-v1"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".allow/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = ".allow/artifacts/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
# Legacy active-goal compatibility is explicit and historical-only. It cannot
# select current work or authorize mutation, implementation, or support state.
active_goal_required = false
closeout_required_for_done_items = true

[import_roots]
owned = ".allow/imports"

[[import_roots.entries]]
id = "owned-imports"
path = ".allow/imports"
ecosystem = "cargo-allow"
role = "owned"
"#
        .to_string();
    }

    r#"schema_version = "1.0"
profile = "spec-system"
mode = "advisory"
generation = "current-v2"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = ".allow/artifacts/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
closeout_required_for_done_items = true

[import_roots]
owned = ".allow/imports"

[[import_roots.entries]]
id = "owned-imports"
path = ".allow/imports"
ecosystem = "cargo-allow"
role = "owned"
"#
    .to_string()
}

pub(super) fn spec_system_legacy_compatibility(
    root: &Path,
    config_path: &Path,
) -> CargoAllowResult<bool> {
    let path = root_relative_path(root, config_path);
    if !path.is_file() {
        return Ok(false);
    }
    let text = read_text_file_capped(&path).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "failed to read existing spec-system profile config {}: {error}",
                path.display()
            ),
        )
    })?;
    let config = parse_spec_system_config_at(Some(&path), &text)?;
    Ok(matches!(config.generation, SpecSystemGeneration::LegacyV1))
}

pub(super) fn legacy_bootstrap_conflicts(root: &Path) -> Vec<PathBuf> {
    [
        ".allow/goals",
        ".allow/goals/README.md",
        ".allow/goals/active.toml",
        ".allow/goals/archive/.gitkeep",
    ]
    .into_iter()
    .map(Path::new)
    .map(|path| root_relative_path(root, path))
    .filter(|path| path.exists())
    .collect()
}

fn doc_artifacts_template() -> String {
    r#"schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"
"#
    .to_string()
}

fn artifact_root_readme(title: &str, role: &str) -> String {
    format!(
        "# {title}\n\nThis directory contains spec-system artifacts for {role}.\n\nRegister governed artifacts in `{DEFAULT_OWNED_ARTIFACT_LEDGER}` so `cargo-allow check --profile spec-system` can validate their source-tree graph links.\n"
    )
}

fn imports_root_readme() -> String {
    r#"# Import Roots

External spec ecosystems discovered under import roots are read-only by default.
cargo-allow does not rewrite imported files unless explicitly promoted.

Place import adapters and discovery notes here when the repository adopts
external spec systems such as Kiro, Spec Kit, or generic `.spec/` trees.
"#
    .to_string()
}

fn active_goal_template() -> String {
    r#"schema_version = "1.0"

# Placeholder execution state for explicit legacy compatibility only.
# This file is historical/read-only metadata and is not current work authority.
id = "spec-system-profile"
title = "Spec-system profile"
status = "active"
owner = "codex"
created = "YYYY-MM-DD"

objective = """
Keep proposals, specs, ADRs, implementation plans, active goals, support tiers,
policy ledgers, and closeouts linked and linted.
"""

linked_plan = "plans/spec-system/implementation-plan.md"

[[work_item]]
id = "spec-system-pr-001"
status = "ready"
title = "Register source-of-truth artifacts"
proof_commands = [
  "cargo-allow check --profile spec-system --mode audit",
  "cargo-allow worklist --profile spec-system --format json",
]
"#
    .to_string()
}

fn support_tiers_template() -> String {
    r#"# Support Tiers

| Surface | Tier | Claim | Proof command | Notes |
| --- | --- | --- | --- | --- |
| Spec-system profile | Advisory | Source-of-truth graph artifacts can be linted. | cargo-allow check --profile spec-system --mode audit | Opt-in profile; structural validation only. |
"#
    .to_string()
}

fn template_contents(path: &str) -> String {
    let (id, kind, title) = match path {
        "docs/templates/proposal.md" => ("CARGO-ALLOW-PROP-0000", "proposal", "Proposal"),
        "docs/templates/spec.md" => ("CARGO-ALLOW-SPEC-0000", "spec", "Spec"),
        "docs/templates/adr.md" => ("CARGO-ALLOW-ADR-0000", "adr", "ADR"),
        "docs/templates/implementation-plan.md" => (
            "CARGO-ALLOW-PLAN-0000",
            "implementation_plan",
            "Implementation Plan",
        ),
        "docs/templates/plan-item.md" => ("CARGO-ALLOW-ITEM-0000", "plan_item", "Plan Item"),
        "docs/templates/closeout.md" => ("CARGO-ALLOW-CLOSEOUT-0000", "closeout", "Closeout"),
        "docs/templates/pr-body.md" => ("CARGO-ALLOW-PR-0000", "release_record", "PR Body"),
        _ => ("CARGO-ALLOW-ARTIFACT-0000", "artifact", "Artifact"),
    };
    format!(
        r#"---
id: {id}
kind: {kind}
status: draft
owner: repo-infra
created: YYYY-MM-DD
---

# {title}: Title

## Purpose

State the artifact's job in the source-of-truth graph.

## Links

- Linked proposal:
- Linked spec:
- Linked plan:

## Required Evidence

- Proof command or artifact:

## Claim Boundary

Structural source-tree graph metadata only. This artifact does not prove command
execution or semantic correctness by itself.

## Rollback

Describe how to supersede, withdraw, or close this artifact.
"#
    )
}
