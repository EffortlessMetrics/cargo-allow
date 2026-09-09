//! Policy and proof enrichment for one exact dependency graph delta
//! (#3920 PR C): every compiled delta row is attached to the evidence
//! the linked authorities hold for that exact package identity, and
//! the row's disposition states what that evidence supports.
//!
//! The graph delta does not duplicate other policy owners. It links
//! each affected row to the exact evidence that applies: #3903
//! declared direct floors and MSRV proof, #2038 selected dependency
//! feature policy, #1897 cargo-deny results, #2924 packaging
//! evidence, and #3359 per-product support selection. A cargo-deny
//! pass does not erase a downgrade or feature expansion; a graph
//! delta does not itself prove the new dependency is safe or
//! compatible. Model and bot comments are never graph authority: the
//! bundle carries typed references only, and any free-text reference
//! fails closed as malformed input.

use serde::{Deserialize, Serialize};

use crate::artifacts::dependency_graph_delta_v1::{
    DependencyGraphDeltaIdentityV1, DependencyGraphDeltaKindV1, DependencyGraphDeltaReceiptV1,
};

pub const DEPENDENCY_GRAPH_EVIDENCE_SCHEMA_ID: &str = "cargo-allow.dependency-graph-evidence.v1";
pub const DEPENDENCY_GRAPH_EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// Upper bounds keeping one receipt reviewable; a bundle or delta
/// beyond them is malformed input, not a larger denominator.
pub const DEPENDENCY_GRAPH_EVIDENCE_MAX_ROWS: usize = 2048;
pub const DEPENDENCY_GRAPH_EVIDENCE_MAX_RECORDS: usize = 8192;
pub const DEPENDENCY_GRAPH_EVIDENCE_MAX_REF_LEN: usize = 256;

/// The evidence authorities that can bind to one delta row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyEvidenceAuthorityV1 {
    /// #3903 declared direct floors and MSRV proof.
    MinimumVersion,
    /// #2038 selected dependency feature policy.
    DependencyFeature,
    /// #1897 cargo-deny advisories/bans/licenses/sources results.
    CargoDeny,
    /// #2924 packaging and package-candidate evidence.
    PackageCandidate,
    /// #3359 per-product support selection.
    SupportMatrix,
}

impl DependencyEvidenceAuthorityV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MinimumVersion => "minimum_version",
            Self::DependencyFeature => "dependency_feature",
            Self::CargoDeny => "cargo_deny",
            Self::PackageCandidate => "package_candidate",
            Self::SupportMatrix => "support_matrix",
        }
    }

    /// The typed reference scheme this authority's records must carry.
    #[must_use]
    pub const fn reference_scheme(self) -> &'static str {
        match self {
            Self::MinimumVersion => "receipt",
            Self::DependencyFeature => "policy",
            Self::CargoDeny => "deny",
            Self::PackageCandidate => "receipt",
            Self::SupportMatrix => "policy",
        }
    }
}

/// What the attached evidence supports for one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyEvidenceDispositionV1 {
    /// The row moved and no authority resolved it.
    ObservedMovement,
    /// An owning authority flags this row (deny ban, floor violation).
    PolicyFinding,
    /// Current evidence binds this exact row identity.
    EvidenceCurrent,
    /// An in-scope authority has no current evidence for this row.
    EvidenceMissing,
    /// Advisory or unknown external evidence is retained and needs a
    /// human selection; it never upgrades the row to clean.
    NeedsDecision,
}

impl DependencyEvidenceDispositionV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObservedMovement => "observed_movement",
            Self::PolicyFinding => "policy_finding",
            Self::EvidenceCurrent => "evidence_current",
            Self::EvidenceMissing => "evidence_missing",
            Self::NeedsDecision => "needs_decision",
        }
    }
}

/// One evidence record supplied by a CI/dev producer for one exact
/// package identity. Free-text model or bot commentary has no field
/// to land in: `reference` must be `<scheme>:<opaque>` with the
/// authority's expected scheme and no whitespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEvidenceRecordV1 {
    pub authority: DependencyEvidenceAuthorityV1,
    pub package_name: String,
    /// The exact resolved version the evidence covers.
    pub version: String,
    /// Typed reference (`receipt:...`, `policy:...`, `deny:...`).
    pub reference: String,
    /// The authority flags this row as a policy finding.
    pub finding: bool,
    /// Advisory evidence is retained but never resolves a row clean.
    pub advisory: bool,
}

