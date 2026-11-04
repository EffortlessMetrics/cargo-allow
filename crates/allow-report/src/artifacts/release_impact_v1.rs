//! Typed release-impact routing contract (#3794).
//!
//! One pure, source-only classifier decides whether a base/head change
//! is release-sensitive, and the workflow graph routes the exact-subject
//! release rehearsal (#3792) from that decision. The classifier reads
//! checked repository inventories only — no Cargo metadata, no rustc,
//! no network, no repository code execution, no credentials.
//!
//! Laws (from the #3794 checkpoint packet):
//! - the three-vocabulary decision is `release_sensitive` (a checked
//!   release-sensitive rule matched), `unrelated` (every path is on the
//!   narrow provably-unrelated list), or `unknown` (an unmatched path
//!   or an unresolved semantic owner remains). Unknown selects the
//!   rehearsal: the classifier fails closed;
//! - the receipt binds the exact base/head/merge-base subject, so a
//!   base movement with an identical PR head stales the decision;
//! - the denominator is the checked path rules plus the #3786
//!   release-identity semantic-owner inventory plus the package
//!   topology's include/file-list authority (package roots and packaged
//!   asset roots), so an indirect generated consumer cannot escape;
//! - the terminal aggregate validates routing completeness only. It
//!   distinguishes a legitimate `NotApplicable` from missing, skipped,
//!   cancelled, stale, or malformed proof, and it never interprets the
//!   rehearsal harness status as release authorization (#3792 owns the
//!   discriminating-status correction).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use allow_core::sha256_v1_bytes;

pub const RELEASE_IMPACT_SCHEMA_ID: &str = "cargo-allow.release-impact.v1";
pub const RELEASE_IMPACT_SCHEMA_VERSION: u32 = 1;
pub const RELEASE_REHEARSAL_OBSERVATION_SCHEMA_ID: &str =
    "cargo-allow.release-rehearsal-observation.v1";
pub const RELEASE_REHEARSAL_OBSERVATION_SCHEMA_VERSION: u32 = 1;
pub const RELEASE_REHEARSAL_REQUIRED_SCHEMA_ID: &str = "cargo-allow.release-rehearsal-required.v1";
pub const RELEASE_REHEARSAL_REQUIRED_SCHEMA_VERSION: u32 = 1;

/// The exact release-sensitive path denominator from the #3794 packet.
/// Ordered: the first matching rule decides the path.
pub const RELEASE_SENSITIVE_PATH_RULES_V1: &[ReleaseImpactPathRuleV1] = &[
    ReleaseImpactPathRuleV1 {
        name: "workspace-manifest",
        owner: "#3794 release manifest authority",
        strategy: ReleaseImpactPathMatchV1::Exact("Cargo.toml"),
    },
    ReleaseImpactPathRuleV1 {
        name: "workspace-lockfile",
        owner: "#3794 release manifest authority",
        strategy: ReleaseImpactPathMatchV1::Exact("Cargo.lock"),
    },
    ReleaseImpactPathRuleV1 {
        name: "package-manifest",
        owner: "#3794 release manifest authority",
        strategy: ReleaseImpactPathMatchV1::PrefixAndContains {
            prefix: "crates/",
            needle: "/Cargo.toml",
        },
    },
    ReleaseImpactPathRuleV1 {
        name: "package-topology",
        owner: "#2923/#3794 package topology authority",
        strategy: ReleaseImpactPathMatchV1::Prefix("policy/product-package-topology"),
    },
    ReleaseImpactPathRuleV1 {
        name: "support-matrix",
        owner: "#3794 release identity authority",
        strategy: ReleaseImpactPathMatchV1::Exact("docs/support-matrix.toml"),
    },
    ReleaseImpactPathRuleV1 {
        name: "changie-config",
        owner: "#3794 release-record authority",
        strategy: ReleaseImpactPathMatchV1::Exact(".changie.yaml"),
    },
    ReleaseImpactPathRuleV1 {
        name: "changie-fragments",
        owner: "#3794 release-record authority",
        strategy: ReleaseImpactPathMatchV1::Prefix(".changes/"),
    },
    ReleaseImpactPathRuleV1 {
        name: "changelog",
        owner: "#3794 release-record authority",
        strategy: ReleaseImpactPathMatchV1::Exact("CHANGELOG.md"),
    },
    ReleaseImpactPathRuleV1 {
        name: "release-docs",
        owner: "#3794 release-record authority",
        strategy: ReleaseImpactPathMatchV1::Prefix("docs/release/"),
    },
    ReleaseImpactPathRuleV1 {
        name: "release-schema",
        owner: "#3794 release contract authority",
        strategy: ReleaseImpactPathMatchV1::PrefixAndContains {
            prefix: "docs/schemas/",
            needle: "release",
        },
    },
    ReleaseImpactPathRuleV1 {
        name: "release-script",
        owner: "#3794 release tooling authority",
        strategy: ReleaseImpactPathMatchV1::DirectoryFileContains {
            directory: "scripts/",
            needle: "release",
        },
    },
    ReleaseImpactPathRuleV1 {
        name: "candidate-script",
        owner: "#3750/#3794 candidate tooling authority",
        strategy: ReleaseImpactPathMatchV1::DirectoryFileContains {
            directory: "scripts/",
            needle: "candidate",
        },
    },
    ReleaseImpactPathRuleV1 {
        name: "release-workflow",
        owner: "#3760/#3794 release workflow authority",
        strategy: ReleaseImpactPathMatchV1::Prefix(".github/workflows/release"),
    },
    ReleaseImpactPathRuleV1 {
        name: "release-control-source",
        owner: "#3760/#3794 release-control authority",
        strategy: ReleaseImpactPathMatchV1::Prefix("release-control/"),
    },
];

/// The narrow provably-unrelated denominator. A path on this list has a
/// checked owner that cannot change release bytes, release tooling, or
/// release identity; anything not decided by the rules above, the
/// semantic-owner inventory, the package topology, or this list is
/// `unknown` and requires the rehearsal.
pub const RELEASE_UNRELATED_PATH_RULES_V1: &[ReleaseImpactPathRuleV1] = &[
    ReleaseImpactPathRuleV1 {
        name: "docs-non-release",
        owner: "#3794 documentation owner",
        strategy: ReleaseImpactPathMatchV1::Prefix("docs/"),
    },
    ReleaseImpactPathRuleV1 {
        name: "dependabot-config",
        owner: "#3794 dependency automation owner",
        strategy: ReleaseImpactPathMatchV1::Exact(".github/dependabot.yml"),
    },
    ReleaseImpactPathRuleV1 {
        name: "issue-forms",
        owner: "#3794 repository automation owner",
        strategy: ReleaseImpactPathMatchV1::Prefix(".github/ISSUE_TEMPLATE/"),
    },
    ReleaseImpactPathRuleV1 {
        name: "pr-template",
        owner: "#3794 repository automation owner",
        strategy: ReleaseImpactPathMatchV1::Prefix(".github/PULL_REQUEST_TEMPLATE"),
    },
];

