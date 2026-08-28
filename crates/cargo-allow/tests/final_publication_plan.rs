use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RowClass {
    UploadCandidate,
    ExistingSharedPrerequisite,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RowStatus {
    Pending,
    AlreadyExact,
    ReadyToUpload,
    WaitingOnPrerequisite,
    WaitingOnObservation,
    Conflict,
    RecoveryRequired,
    NotApplicable,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowPublicationRowV1 {
    pub package_id: String,
    pub exact_version: String,
    pub row_class: RowClass,
    pub candidate_sha256: Option<String>,
    pub normalized_dependencies: Vec<String>,
    pub prerequisite_package_ids: Vec<String>,
    pub publication_order: usize,
    pub current_status: RowStatus,
    pub tie_break_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowPublicationUnlockV1 {
    pub tag_remote_observed: bool,
    pub lease_valid: bool,
    pub prerequisites_all_exact: bool,
    pub checkpoint_agreed: bool,
    pub no_unrecovered_incidents: bool,
    pub archive_custody_valid: bool,
    pub preflight_non_conflicting: bool,
}

impl CargoAllowPublicationUnlockV1 {
    pub fn is_ready_to_upload(&self) -> bool {
        self.tag_remote_observed
            && self.lease_valid
            && self.prerequisites_all_exact
            && self.checkpoint_agreed
            && self.no_unrecovered_incidents
            && self.archive_custody_valid
            && self.preflight_non_conflicting
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowPublicationPlanV1 {
    pub schema_version: String,
    pub plan_id: String,
    pub rows: Vec<CargoAllowPublicationRowV1>,
}

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

pub fn derive_publication_plan(
    dependencies: &BTreeMap<&str, Vec<&str>>,
    shared_prerequisites: &[&str],
) -> Result<CargoAllowPublicationPlanV1, io::Error> {
    let mut rows = Vec::new();
    let mut visited = BTreeSet::new();
    let mut order = 0;

    // First add shared prerequisites as AlreadyExact
    for shared in shared_prerequisites {
        rows.push(CargoAllowPublicationRowV1 {
            package_id: shared.to_string(),
            exact_version: "0.1.0".to_string(),
            row_class: RowClass::ExistingSharedPrerequisite,
            candidate_sha256: None,
            normalized_dependencies: Vec::new(),
            prerequisite_package_ids: Vec::new(),
            publication_order: order,
            current_status: RowStatus::AlreadyExact,
            tie_break_reason: "Pre-existing immutable shared prerequisite 0.1.0".to_string(),
        });
        visited.insert(shared.to_string());
        order += 1;
    }

    // Topological derivation for remaining cargo-allow packages
    let mut remaining: BTreeMap<&str, Vec<&str>> = dependencies.clone();

    while !remaining.is_empty() {
        let mut progress = false;
        let keys: Vec<&str> = remaining.keys().copied().collect();

        for key in keys {
            let empty_deps = Vec::new();
            let deps = remaining.get(key).unwrap_or(&empty_deps);
            let all_deps_visited = deps.iter().all(|d| visited.contains(*d));

            if all_deps_visited {
                rows.push(CargoAllowPublicationRowV1 {
                    package_id: key.to_string(),
                    exact_version: "0.2.0".to_string(),
                    row_class: RowClass::UploadCandidate,
                    candidate_sha256: Some(format!("mock-sha256-{key}")),
                    normalized_dependencies: deps
                        .iter()
                        .map(|d| format!("{d} = \"=0.2.0\""))
                        .collect(),
                    prerequisite_package_ids: deps.iter().map(|d| d.to_string()).collect(),
                    publication_order: order,
                    current_status: RowStatus::Pending,
                    tie_break_reason:
                        "Topologically unblocked with all dependencies in visited set".to_string(),
                });
                visited.insert(key.to_string());
                remaining.remove(key);
                order += 1;
                progress = true;
                break; // Deterministic step
            }
        }

        if !progress {
            return Err(io::Error::other(
                "Cycle or missing dependency in publication DAG",
            ));
        }
    }

    Ok(CargoAllowPublicationPlanV1 {
        schema_version: "1.0".to_string(),
        plan_id: "cargo-allow-0.2.0-clean-plan-v1".to_string(),
        rows,
    })
}

#[test]
fn test_publication_dag_derivation_and_ordering() -> Result<(), Box<dyn Error>> {
    let mut deps = BTreeMap::new();
    deps.insert("allow-core", vec!["effortless-repo-protocol"]);
    deps.insert("allow-policy", vec!["allow-core"]);
    deps.insert("allow-policy-legacy", vec!["allow-core", "allow-policy"]);
    deps.insert("allow-inventory", vec!["allow-core"]);
    deps.insert("allow-files", vec!["allow-core"]);
    deps.insert("allow-rust", vec!["allow-core"]);
    deps.insert("allow-match", vec!["allow-core"]);
    deps.insert("allow-report", vec!["allow-core"]);
    deps.insert("allow-diff", vec!["allow-core", "allow-policy"]);
    deps.insert(
        "cargo-allow",
        vec![
            "allow-core",
            "allow-policy",
            "allow-inventory",
            "allow-diff",
        ],
    );

    let shared = vec![
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
        "effortless-repo-edit",
    ];

    let plan = derive_publication_plan(&deps, &shared)?;

    // Must have 13 total rows (3 shared + 10 upload)
    require(plan.rows.len() == 13, "plan must contain exactly 13 rows")?;

    // First 3 rows must be ExistingSharedPrerequisite
    for i in 0..3 {
        let row = plan
            .rows
            .get(i)
            .ok_or_else(|| io::Error::other("missing row"))?;
        require(
            row.row_class == RowClass::ExistingSharedPrerequisite,
            "first rows must be shared prerequisites",
        )?;
        require(
            row.current_status == RowStatus::AlreadyExact,
            "shared prerequisites must be AlreadyExact",
        )?;
    }

    // allow-core must appear before cargo-allow
    let core_idx = plan
        .rows
        .iter()
        .position(|r| r.package_id == "allow-core")
        .ok_or_else(|| io::Error::other("missing allow-core"))?;
    let cli_idx = plan
        .rows
        .iter()
        .position(|r| r.package_id == "cargo-allow")
        .ok_or_else(|| io::Error::other("missing cargo-allow"))?;
    require(
        core_idx < cli_idx,
        "allow-core must be published before cargo-allow",
    )?;

    Ok(())
}

#[test]
fn test_unlock_conditions() -> Result<(), Box<dyn Error>> {
    let ready_unlock = CargoAllowPublicationUnlockV1 {
        tag_remote_observed: true,
        lease_valid: true,
        prerequisites_all_exact: true,
        checkpoint_agreed: true,
        no_unrecovered_incidents: true,
        archive_custody_valid: true,
        preflight_non_conflicting: true,
    };
    require(
        ready_unlock.is_ready_to_upload(),
        "all preconditions true must be ready",
    )?;

    // Negative control: tag not observed
    let mut unready = ready_unlock.clone();
    unready.tag_remote_observed = false;
    require(
        !unready.is_ready_to_upload(),
        "tag unobserved must block upload",
    )?;

    // Negative control: unrecovered incidents
    let mut incident = ready_unlock.clone();
    incident.no_unrecovered_incidents = false;
    require(
        !incident.is_ready_to_upload(),
        "unrecovered incidents must block upload",
    )?;

    // Negative control: checkpoint disagreement
    let mut chk_mismatch = ready_unlock.clone();
    chk_mismatch.checkpoint_agreed = false;
    require(
        !chk_mismatch.is_ready_to_upload(),
        "checkpoint mismatch must block upload",
    )?;

    Ok(())
}