/// The bundle one producer run supplies for one exact delta. The
/// base/head/product/target identity must equal the delta receipt's:
/// identity movement stales the bundle and fails closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEvidenceBundleV1 {
    /// Authorities whose coverage is expected for moved rows in this
    /// run; a moved row without a binding record for an in-scope
    /// authority is EvidenceMissing.
    pub authorities_in_scope: Vec<DependencyEvidenceAuthorityV1>,
    pub base_commit: String,
    pub head_commit: String,
    pub product: String,
    pub target: String,
    pub records: Vec<DependencyEvidenceRecordV1>,
}

/// One attachment: a single authority's verdict on one row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEvidenceAttachmentV1 {
    pub authority: DependencyEvidenceAuthorityV1,
    pub disposition: DependencyEvidenceDispositionV1,
    pub reference: String,
    pub advisory: bool,
}

/// One delta row enriched with its evidence adjacency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphEvidenceRowV1 {
    pub package_name: String,
    pub kind: DependencyGraphDeltaKindV1,
    pub disposition: DependencyEvidenceDispositionV1,
    pub attachments: Vec<DependencyEvidenceAttachmentV1>,
}

/// The receipt-level result. `EvidenceCurrent` requires every row
/// current, no advisory attachment, and a non-empty denominator;
/// anything else is `DecisionRequired`; a failed enrichment
/// (identity mismatch, malformed bundle) never produces a receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphEvidenceResultV1 {
    DecisionRequired,
    EvidenceCurrent,
}

impl DependencyGraphEvidenceResultV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DecisionRequired => "decision_required",
            Self::EvidenceCurrent => "evidence_current",
        }
    }
}

/// The enriched receipt: the exact delta's identity plus per-row
/// evidence adjacency. Machine JSON and human Markdown render from
/// this one semantic result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphEvidenceReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub delta_identity: DependencyGraphDeltaIdentityV1,
    pub rows: Vec<DependencyGraphEvidenceRowV1>,
    pub complete: bool,
    pub result: DependencyGraphEvidenceResultV1,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Validate one typed reference: `<scheme>:<opaque>`, no whitespace,
/// scheme equal to the authority's expected scheme.
#[must_use]
fn reference_is_valid(authority: DependencyEvidenceAuthorityV1, reference: &str) -> bool {
    if reference.is_empty()
        || reference.len() > DEPENDENCY_GRAPH_EVIDENCE_MAX_REF_LEN
        || reference.chars().any(char::is_whitespace)
    {
        return false;
    }
    let Some((scheme, opaque)) = reference.split_once(':') else {
        return false;
    };
    scheme == authority.reference_scheme() && !opaque.is_empty()
}