/// Governance surfaces whose release sensitivity is semantic ("only
/// when release authority or stop law changes"). A lexical classifier
/// cannot resolve that qualifier, so the decision is `unknown` and the
/// rehearsal is required — the fail-closed direction.
pub const RELEASE_UNRESOLVED_GOVERNANCE_RULES_V1: &[ReleaseImpactPathRuleV1] = &[
    ReleaseImpactPathRuleV1 {
        name: "agents-governance",
        owner: "#3760 stop law authority — release-authority qualifier unresolvable lexically",
        strategy: ReleaseImpactPathMatchV1::Exact("AGENTS.md"),
    },
    ReleaseImpactPathRuleV1 {
        name: "gemini-governance",
        owner: "#3760 stop law authority — release-authority qualifier unresolvable lexically",
        strategy: ReleaseImpactPathMatchV1::Exact("GEMINI.md"),
    },
    ReleaseImpactPathRuleV1 {
        name: "campaign-skill-governance",
        owner: "#3768/#3760 campaign skill — release-authority qualifier unresolvable lexically",
        strategy: ReleaseImpactPathMatchV1::Prefix(".agents/"),
    },
    ReleaseImpactPathRuleV1 {
        name: "workflow-graph-unresolved",
        owner: "#3794 workflow graph owner — a non-release workflow change can alter the rehearsal graph itself",
        strategy: ReleaseImpactPathMatchV1::Prefix(".github/workflows/"),
    },
    ReleaseImpactPathRuleV1 {
        name: "local-action-unresolved",
        owner: "#3794 workflow construction owner — action changes can alter rehearsal steps",
        strategy: ReleaseImpactPathMatchV1::Prefix(".github/actions/"),
    },
    ReleaseImpactPathRuleV1 {
        name: "root-checkout-tooling-unresolved",
        owner: "#3794 checkout/toolchain owner — root tooling affects proof execution",
        strategy: ReleaseImpactPathMatchV1::Prefix("rust-"),
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseImpactDecisionV1 {
    ReleaseSensitive,
    Unrelated,
    Unknown,
}

impl ReleaseImpactDecisionV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReleaseSensitive => "release_sensitive",
            Self::Unrelated => "unrelated",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn rehearsal_required(self) -> bool {
        !matches!(self, Self::Unrelated)
    }
}

/// The subject the classifier decides against. All three SHAs bind the
/// receipt: a base movement with an identical PR head is a different
/// subject and stales every downstream receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseImpactSubjectV1 {
    pub base_sha: String,
    pub head_sha: String,
    pub merge_base_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseImpactClassifierInputV1 {
    pub subject: ReleaseImpactSubjectV1,
    /// Forward-slash repository-relative changed paths, one per file
    /// (`git diff --name-only <merge-base> <head>`). Order-insensitive:
    /// the classifier sorts and deduplicates before deciding.
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseImpactPathMatchV1 {
    Exact(&'static str),
    Prefix(&'static str),
    PrefixAndContains {
        prefix: &'static str,
        needle: &'static str,
    },
    DirectoryFileContains {
        directory: &'static str,
        needle: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseImpactPathRuleV1 {
    pub name: &'static str,
    pub owner: &'static str,
    pub strategy: ReleaseImpactPathMatchV1,
}

impl ReleaseImpactPathRuleV1 {
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        match self.strategy {
            ReleaseImpactPathMatchV1::Exact(exact) => path == exact,
            ReleaseImpactPathMatchV1::Prefix(prefix) => path.starts_with(prefix),
            ReleaseImpactPathMatchV1::PrefixAndContains { prefix, needle } => {
                path.starts_with(prefix) && path.contains(needle)
            }
            ReleaseImpactPathMatchV1::DirectoryFileContains { directory, needle } => {
                path.starts_with(directory)
                    && path
                        .rsplit('/')
                        .next()
                        .is_some_and(|file| file.contains(needle))
            }
        }
    }
}

/// One path's routing decision, retained verbatim so the receipt is
/// auditable without re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseImpactPathDecisionV1 {
    pub path: String,
    pub decision: ReleaseImpactDecisionV1,
    pub rule: String,
    pub owner: String,
}

/// The checked denominator the classifier consumed, with a content
/// digest so a receipt proves which denominator generation decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseImpactDenominatorBindingV1 {
    pub path_rules: usize,
    pub unrelated_rules: usize,
    pub governance_rules: usize,
    pub semantic_owner_paths: usize,
    pub package_roots: usize,
    pub packaged_asset_roots: usize,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseImpactReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub base_sha: String,
    pub head_sha: String,
    pub merge_base_sha: String,
    pub classification: ReleaseImpactDecisionV1,
    pub rehearsal_required: bool,
    pub path_decisions: Vec<ReleaseImpactPathDecisionV1>,
    pub matched_release_sensitive_paths: Vec<String>,
    pub unknown_paths: Vec<String>,
    pub denominator: ReleaseImpactDenominatorBindingV1,
    pub rationale: String,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// The #3786 release-identity semantic-owner inventory joined with the
/// package topology include/file-list authority. Both inventories are
/// read from checked repository policy files only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseImpactDenominatorV1 {
    /// Exact paths from `policy/release-identity-consumers.toml`
    /// mapped to the row's owner reference.
    pub semantic_owner_paths: Vec<(String, String)>,
    /// `crates/<cargo_package_name>/` prefixes for every publish=true
    /// topology row, mapped to the row's logical_id: a change under a
    /// published package directory changes shipped package bytes.
    pub package_roots: Vec<(String, String)>,
    /// Topology `asset_roots` shipped inside the `.crate`, mapped to
    /// the owning logical_id.
    pub packaged_asset_roots: Vec<(String, String)>,
}

impl ReleaseImpactDenominatorV1 {
    #[must_use]
    pub fn binding(&self) -> ReleaseImpactDenominatorBindingV1 {
        let mut rule_material = String::new();
        for rule in RELEASE_SENSITIVE_PATH_RULES_V1 {
            rule_material.push_str("rule:");
            rule_material.push_str(rule.name);
            rule_material.push('\n');
        }
        for rule in RELEASE_UNRELATED_PATH_RULES_V1 {
            rule_material.push_str("unrelated:");
            rule_material.push_str(rule.name);
            rule_material.push('\n');
        }
        for rule in RELEASE_UNRESOLVED_GOVERNANCE_RULES_V1 {
            rule_material.push_str("governance:");
            rule_material.push_str(rule.name);
            rule_material.push('\n');
        }
        for (path, owner) in &self.semantic_owner_paths {
            rule_material.push_str("semantic-owner:");
            rule_material.push_str(path);
            rule_material.push('\x1f');
            rule_material.push_str(owner);
            rule_material.push('\n');
        }
        for (root, logical_id) in &self.package_roots {
            rule_material.push_str("package-root:");
            rule_material.push_str(root);
            rule_material.push('\x1f');
            rule_material.push_str(logical_id);
            rule_material.push('\n');
        }
        for (root, logical_id) in &self.packaged_asset_roots {
            rule_material.push_str("asset-root:");
            rule_material.push_str(root);
            rule_material.push('\x1f');
            rule_material.push_str(logical_id);
            rule_material.push('\n');
        }
        ReleaseImpactDenominatorBindingV1 {
            path_rules: RELEASE_SENSITIVE_PATH_RULES_V1.len(),
            unrelated_rules: RELEASE_UNRELATED_PATH_RULES_V1.len(),
            governance_rules: RELEASE_UNRESOLVED_GOVERNANCE_RULES_V1.len(),
            semantic_owner_paths: self.semantic_owner_paths.len(),
            package_roots: self.package_roots.len(),
            packaged_asset_roots: self.packaged_asset_roots.len(),
            digest: sha256_v1_bytes(rule_material.as_bytes()),
        }
    }
}

/// Compile the checked denominator from the repository root: the #3786
/// semantic-owner inventory and the generation-2 package topology.
/// Source-tree reads only; no Cargo metadata, no code execution.
pub fn release_impact_denominator(root: &Path) -> Result<ReleaseImpactDenominatorV1, String> {
    let identity_path = root.join("policy/release-identity-consumers.toml");
    let identity_bytes = std::fs::read(&identity_path)
        .map_err(|error| format!("read {}: {error}", identity_path.display()))?;
    #[derive(Deserialize)]
    struct IdentityDoc {
        schema: String,
        #[serde(default)]
        consumers: Vec<IdentityConsumerRow>,
    }
    #[derive(Deserialize)]
    struct IdentityConsumerRow {
        path: String,
        #[serde(default)]
        owner: Option<String>,
    }
    let identity: IdentityDoc = toml::from_slice(&identity_bytes)
        .map_err(|error| format!("parse {}: {error}", identity_path.display()))?;
    if identity.schema != "cargo-allow.release-identity-consumers.v1" {
        return Err(format!(
            "{} is not the #3786 semantic-owner inventory (schema {})",
            identity_path.display(),
            identity.schema
        ));
    }
    let mut semantic_owner_paths = Vec::new();
    for row in &identity.consumers {
        semantic_owner_paths.push((
            row.path.clone(),
            row.owner.clone().unwrap_or_else(|| "#3752".to_string()),
        ));
    }
    semantic_owner_paths.sort();
    semantic_owner_paths.dedup();

    let topology_path = root.join("policy/product-package-topology-v2.toml");
    let topology_bytes = std::fs::read(&topology_path)
        .map_err(|error| format!("read {}: {error}", topology_path.display()))?;
    #[derive(Deserialize)]
    struct TopologyDoc {
        schema_version: String,
        authority_generation: u32,
        #[serde(default)]
        package: Vec<TopologyPackageRow>,
    }
    #[derive(Deserialize)]
    struct TopologyPackageRow {
        logical_id: String,
        cargo_package_name: String,
        publish: bool,
        #[serde(default)]
        asset_roots: Vec<String>,
    }
    let topology: TopologyDoc = toml::from_slice(&topology_bytes)
        .map_err(|error| format!("parse {}: {error}", topology_path.display()))?;
    if topology.schema_version != "2.0" || topology.authority_generation != 2 {
        return Err(format!(
            "{} is not the generation-2 package include authority (schema_version {}, generation {})",
            topology_path.display(),
            topology.schema_version,
            topology.authority_generation
        ));
    }
    let mut package_roots = Vec::new();
    let mut packaged_asset_roots = Vec::new();
    for row in &topology.package {
        if !row.publish {
            continue;
        }
        package_roots.push((
            format!("crates/{}/", row.cargo_package_name),
            row.logical_id.clone(),
        ));
        for asset_root in &row.asset_roots {
            packaged_asset_roots.push((
                format!("{}/", asset_root.trim_end_matches('/')),
                row.logical_id.clone(),
            ));
        }
    }
    package_roots.sort();
    package_roots.dedup();
    packaged_asset_roots.sort();
    packaged_asset_roots.dedup();

    Ok(ReleaseImpactDenominatorV1 {
        semantic_owner_paths,
        package_roots,
        packaged_asset_roots,
    })
}

/// Lowercase 40- or 64-hex Git object name.
#[must_use]
pub fn is_git_object_name(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_subject(subject: &ReleaseImpactSubjectV1) -> Result<(), String> {
    for (field, sha) in [
        ("base_sha", subject.base_sha.as_str()),
        ("head_sha", subject.head_sha.as_str()),
        ("merge_base_sha", subject.merge_base_sha.as_str()),
    ] {
        if !is_git_object_name(sha) {
            return Err(format!(
                "subject.{field} is not a lowercase 40/64-hex Git object name"
            ));
        }
    }
    Ok(())
}

fn normalize_changed_paths(paths: &[String]) -> Result<Vec<String>, String> {
    let mut normalized = Vec::with_capacity(paths.len());
    for path in paths {
        if path.is_empty() {
            continue;
        }
        if path.contains('\\') {
            return Err(format!(
                "changed path {path:?} carries a backslash; the denominator is forward-slash only"
            ));
        }
        if path.contains('\n') || path.contains('\r') || path.contains('\0') {
            return Err(format!(
                "changed path {path:?} carries a line or NUL separator"
            ));
        }
        if path.starts_with('/') {
            return Err(format!("changed path {path:?} is not repository-relative"));
        }
        normalized.push(path.clone());
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

/// Decide every path and compile the receipt. Pure function of the
/// input plus the denominator: identical inputs produce identical
/// receipts apart from nothing — there is no clock in the receipt, so
/// two runs over the same subject are byte-identical.
pub fn classify_release_impact_v1(
    input: &ReleaseImpactClassifierInputV1,
    denominator: &ReleaseImpactDenominatorV1,
) -> Result<CargoAllowReleaseImpactReceiptV1, String> {
    validate_subject(&input.subject)?;
    let paths = normalize_changed_paths(&input.changed_paths)?;

    let mut decisions = Vec::with_capacity(paths.len());
    let mut matched = Vec::new();
    let mut unknown = Vec::new();
    for path in &paths {
        let decision = decide_path(path, denominator);
        if decision.decision == ReleaseImpactDecisionV1::ReleaseSensitive {
            matched.push(path.clone());
        }
        if decision.decision == ReleaseImpactDecisionV1::Unknown {
            unknown.push(path.clone());
        }
        decisions.push(decision);
    }

    let classification = if !matched.is_empty() {
        ReleaseImpactDecisionV1::ReleaseSensitive
    } else if !unknown.is_empty() {
        ReleaseImpactDecisionV1::Unknown
    } else {
        ReleaseImpactDecisionV1::Unrelated
    };
    let rationale = match classification {
        ReleaseImpactDecisionV1::ReleaseSensitive => format!(
            "{} release-sensitive path(s) matched the checked release denominator; the full exact-subject release rehearsal (#3792) is required.",
            matched.len()
        ),
        ReleaseImpactDecisionV1::Unknown => format!(
            "{} path(s) are unknown or carry an unresolved semantic owner; the classifier fails closed and requires the full exact-subject release rehearsal (#3792).",
            unknown.len()
        ),
        ReleaseImpactDecisionV1::Unrelated => {
            "Every changed path sits on the narrow provably-unrelated list; no release-sensitive surface moved, so the rehearsal is not required for this subject.".to_string()
        }
    };

    Ok(CargoAllowReleaseImpactReceiptV1 {
        schema_id: RELEASE_IMPACT_SCHEMA_ID.to_string(),
        schema_version: RELEASE_IMPACT_SCHEMA_VERSION,
        base_sha: input.subject.base_sha.clone(),
        head_sha: input.subject.head_sha.clone(),
        merge_base_sha: input.subject.merge_base_sha.clone(),
        rehearsal_required: classification.rehearsal_required(),
        classification,
        path_decisions: decisions,
        matched_release_sensitive_paths: matched,
        unknown_paths: unknown,
        denominator: denominator.binding(),
        rationale,
        limitations: vec![
            "path_denominator_only: the classifier decides from checked path inventories, not from content semantics; it cannot prove a docs change cannot affect release surfaces, only that its owner cannot".to_string(),
            "governance_qualifiers_unresolved: AGENTS.md/GEMINI.md/campaign-skill and non-release workflow changes select the rehearsal because their release-authority qualifier is semantic and unresolved".to_string(),
        ],
        claim_boundary: "Typed release-impact routing decision for one exact base/head/merge-base subject, compiled from checked repository inventories (path rules, #3786 semantic-owner inventory, generation-2 package topology) without Cargo metadata, rustc, network access, credential reads, or repository code execution. It selects proof routing only; it grants no release authority and does not classify finding semantics.".to_string(),
    })
}

fn decide_path(
    path: &str,
    denominator: &ReleaseImpactDenominatorV1,
) -> ReleaseImpactPathDecisionV1 {
    for rule in RELEASE_SENSITIVE_PATH_RULES_V1 {
        if rule.matches(path) {
            return ReleaseImpactPathDecisionV1 {
                path: path.to_string(),
                decision: ReleaseImpactDecisionV1::ReleaseSensitive,
                rule: rule.name.to_string(),
                owner: rule.owner.to_string(),
            };
        }
    }
    for (semantic_path, owner) in &denominator.semantic_owner_paths {
        if semantic_path == path {
            return ReleaseImpactPathDecisionV1 {
                path: path.to_string(),
                decision: ReleaseImpactDecisionV1::ReleaseSensitive,
                rule: "semantic-owner-inventory".to_string(),
                owner: owner.clone(),
            };
        }
    }
    for (package_root, logical_id) in &denominator.package_roots {
        if path.starts_with(package_root.as_str()) {
            return ReleaseImpactPathDecisionV1 {
                path: path.to_string(),
                decision: ReleaseImpactDecisionV1::ReleaseSensitive,
                rule: "package-bytes".to_string(),
                owner: format!("#2923 {logical_id} — published package include authority"),
            };
        }
    }
    for (asset_root, logical_id) in &denominator.packaged_asset_roots {
        if path.starts_with(asset_root.as_str()) {
            return ReleaseImpactPathDecisionV1 {
                path: path.to_string(),
                decision: ReleaseImpactDecisionV1::ReleaseSensitive,
                rule: "packaged-asset-root".to_string(),
                owner: format!("#2923 {logical_id} — packaged asset root"),
            };
        }
    }
    for rule in RELEASE_UNRESOLVED_GOVERNANCE_RULES_V1 {
        if rule.matches(path) {
            return ReleaseImpactPathDecisionV1 {
                path: path.to_string(),
                decision: ReleaseImpactDecisionV1::Unknown,
                rule: rule.name.to_string(),
                owner: rule.owner.to_string(),
            };
        }
    }
    for rule in RELEASE_UNRELATED_PATH_RULES_V1 {
        if rule.matches(path) {
            return ReleaseImpactPathDecisionV1 {
                path: path.to_string(),
                decision: ReleaseImpactDecisionV1::Unrelated,
                rule: rule.name.to_string(),
                owner: rule.owner.to_string(),
            };
        }
    }
    ReleaseImpactPathDecisionV1 {
        path: path.to_string(),
        decision: ReleaseImpactDecisionV1::Unknown,
        rule: "unmatched-path".to_string(),
        owner: "unresolved semantic owner — fail closed per the #3794 packet".to_string(),
    }
}

/// Structural validation of a decoded impact receipt: schema identity,
/// subject shape, and decision/flag consistency.
pub fn validate_release_impact_receipt_v1(
    receipt: &CargoAllowReleaseImpactReceiptV1,
) -> Result<(), String> {
    if receipt.schema_id != RELEASE_IMPACT_SCHEMA_ID {
        return Err(format!(
            "schema_id must be {RELEASE_IMPACT_SCHEMA_ID}; observed {}",
            receipt.schema_id
        ));
    }
    if receipt.schema_version != RELEASE_IMPACT_SCHEMA_VERSION {
        return Err(format!(
            "schema_version must be {RELEASE_IMPACT_SCHEMA_VERSION}; observed {}",
            receipt.schema_version
        ));
    }
    let subject = ReleaseImpactSubjectV1 {
        base_sha: receipt.base_sha.clone(),
        head_sha: receipt.head_sha.clone(),
        merge_base_sha: receipt.merge_base_sha.clone(),
    };
    validate_subject(&subject)?;
    if receipt.classification.rehearsal_required() != receipt.rehearsal_required {
        return Err(
            "rehearsal_required must equal (classification != unrelated); the flag drifted from the classification".to_string(),
        );
    }
    let mut sorted_decisions = receipt.path_decisions.clone();
    sorted_decisions.sort_by(|left, right| left.path.cmp(&right.path));
    sorted_decisions.dedup_by(|left, right| left.path == right.path);
    if sorted_decisions.len() != receipt.path_decisions.len() {
        return Err("path_decisions must be unique per path".to_string());
    }
    let sensitive: Vec<String> = receipt
        .path_decisions
        .iter()
        .filter(|row| row.decision == ReleaseImpactDecisionV1::ReleaseSensitive)
        .map(|row| row.path.clone())
        .collect();
    let unknown: Vec<String> = receipt
        .path_decisions
        .iter()
        .filter(|row| row.decision == ReleaseImpactDecisionV1::Unknown)
        .map(|row| row.path.clone())
        .collect();
    if sensitive != receipt.matched_release_sensitive_paths {
        return Err(
            "matched_release_sensitive_paths must be exactly the release_sensitive path decisions in receipt order".to_string(),
        );
    }
    if unknown != receipt.unknown_paths {
        return Err(
            "unknown_paths must be exactly the unknown path decisions in receipt order".to_string(),
        );
    }
    let expected_classification = if !sensitive.is_empty() {
        ReleaseImpactDecisionV1::ReleaseSensitive
    } else if !unknown.is_empty() {
        ReleaseImpactDecisionV1::Unknown
    } else {
        ReleaseImpactDecisionV1::Unrelated
    };
    if receipt.classification != expected_classification {
        return Err(
            "classification must equal the aggregate of the retained path decisions".to_string(),
        );
    }
    if receipt.denominator.digest.is_empty()
        || !receipt.denominator.digest.starts_with("sha256:v1:")
    {
        return Err("denominator.digest must be a sha256:v1 content digest".to_string());
    }
    if receipt.rationale.is_empty() || receipt.claim_boundary.is_empty() {
        return Err("rationale and claim_boundary must be non-empty".to_string());
    }
    Ok(())
}

/// The rehearsal job's checked observation of what actually ran. The
/// observation records the harness invocation verbatim; it never
/// interprets the harness status as release authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRehearsalObservationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub head_sha: String,
    pub base_sha: String,
    pub merge_base_sha: String,
    /// Digest of the exact impact-receipt bytes this observation was
    /// compiled from, so a sibling's receipt can never authorize this
    /// observation.
    pub impact_receipt_digest: String,
    pub rehearsal_required: bool,
    pub outcome: ReleaseRehearsalObservationOutcomeV1,
    pub harness_exit_code: Option<i32>,
    pub harness_aggregate_status: Option<String>,
    pub harness_phase_statuses: BTreeMap<String, String>,
    pub harness_commit_sha: Option<String>,
    pub notes: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRehearsalObservationOutcomeV1 {
    /// The harness was actually invoked and produced a subject-bound
    /// receipt; its exit code and statuses are recorded verbatim.
    Executed,
    /// The validated impact receipt selected no rehearsal; the job
    /// recorded the checked NotApplicable outcome instead.
    NotApplicable,
}

/// Compile the observation from a validated impact receipt. `Executed`
/// requires the harness exit code and a readable harness receipt JSON
/// whose `commit_sha` binds the exact head; anything else fails closed.
pub fn observe_release_rehearsal_v1(
    impact: &CargoAllowReleaseImpactReceiptV1,
    impact_receipt_digest: &str,
    outcome: ReleaseRehearsalObservationOutcomeV1,
    harness_exit_code: Option<i32>,
    harness_receipt: Option<&serde_json::Value>,
) -> Result<ReleaseRehearsalObservationV1, String> {
    validate_release_impact_receipt_v1(impact)?;
    if impact_receipt_digest.is_empty() || !impact_receipt_digest.starts_with("sha256:v1:") {
        return Err("impact_receipt_digest must be a sha256:v1 content digest".to_string());
    }
    let (notes, exit_code, aggregate_status, phase_statuses, commit_sha) = match outcome {
        ReleaseRehearsalObservationOutcomeV1::NotApplicable => {
            if harness_exit_code.is_some() || harness_receipt.is_some() {
                return Err(
                    "a NotApplicable observation must not carry harness invocation evidence"
                        .to_string(),
                );
            }
            (
                vec![
                    "the validated impact receipt selected rehearsal_required=false; no harness invocation belongs to this subject".to_string(),
                ],
                None,
                None,
                BTreeMap::new(),
                None,
            )
        }
        ReleaseRehearsalObservationOutcomeV1::Executed => {
            let Some(exit_code) = harness_exit_code else {
                return Err(
                    "an Executed observation requires the recorded harness exit code".to_string(),
                );
            };
            let Some(receipt) = harness_receipt else {
                return Err(
                    "an Executed observation requires the harness receipt JSON; a missing receipt can never prove invocation".to_string(),
                );
            };
            let commit_sha = receipt
                .get("commit_sha")
                .and_then(serde_json::Value::as_str)
                .ok_or("the harness receipt records no commit_sha subject binding")?
                .to_string();
            if commit_sha != impact.head_sha {
                return Err(format!(
                    "the harness receipt binds commit_sha {commit_sha}, not the validated head {}",
                    impact.head_sha
                ));
            }
            let aggregate_status = receipt
                .get("aggregate_status")
                .and_then(serde_json::Value::as_str)
                .ok_or("the harness receipt records no aggregate_status")?
                .to_string();
            let mut phase_statuses = BTreeMap::new();
            match receipt.get("phases").and_then(serde_json::Value::as_object) {
                Some(phases) => {
                    for (name, status) in phases {
                        phase_statuses.insert(
                            name.clone(),
                            status.as_str().unwrap_or("<non-string>").to_string(),
                        );
                    }
                }
                None => {
                    return Err(
                        "the harness receipt records no phases object; invocation proof is incomplete"
                            .to_string(),
                    );
                }
            }
            (
                vec![format!(
                    "harness invoked with exit code {exit_code}; aggregate_status and phase statuses are recorded verbatim and are not interpreted here (#3792 owns the discriminating-status correction)"
                )],
                Some(exit_code),
                Some(aggregate_status),
                phase_statuses,
                Some(commit_sha),
            )
        }
    };

    Ok(ReleaseRehearsalObservationV1 {
        schema_id: RELEASE_REHEARSAL_OBSERVATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_REHEARSAL_OBSERVATION_SCHEMA_VERSION,
        head_sha: impact.head_sha.clone(),
        base_sha: impact.base_sha.clone(),
        merge_base_sha: impact.merge_base_sha.clone(),
        impact_receipt_digest: impact_receipt_digest.to_string(),
        rehearsal_required: impact.rehearsal_required,
        outcome,
        harness_exit_code: exit_code,
        harness_aggregate_status: aggregate_status,
        harness_phase_statuses: phase_statuses,
        harness_commit_sha: commit_sha,
        notes,
        claim_boundary: "Checked observation of the exact-subject release rehearsal routing: the recorded harness exit code, aggregate_status, and phase statuses are verbatim producer output bound to the validated impact receipt digest. This observation interprets nothing as release authorization and carries no token context.".to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRehearsalRequiredAggregateV1 {
    Passed,
    Failed,
}

/// The always-run terminal aggregate result over the impact receipt and
/// the rehearsal observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseRehearsalRequiredResultV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub expected_base_sha: String,
    pub expected_head_sha: String,
    pub expected_merge_base_sha: String,
    pub impact_classification: Option<ReleaseImpactDecisionV1>,
    pub rehearsal_required: Option<bool>,
    pub rehearsal_outcome: Option<ReleaseRehearsalObservationOutcomeV1>,
    pub harness_aggregate_status: Option<String>,
    pub aggregate: ReleaseRehearsalRequiredAggregateV1,
    pub failure_reasons: Vec<String>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// The terminal routing aggregate. Validates routing completeness for
/// the exact expected subject: the impact receipt must be valid and
/// subject-bound, and the observation must exist, bind the same impact
/// artifact digest and subject, and agree with the routing decision.
/// Missing, skipped, stale, contradictory, or malformed proof fails the
/// aggregate. It never interprets the harness status as authorization.
pub fn evaluate_release_rehearsal_required_v1(
    impact_result: Result<&CargoAllowReleaseImpactReceiptV1, String>,
    impact_receipt_digest: Option<&str>,
    observation_result: Option<Result<&ReleaseRehearsalObservationV1, String>>,
    expected: &ReleaseImpactSubjectV1,
) -> CargoAllowReleaseRehearsalRequiredResultV1 {
    let mut failure_reasons = Vec::new();
    validate_subject(expected)
        .unwrap_or_else(|error| failure_reasons.push(format!("expected subject invalid: {error}")));

    let impact = match impact_result {
        Ok(impact) => {
            validate_release_impact_receipt_v1(impact).unwrap_or_else(|error| {
                failure_reasons.push(format!("impact receipt invalid: {error}"))
            });
            if impact.head_sha != expected.head_sha
                || impact.base_sha != expected.base_sha
                || impact.merge_base_sha != expected.merge_base_sha
            {
                failure_reasons.push(format!(
                    "impact receipt binds base {} head {} merge-base {}, not the expected subject base {} head {} merge-base {}",
                    impact.base_sha,
                    impact.head_sha,
                    impact.merge_base_sha,
                    expected.base_sha,
                    expected.head_sha,
                    expected.merge_base_sha,
                ));
            }
            Some(impact)
        }
        Err(error) => {
            failure_reasons.push(format!("impact receipt unreadable or malformed: {error}"));
            None
        }
    };

    let mut observation = None;
    match observation_result {
        Some(Ok(observed)) => {
            observation = Some(observed);
            if let Some(impact) = impact {
                if observed.head_sha != impact.head_sha
                    || observed.base_sha != impact.base_sha
                    || observed.merge_base_sha != impact.merge_base_sha
                {
                    failure_reasons.push(
                        "rehearsal observation binds a different subject than the impact receipt"
                            .to_string(),
                    );
                }
                if let Some(digest) = impact_receipt_digest {
                    if observed.impact_receipt_digest != digest {
                        failure_reasons.push(
                            "rehearsal observation was compiled from a different impact-receipt artifact (digest mismatch)".to_string(),
                        );
                    }
                } else {
                    failure_reasons.push(
                        "the exact impact-receipt digest was not supplied; the observation binding cannot be verified".to_string(),
                    );
                }
                if observed.rehearsal_required != impact.rehearsal_required {
                    failure_reasons.push(
                        "rehearsal observation carries a different rehearsal_required flag than the impact receipt".to_string(),
                    );
                }
                match (impact.rehearsal_required, observed.outcome) {
                    (true, ReleaseRehearsalObservationOutcomeV1::NotApplicable) => failure_reasons
                        .push(
                            "routing contradiction: the impact receipt requires the rehearsal but the observation records NotApplicable".to_string(),
                        ),
                    (false, ReleaseRehearsalObservationOutcomeV1::Executed) => failure_reasons
                        .push(
                            "routing contradiction: the impact receipt does not require the rehearsal but the observation records Executed".to_string(),
                        ),
                    _ => {}
                }
                if observed.outcome == ReleaseRehearsalObservationOutcomeV1::Executed
                    && (observed.harness_exit_code.is_none()
                        || observed.harness_aggregate_status.is_none()
                        || observed.harness_commit_sha.is_none()
                        || observed.harness_phase_statuses.is_empty())
                {
                    failure_reasons.push(
                        "rehearsal observation records Executed without complete invocation evidence (exit code, aggregate status, subject binding, phase statuses)".to_string(),
                    );
                }
            }
        }
        Some(Err(error)) => {
            failure_reasons.push(format!(
                "rehearsal observation unreadable or malformed: {error}"
            ));
        }
        None => {
            failure_reasons.push(
                "rehearsal proof missing: the always-run rehearsal job produced no observation artifact (skipped, cancelled, or removed evidence is not clean)".to_string(),
            );
        }
    }

    let aggregate = if failure_reasons.is_empty() {
        ReleaseRehearsalRequiredAggregateV1::Passed
    } else {
        ReleaseRehearsalRequiredAggregateV1::Failed
    };

    CargoAllowReleaseRehearsalRequiredResultV1 {
        schema_id: RELEASE_REHEARSAL_REQUIRED_SCHEMA_ID.to_string(),
        schema_version: RELEASE_REHEARSAL_REQUIRED_SCHEMA_VERSION,
        expected_base_sha: expected.base_sha.clone(),
        expected_head_sha: expected.head_sha.clone(),
        expected_merge_base_sha: expected.merge_base_sha.clone(),
        impact_classification: impact.map(|receipt| receipt.classification),
        rehearsal_required: impact.map(|receipt| receipt.rehearsal_required),
        rehearsal_outcome: observation.map(|observed| observed.outcome),
        harness_aggregate_status: observation
            .and_then(|observed| observed.harness_aggregate_status.clone()),
        aggregate,
        failure_reasons,
        limitations: vec![
            "routing_completeness_only: the aggregate proves the routing evidence exists, binds the exact subject, and is internally consistent; it does not interpret the rehearsal harness status as release authorization (#3792 reopened owns the discriminating-status correction)".to_string(),
            "evidence_uploaded_is_not_evidence: artifact upload success proves nothing; only the validated receipt pair passes the aggregate".to_string(),
        ],
        claim_boundary: "Always-run terminal routing aggregate for one exact base/head/merge-base subject: a Passed result means the release-impact decision and the required rehearsal evidence exist, bind this exact subject, and agree — nothing more. It grants no release authority, performs no external action, and receives no token context; required-check selection stays with the #2283/#2284 authority.".to_string(),
    }
}

/// Canonical JSON rendering for the receipt family.
///
/// # Errors
/// Returns the serialization error when a value cannot be rendered.
pub fn render_release_impact_receipt_json(
    receipt: &CargoAllowReleaseImpactReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}

/// Human one-view of the routing decision.
#[must_use]
pub fn render_release_impact_receipt_human(receipt: &CargoAllowReleaseImpactReceiptV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Release impact: {} (rehearsal_required={})",
        receipt.classification.as_str(),
        receipt.rehearsal_required
    ));
    lines.push(format!(
        "Subject: base {} head {} merge-base {}",
        receipt.base_sha, receipt.head_sha, receipt.merge_base_sha
    ));
    lines.push(format!(
        "Denominator: {} path rules, {} unrelated, {} governance, {} semantic-owner rows, {} package roots, {} packaged asset roots ({})",
        receipt.denominator.path_rules,
        receipt.denominator.unrelated_rules,
        receipt.denominator.governance_rules,
        receipt.denominator.semantic_owner_paths,
        receipt.denominator.package_roots,
        receipt.denominator.packaged_asset_roots,
        receipt.denominator.digest
    ));
    lines.push(receipt.rationale.clone());
    for decision in &receipt.path_decisions {
        lines.push(format!(
            "  {} -> {} (rule {}, owner {})",
            decision.path,
            decision.decision.as_str(),
            decision.rule,
            decision.owner
        ));
    }
    lines.join("\n")
}

/// Human one-view of the rehearsal observation.
#[must_use]
pub fn render_release_rehearsal_observation_human(
    observation: &ReleaseRehearsalObservationV1,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Rehearsal observation: {:?} (rehearsal_required={})",
        observation.outcome, observation.rehearsal_required
    ));
    lines.push(format!(
        "Subject: base {} head {} merge-base {}",
        observation.head_sha, observation.base_sha, observation.merge_base_sha
    ));
    lines.push(format!(
        "Impact receipt digest: {}",
        observation.impact_receipt_digest
    ));
    if let Some(exit_code) = observation.harness_exit_code {
        lines.push(format!("Harness exit code: {exit_code}"));
    }
    if let Some(status) = &observation.harness_aggregate_status {
        lines.push(format!("Harness aggregate_status (verbatim): {status}"));
    }
    for (phase, status) in &observation.harness_phase_statuses {
        lines.push(format!("  phase {phase}: {status}"));
    }
    for note in &observation.notes {
        lines.push(format!("Note: {note}"));
    }
    lines.join("\n")
}

/// Human one-view of the terminal routing aggregate.
#[must_use]
pub fn render_release_rehearsal_required_human(
    result: &CargoAllowReleaseRehearsalRequiredResultV1,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Release rehearsal routing ({:?})",
        result.aggregate
    ));
    lines.push(format!(
        "Expected subject: base {} head {} merge-base {}",
        result.expected_base_sha, result.expected_head_sha, result.expected_merge_base_sha
    ));
    if let Some(classification) = result.impact_classification {
        lines.push(format!(
            "Impact classification: {}",
            classification.as_str()
        ));
    }
    if let Some(outcome) = result.rehearsal_outcome {
        lines.push(format!("Rehearsal outcome: {outcome:?}"));
    }
    if let Some(status) = &result.harness_aggregate_status {
        lines.push(format!("Harness aggregate_status (verbatim): {status}"));
    }
    for reason in &result.failure_reasons {
        lines.push(format!("Failure: {reason}"));
    }
    for limitation in &result.limitations {
        lines.push(format!("Limitation: {limitation}"));
    }
    lines.join("\n")
}

/// # Errors
/// Returns the serialization error when a value cannot be rendered.
pub fn render_release_rehearsal_observation_json(
    observation: &ReleaseRehearsalObservationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(observation)
}

/// # Errors
/// Returns the serialization error when a value cannot be rendered.
pub fn render_release_rehearsal_required_json(
    result: &CargoAllowReleaseRehearsalRequiredResultV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const MERGE_BASE: &str = "cccccccccccccccccccccccccccccccccccccccc";

    fn subject() -> ReleaseImpactSubjectV1 {
        ReleaseImpactSubjectV1 {
            base_sha: BASE.to_string(),
            head_sha: HEAD.to_string(),
            merge_base_sha: MERGE_BASE.to_string(),
        }
    }

    fn denominator() -> ReleaseImpactDenominatorV1 {
        ReleaseImpactDenominatorV1 {
            semantic_owner_paths: vec![(
                "scripts/proof-direct-floors.sh".to_string(),
                "#3903".to_string(),
            )],
            package_roots: vec![
                ("crates/allow-match/".to_string(), "allow-match".to_string()),
                ("crates/cargo-allow/".to_string(), "cargo-allow".to_string()),
            ],
            packaged_asset_roots: vec![
                ("docs/schemas/".to_string(), "cargo-allow".to_string()),
                ("examples/".to_string(), "cargo-allow".to_string()),
            ],
        }
    }

    fn classify(paths: &[&str]) -> CargoAllowReleaseImpactReceiptV1 {
        classify_with_denominator(paths, &denominator())
    }

    fn classify_with_denominator(
        paths: &[&str],
        denominator: &ReleaseImpactDenominatorV1,
    ) -> CargoAllowReleaseImpactReceiptV1 {
        let input = ReleaseImpactClassifierInputV1 {
            subject: subject(),
            changed_paths: paths.iter().map(|path| (*path).to_string()).collect(),
        };
        classify_release_impact_v1(&input, denominator).unwrap_or_else(|error| {
            std::panic::panic_any(format!("classify should succeed: {error}"))
        })
    }

    #[test]
    fn unknown_paths_are_actually_selected_and_require_the_rehearsal() {
        // #3950 left the Unknown variant never selected; the production
        // classifier must select it for unmatched input.
        let receipt = classify(&["unrelated-data.bin"]);
        assert_eq!(receipt.classification, ReleaseImpactDecisionV1::Unknown);
        assert!(receipt.rehearsal_required);
        assert_eq!(receipt.unknown_paths, vec!["unrelated-data.bin"]);
        assert!(receipt.matched_release_sensitive_paths.is_empty());
    }

    #[test]
    fn governance_qualifiers_fail_closed_to_unknown() {
        for path in ["AGENTS.md", "GEMINI.md", ".agents/skills/x/SKILL.md"] {
            let receipt = classify(&[path]);
            assert_eq!(
                receipt.classification,
                ReleaseImpactDecisionV1::Unknown,
                "{path}"
            );
            assert!(receipt.rehearsal_required, "{path}");
        }
    }

    #[test]
    fn non_release_workflow_graph_changes_fail_closed() {
        let receipt = classify(&[".github/workflows/ci.yml"]);
        assert_eq!(receipt.classification, ReleaseImpactDecisionV1::Unknown);
        assert!(receipt.rehearsal_required);
        assert_eq!(receipt.path_decisions[0].rule, "workflow-graph-unresolved");
    }

    #[test]
    fn release_sensitive_path_rules_decide_the_checked_denominator() {
        let cases = [
            ("Cargo.toml", "workspace-manifest"),
            ("Cargo.lock", "workspace-lockfile"),
            ("crates/allow-core/Cargo.toml", "package-manifest"),
            (
                "policy/product-package-topology-v2.toml",
                "package-topology",
            ),
            ("docs/support-matrix.toml", "support-matrix"),
            (".changie.yaml", "changie-config"),
            (".changes/Added-20261104-1200.yaml", "changie-fragments"),
            ("CHANGELOG.md", "changelog"),
            ("docs/release/0.2.0.md", "release-docs"),
            (
                "docs/schemas/cargo-allow.release-impact.v1.schema.json",
                "release-schema",
            ),
            ("scripts/release-rehearsal.py", "release-script"),
            ("scripts/exact-candidate-package-set.sh", "candidate-script"),
            (".github/workflows/release.yml", "release-workflow"),
            (
                ".github/workflows/release-authorized.yml",
                "release-workflow",
            ),
            (
                "release-control/authorization/decision.toml",
                "release-control-source",
            ),
        ];
        for (path, rule) in cases {
            let receipt = classify(&[path]);
            assert_eq!(
                receipt.classification,
                ReleaseImpactDecisionV1::ReleaseSensitive,
                "{path} must be release sensitive"
            );
            assert!(receipt.rehearsal_required, "{path}");
            assert_eq!(receipt.path_decisions[0].rule, rule, "{path}");
        }
    }

    #[test]
    fn semantic_owner_inventory_and_topology_authorities_decide() {
        // #3786 semantic-owner inventory row: scripts/proof-direct-floors.sh
        // matches no release-sensitive path rule (its name carries neither
        // "release" nor "candidate"), so only the checked inventory row
        // (#3903) decides it — the layer that catches the indirect
        // generated consumer the packet warns about.
        let receipt = classify(&["scripts/proof-direct-floors.sh"]);
        assert_eq!(
            receipt.classification,
            ReleaseImpactDecisionV1::ReleaseSensitive
        );
        assert_eq!(receipt.path_decisions[0].rule, "semantic-owner-inventory");
        assert_eq!(receipt.path_decisions[0].owner, "#3903");

        // Package include/file-list authority: shipped package bytes.
        let receipt = classify(&["crates/allow-match/src/lib.rs"]);
        assert_eq!(
            receipt.classification,
            ReleaseImpactDecisionV1::ReleaseSensitive
        );
        assert_eq!(receipt.path_decisions[0].rule, "package-bytes");

        // Packaged asset root authority.
        for path in [
            "docs/schemas/doctor.schema.json",
            "examples/github-actions/x.yml",
        ] {
            let receipt = classify(&[path]);
            assert_eq!(
                receipt.classification,
                ReleaseImpactDecisionV1::ReleaseSensitive,
                "{path}"
            );
            assert_eq!(
                receipt.path_decisions[0].rule, "packaged-asset-root",
                "{path}"
            );
        }
    }

    #[test]
    fn unrelated_docs_typo_stays_on_the_fast_path() {
        let receipt = classify(&["docs/how-to/run-in-ci.md", "docs/README.md"]);
        assert_eq!(receipt.classification, ReleaseImpactDecisionV1::Unrelated);
        assert!(!receipt.rehearsal_required);
        assert!(receipt.unknown_paths.is_empty());
        assert!(receipt.matched_release_sensitive_paths.is_empty());
    }

    #[test]
    fn one_sensitive_path_in_a_mixed_set_decides_the_subject() {
        let receipt = classify(&["docs/README.md", "Cargo.lock"]);
        assert_eq!(
            receipt.classification,
            ReleaseImpactDecisionV1::ReleaseSensitive
        );
        assert_eq!(receipt.matched_release_sensitive_paths, vec!["Cargo.lock"]);
        assert_eq!(receipt.path_decisions.len(), 2);
    }

    #[test]
    fn paths_are_sorted_and_deduplicated() {
        let receipt = classify(&["docs/b.md", "docs/a.md", "docs/b.md"]);
        let paths: Vec<&str> = receipt
            .path_decisions
            .iter()
            .map(|row| row.path.as_str())
            .collect();
        assert_eq!(paths, vec!["docs/a.md", "docs/b.md"]);
    }

    #[test]
    fn the_receipt_is_a_pure_function_of_subject_and_paths() {
        let first = classify(&["Cargo.lock"]);
        let second = classify(&["Cargo.lock"]);
        let text_first = render_release_impact_receipt_json(&first)
            .unwrap_or_else(|error| std::panic::panic_any(format!("render: {error}")));
        let text_second = render_release_impact_receipt_json(&second)
            .unwrap_or_else(|error| std::panic::panic_any(format!("render: {error}")));
        assert_eq!(text_first, text_second, "no clock enters the receipt");
    }

    #[test]
    fn base_movement_with_an_identical_head_is_a_different_subject() {
        let input_one = ReleaseImpactClassifierInputV1 {
            subject: subject(),
            changed_paths: vec!["docs/README.md".to_string()],
        };
        let input_two = ReleaseImpactClassifierInputV1 {
            subject: ReleaseImpactSubjectV1 {
                base_sha: BASE.to_string(),
                head_sha: HEAD.to_string(),
                merge_base_sha: "dddddddddddddddddddddddddddddddddddddddd".to_string(),
            },
            changed_paths: vec![
                "docs/README.md".to_string(),
                "docs/release/0.2.0.md".to_string(),
            ],
        };
        let denominator = denominator();
        let one = classify_release_impact_v1(&input_one, &denominator)
            .unwrap_or_else(|error| std::panic::panic_any(format!("classify: {error}")));
        let two = classify_release_impact_v1(&input_two, &denominator)
            .unwrap_or_else(|error| std::panic::panic_any(format!("classify: {error}")));
        assert_ne!(one.merge_base_sha, two.merge_base_sha);
        assert_eq!(one.classification, ReleaseImpactDecisionV1::Unrelated);
        assert_eq!(
            two.classification,
            ReleaseImpactDecisionV1::ReleaseSensitive
        );
    }

    #[test]
    fn malformed_input_fails_closed() {
        let bad_subject = ReleaseImpactClassifierInputV1 {
            subject: ReleaseImpactSubjectV1 {
                base_sha: "HEAD".to_string(),
                head_sha: HEAD.to_string(),
                merge_base_sha: MERGE_BASE.to_string(),
            },
            changed_paths: vec![],
        };
        assert!(classify_release_impact_v1(&bad_subject, &denominator()).is_err());

        let backslash = ReleaseImpactClassifierInputV1 {
            subject: subject(),
            changed_paths: vec![String::from("crates\\allow-core\\src\\lib.rs")],
        };
        assert!(classify_release_impact_v1(&backslash, &denominator()).is_err());

        let absolute = ReleaseImpactClassifierInputV1 {
            subject: subject(),
            changed_paths: vec![String::from("/etc/passwd")],
        };
        assert!(classify_release_impact_v1(&absolute, &denominator()).is_err());
    }

    #[test]
    fn empty_diff_selects_the_fast_path() {
        let input = ReleaseImpactClassifierInputV1 {
            subject: subject(),
            changed_paths: vec![],
        };
        let receipt = classify_release_impact_v1(&input, &denominator())
            .unwrap_or_else(|error| std::panic::panic_any(format!("classify: {error}")));
        assert_eq!(receipt.classification, ReleaseImpactDecisionV1::Unrelated);
        assert!(!receipt.rehearsal_required);
    }

    #[test]
    fn the_denominator_binding_is_stable_and_covers_the_table() {
        let denominator = denominator();
        let binding = denominator.binding();
        assert_eq!(binding.path_rules, RELEASE_SENSITIVE_PATH_RULES_V1.len());
        assert_eq!(
            binding.unrelated_rules,
            RELEASE_UNRELATED_PATH_RULES_V1.len()
        );
        assert_eq!(
            binding.governance_rules,
            RELEASE_UNRESOLVED_GOVERNANCE_RULES_V1.len()
        );
        assert_eq!(binding.semantic_owner_paths, 1);
        assert_eq!(binding.package_roots, 2);
        assert_eq!(binding.packaged_asset_roots, 2);
        assert!(binding.digest.starts_with("sha256:v1:"));
        assert_eq!(
            binding,
            denominator.binding(),
            "the digest is deterministic"
        );
    }

    #[test]
    fn validation_rejects_drift_between_flag_and_classification() {
        let mut receipt = classify(&["Cargo.lock"]);
        receipt.rehearsal_required = false;
        assert!(validate_release_impact_receipt_v1(&receipt).is_err());
    }

    #[test]
    fn validation_rejects_schema_identity_drift() {
        let mut receipt = classify(&["docs/README.md"]);
        receipt.schema_id = "cargo-allow.release-impact.v2".to_string();
        assert!(validate_release_impact_receipt_v1(&receipt).is_err());
    }

    #[test]
    fn a_valid_unrelated_receipt_validates() {
        let receipt = classify(&["docs/README.md"]);
        assert!(validate_release_impact_receipt_v1(&receipt).is_ok());
        let text = render_release_impact_receipt_json(&receipt)
            .unwrap_or_else(|error| std::panic::panic_any(format!("render: {error}")));
        let parsed: CargoAllowReleaseImpactReceiptV1 = serde_json::from_str(&text)
            .unwrap_or_else(|error| std::panic::panic_any(format!("round trip: {error}")));
        assert_eq!(parsed, receipt);
    }

    fn executed_receipt() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "1.0",
            "receipt_id": "rehearsal-fixture",
            "commit_sha": HEAD,
            "aggregate_status": "Incomplete",
            "phases": {
                "release_identity": "Complete",
                "candidate_package_set": "Complete",
                "shared_prerequisites": "Complete",
                "publisher_state_machine": "Complete",
                "docs_and_support_identity": "Complete",
                "manifest_and_assets": "Complete",
                "authorization_boundary": "Incomplete",
                "workflow_graph_permissions": "Complete"
            }
        })
    }

    #[test]
    fn executed_observations_bind_the_exact_subject_verbatim() {
        let impact = classify(&["Cargo.lock"]);
        let observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:aa",
            ReleaseRehearsalObservationOutcomeV1::Executed,
            Some(1),
            Some(&executed_receipt()),
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        assert_eq!(
            observation.outcome,
            ReleaseRehearsalObservationOutcomeV1::Executed
        );
        assert_eq!(observation.harness_exit_code, Some(1));
        assert_eq!(
            observation.harness_aggregate_status.as_deref(),
            Some("Incomplete")
        );
        assert_eq!(
            observation.harness_commit_sha.as_deref(),
            Some(HEAD),
            "the observation binds the exact head"
        );
        assert_eq!(observation.harness_phase_statuses.len(), 8);
    }

    #[test]
    fn an_executed_observation_requires_real_invocation_evidence() {
        let impact = classify(&["Cargo.lock"]);
        assert!(
            observe_release_rehearsal_v1(
                &impact,
                "sha256:v1:aa",
                ReleaseRehearsalObservationOutcomeV1::Executed,
                None,
                Some(&executed_receipt()),
            )
            .is_err(),
            "no exit code cannot prove invocation"
        );
        assert!(
            observe_release_rehearsal_v1(
                &impact,
                "sha256:v1:aa",
                ReleaseRehearsalObservationOutcomeV1::Executed,
                Some(0),
                None,
            )
            .is_err(),
            "a missing receipt cannot prove invocation"
        );
        let mut foreign = executed_receipt();
        foreign["commit_sha"] =
            serde_json::Value::String("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string());
        assert!(
            observe_release_rehearsal_v1(
                &impact,
                "sha256:v1:aa",
                ReleaseRehearsalObservationOutcomeV1::Executed,
                Some(0),
                Some(&foreign),
            )
            .is_err(),
            "a foreign-subject harness receipt must not bind"
        );
    }

    #[test]
    fn not_applicable_observations_carry_no_harness_evidence() {
        let impact = classify(&["docs/README.md"]);
        let observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:bb",
            ReleaseRehearsalObservationOutcomeV1::NotApplicable,
            None,
            None,
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        assert_eq!(
            observation.outcome,
            ReleaseRehearsalObservationOutcomeV1::NotApplicable
        );
        assert!(!observation.rehearsal_required);
        assert!(
            observe_release_rehearsal_v1(
                &impact,
                "sha256:v1:bb",
                ReleaseRehearsalObservationOutcomeV1::NotApplicable,
                Some(0),
                Some(&executed_receipt()),
            )
            .is_err(),
            "NotApplicable must not carry harness evidence"
        );
    }

    #[test]
    fn the_terminal_aggregate_passes_a_consistent_required_pair() {
        let impact = classify(&["Cargo.lock"]);
        let observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:aa",
            ReleaseRehearsalObservationOutcomeV1::Executed,
            Some(1),
            Some(&executed_receipt()),
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&impact),
            Some("sha256:v1:aa"),
            Some(Ok(&observation)),
            &subject(),
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Passed
        );
        assert!(result.failure_reasons.is_empty());
        assert_eq!(
            result.harness_aggregate_status.as_deref(),
            Some("Incomplete")
        );
    }

    #[test]
    fn the_terminal_aggregate_passes_a_consistent_not_applicable_pair() {
        let impact = classify(&["docs/README.md"]);
        let observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:bb",
            ReleaseRehearsalObservationOutcomeV1::NotApplicable,
            None,
            None,
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&impact),
            Some("sha256:v1:bb"),
            Some(Ok(&observation)),
            &subject(),
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Passed
        );
    }

    #[test]
    fn missing_observation_is_not_clean() {
        let impact = classify(&["Cargo.lock"]);
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&impact),
            Some("sha256:v1:aa"),
            None,
            &subject(),
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Failed
        );
        assert!(
            result
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("rehearsal proof missing")),
            "a skipped/cancelled/missing rehearsal is not clean: {:?}",
            result.failure_reasons
        );
    }

    #[test]
    fn stale_subjects_are_not_clean() {
        let impact = classify(&["Cargo.lock"]);
        let observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:aa",
            ReleaseRehearsalObservationOutcomeV1::Executed,
            Some(1),
            Some(&executed_receipt()),
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        let moved_subject = ReleaseImpactSubjectV1 {
            base_sha: BASE.to_string(),
            head_sha: HEAD.to_string(),
            merge_base_sha: "dddddddddddddddddddddddddddddddddddddddd".to_string(),
        };
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&impact),
            Some("sha256:v1:aa"),
            Some(Ok(&observation)),
            &moved_subject,
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Failed
        );
        assert!(
            result
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("not the expected subject")),
            "a base movement with an identical head stales the pair: {:?}",
            result.failure_reasons
        );
    }

    #[test]
    fn a_sibling_impact_artifact_cannot_authorize_this_subject() {
        let impact = classify(&["Cargo.lock"]);
        let observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:aa",
            ReleaseRehearsalObservationOutcomeV1::Executed,
            Some(1),
            Some(&executed_receipt()),
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&impact),
            Some("sha256:v1:other"),
            Some(Ok(&observation)),
            &subject(),
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Failed
        );
        assert!(
            result
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("digest mismatch")),
            "a retained sibling receipt cannot authorize a different head: {:?}",
            result.failure_reasons
        );
    }

    #[test]
    fn routing_contradictions_are_not_clean() {
        let sensitive = classify(&["Cargo.lock"]);
        let unrelated = classify(&["docs/README.md"]);
        let not_applicable = observe_release_rehearsal_v1(
            &unrelated,
            "sha256:v1:bb",
            ReleaseRehearsalObservationOutcomeV1::NotApplicable,
            None,
            None,
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&sensitive),
            Some("sha256:v1:aa"),
            Some(Ok(&not_applicable)),
            &subject(),
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Failed
        );
        assert!(
            result
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("routing contradiction")),
            "{:?}",
            result.failure_reasons
        );
    }

    #[test]
    fn an_uploaded_but_semantically_failed_pair_is_not_clean() {
        // Fixture 10: upload-artifact succeeds after semantic failure.
        // The artifact exists; the observation inside it claims Executed
        // but carries no invocation evidence, so the aggregate fails
        // even though the upload step itself would be green.
        let impact = classify(&["Cargo.lock"]);
        let mut observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:aa",
            ReleaseRehearsalObservationOutcomeV1::Executed,
            Some(1),
            Some(&executed_receipt()),
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        observation.harness_phase_statuses.clear();
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&impact),
            Some("sha256:v1:aa"),
            Some(Ok(&observation)),
            &subject(),
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Failed
        );
        assert!(
            result
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("complete invocation evidence")),
            "{:?}",
            result.failure_reasons
        );
    }

    #[test]
    fn a_malformed_impact_receipt_fails_the_aggregate() {
        let result = evaluate_release_rehearsal_required_v1(
            Err("unexpected end of input".to_string()),
            None,
            None,
            &subject(),
        );
        assert_eq!(
            result.aggregate,
            ReleaseRehearsalRequiredAggregateV1::Failed
        );
        assert!(
            result
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("impact receipt unreadable or malformed")),
            "{:?}",
            result.failure_reasons
        );
    }

    #[test]
    fn the_required_result_round_trips() {
        let impact = classify(&["Cargo.lock"]);
        let observation = observe_release_rehearsal_v1(
            &impact,
            "sha256:v1:aa",
            ReleaseRehearsalObservationOutcomeV1::Executed,
            Some(1),
            Some(&executed_receipt()),
        )
        .unwrap_or_else(|error| std::panic::panic_any(format!("observe: {error}")));
        let result = evaluate_release_rehearsal_required_v1(
            Ok(&impact),
            Some("sha256:v1:aa"),
            Some(Ok(&observation)),
            &subject(),
        );
        let text = render_release_rehearsal_required_json(&result)
            .unwrap_or_else(|error| std::panic::panic_any(format!("render: {error}")));
        let parsed: CargoAllowReleaseRehearsalRequiredResultV1 = serde_json::from_str(&text)
            .unwrap_or_else(|error| std::panic::panic_any(format!("round trip: {error}")));
        assert_eq!(parsed, result);
    }

    #[test]
    fn git_object_names_fail_closed() {
        assert!(is_git_object_name(BASE));
        assert!(is_git_object_name(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
        assert!(!is_git_object_name("HEAD"));
        assert!(!is_git_object_name(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        ));
        assert!(!is_git_object_name("aaaa"));
    }
}