/// Attach the bundle's evidence to the exact delta receipt and emit
/// the enriched receipt. Pure and deterministic. Fails closed (Err)
/// on identity mismatch, malformed references, or bound overflow —
/// malformed input never becomes a receipt.
pub fn attach_dependency_graph_evidence(
    delta: &DependencyGraphDeltaReceiptV1,
    bundle: &DependencyEvidenceBundleV1,
) -> Result<DependencyGraphEvidenceReceiptV1, String> {
    // Identity movement between the delta and the bundle stales the
    // evidence: an old receipt can never stay current (negative
    // control 10).
    if bundle.base_commit != delta.identity.base_commit
        || bundle.head_commit != delta.identity.head_commit
        || bundle.product != delta.identity.product
        || bundle.target != delta.identity.target
    {
        return Err(
            "evidence bundle identity does not match the delta receipt (base/head/product/target); \
             the evidence is stale for this delta"
                .to_string(),
        );
    }
    if delta.rows.len() > DEPENDENCY_GRAPH_EVIDENCE_MAX_ROWS {
        return Err(format!(
            "delta row count {} exceeds the enrichment bound {DEPENDENCY_GRAPH_EVIDENCE_MAX_ROWS}",
            delta.rows.len()
        ));
    }
    if bundle.records.len() > DEPENDENCY_GRAPH_EVIDENCE_MAX_RECORDS {
        return Err(format!(
            "evidence record count {} exceeds the enrichment bound \
             {DEPENDENCY_GRAPH_EVIDENCE_MAX_RECORDS}",
            bundle.records.len()
        ));
    }
    for record in &bundle.records {
        if !reference_is_valid(record.authority, &record.reference) {
            return Err(format!(
                "evidence reference for {}@{} is not a typed {} reference; model or bot \
                 commentary is never graph authority",
                record.package_name,
                record.version,
                record.authority.as_str()
            ));
        }
    }

    let mut rows = Vec::with_capacity(delta.rows.len());
    for delta_row in &delta.rows {
        let binding_version = if delta_row.kind == DependencyGraphDeltaKindV1::PackageRemoved {
            delta_row.base_version.as_str()
        } else {
            delta_row.head_version.as_str()
        };
        let mut attachments: Vec<DependencyEvidenceAttachmentV1> = Vec::new();
        for record in &bundle.records {
            if record.package_name != delta_row.package_name || record.version != binding_version {
                continue;
            }
            // A current record binds this exact identity. Evidence is
            // attached for what it is: a finding, an advisory note, or
            // current coverage.
            let disposition = if record.finding {
                DependencyEvidenceDispositionV1::PolicyFinding
            } else if record.advisory {
                DependencyEvidenceDispositionV1::NeedsDecision
            } else {
                DependencyEvidenceDispositionV1::EvidenceCurrent
            };
            attachments.push(DependencyEvidenceAttachmentV1 {
                authority: record.authority,
                disposition,
                reference: record.reference.clone(),
                advisory: record.advisory,
            });
        }
        let moved = delta_row.kind.is_semantic()
            && delta_row.kind != DependencyGraphDeltaKindV1::NoSemanticGraphChange;
        if moved {
            // An in-scope authority with no binding record leaves the
            // row's evidence missing — never clean by default.
            for authority in &bundle.authorities_in_scope {
                let bound = attachments
                    .iter()
                    .any(|attachment| attachment.authority == *authority);
                if !bound {
                    attachments.push(DependencyEvidenceAttachmentV1 {
                        authority: *authority,
                        disposition: DependencyEvidenceDispositionV1::EvidenceMissing,
                        reference: String::new(),
                        advisory: false,
                    });
                }
            }
        }
        let disposition = if attachments.iter().any(|attachment| {
            attachment.disposition == DependencyEvidenceDispositionV1::PolicyFinding
        }) {
            DependencyEvidenceDispositionV1::PolicyFinding
        } else if attachments.iter().any(|attachment| {
            attachment.disposition == DependencyEvidenceDispositionV1::NeedsDecision
        }) {
            DependencyEvidenceDispositionV1::NeedsDecision
        } else if attachments.iter().any(|attachment| {
            attachment.disposition == DependencyEvidenceDispositionV1::EvidenceMissing
        }) {
            DependencyEvidenceDispositionV1::EvidenceMissing
        } else if attachments.iter().any(|attachment| {
            attachment.disposition == DependencyEvidenceDispositionV1::EvidenceCurrent
        }) {
            DependencyEvidenceDispositionV1::EvidenceCurrent
        } else {
            DependencyEvidenceDispositionV1::ObservedMovement
        };
        attachments.sort_by(|a, b| {
            a.authority
                .as_str()
                .cmp(b.authority.as_str())
                .then(a.reference.cmp(&b.reference))
        });
        rows.push(DependencyGraphEvidenceRowV1 {
            package_name: delta_row.package_name.clone(),
            kind: delta_row.kind,
            disposition,
            attachments,
        });
    }
    rows.sort_by(|a, b| {
        a.package_name
            .cmp(&b.package_name)
            .then(a.kind.as_str().cmp(b.kind.as_str()))
    });

    // A cargo-deny pass or any other evidence pass never makes the
    // receipt clean: every row must be current with zero advisory
    // residue, and an empty denominator is a decision, not a clean
    // result (negative control 11 and the zero-denominator law).
    let result = if delta.rows.is_empty() || !delta.complete {
        DependencyGraphEvidenceResultV1::DecisionRequired
    } else if rows.iter().all(|row| {
        row.disposition == DependencyEvidenceDispositionV1::EvidenceCurrent
            && !row.attachments.iter().any(|attachment| attachment.advisory)
    }) {
        DependencyGraphEvidenceResultV1::EvidenceCurrent
    } else {
        DependencyGraphEvidenceResultV1::DecisionRequired
    };

    Ok(DependencyGraphEvidenceReceiptV1 {
        schema_id: DEPENDENCY_GRAPH_EVIDENCE_SCHEMA_ID.to_string(),
        schema_version: DEPENDENCY_GRAPH_EVIDENCE_SCHEMA_VERSION,
        delta_identity: delta.identity.clone(),
        rows,
        complete: delta.complete,
        result,
        limitations: vec![
            "enrichment binds evidence by exact package name and resolved version; features, targets, and dependency classes need selected metadata observations".to_string(),
            "model and bot commentary has no field in the bundle; only typed references are retained".to_string(),
        ],
        claim_boundary: "Evidence adjacency for one exact delta: the delta row remains the only graph authority. A cargo-deny pass does not erase a downgrade; advisory evidence stays visible as NeedsDecision; missing in-scope evidence is EvidenceMissing, never clean. Enrichment does not prove a dependency safe or compatible.".to_string(),
    })
}

/// Render the receipt as deterministic human Markdown from the same
/// semantic result the JSON carries.
#[must_use]
fn render_dependency_graph_evidence_human(receipt: &DependencyGraphEvidenceReceiptV1) -> String {
    let mut out = String::new();
    out.push_str("# Dependency graph evidence\n\n");
    out.push_str(&format!(
        "**Result:** `{}` (complete: `{}`)\n\n",
        receipt.result.as_str(),
        receipt.complete
    ));
    out.push_str(&format!(
        "Delta: `{}` `{}` base `{}` head `{}` product `{}` target `{}`\n\n",
        receipt.delta_identity.product,
        receipt.delta_identity.target,
        receipt.delta_identity.base_commit,
        receipt.delta_identity.head_commit,
        receipt.delta_identity.product,
        receipt.delta_identity.target
    ));
    out.push_str("| Row | Kind | Disposition | Attachments |\n|---|---|---|---|\n");
    for row in &receipt.rows {
        let attachments = if row.attachments.is_empty() {
            String::from("—")
        } else {
            row.attachments
                .iter()
                .map(|attachment| {
                    format!(
                        "{} `{}`{}",
                        attachment.authority.as_str(),
                        attachment.disposition.as_str(),
                        if attachment.advisory {
                            " (advisory)"
                        } else {
                            ""
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            row.package_name,
            row.kind.as_str(),
            row.disposition.as_str(),
            attachments
        ));
    }
    out.push_str("\n> Claim boundary: ");
    out.push_str(&receipt.claim_boundary);
    out.push('\n');
    out
}

/// Render the receipt as deterministic JSON.
///
/// # Errors
/// Serialization of a bounded receipt cannot fail in practice; the
/// error is propagated for pipeline honesty.
pub fn render_dependency_graph_evidence_json(
    receipt: &DependencyGraphEvidenceReceiptV1,
) -> Result<String, String> {
    serde_json::to_string_pretty(receipt).map_err(|error| format!("evidence json renders: {error}"))
}

/// Render the receipt in the requested machine or human format.
///
/// # Errors
/// JSON serialization errors propagate.
pub fn render_dependency_graph_evidence(
    receipt: &DependencyGraphEvidenceReceiptV1,
    format: DependencyGraphEvidenceFormat,
) -> Result<String, String> {
    match format {
        DependencyGraphEvidenceFormat::Json => render_dependency_graph_evidence_json(receipt),
        DependencyGraphEvidenceFormat::Human => Ok(render_dependency_graph_evidence_human(receipt)),
    }
}

/// Output rendering selector for the evidence receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyGraphEvidenceFormat {
    Json,
    Human,
}
