use allow_policy::federation::FederationEvaluation;
use allow_report::{FederationReportContext, LedgerContributorSummary};

pub(crate) struct FederationReportBundle {
    ids: Vec<String>,
    paths: Vec<String>,
    roles: Vec<String>,
    dialects: Vec<String>,
    modes: Vec<String>,
    priorities: Vec<u32>,
    lanes: Vec<Vec<String>>,
    federation_version: &'static str,
    precedence_applied: &'static str,
}

impl FederationReportBundle {
    pub(crate) fn from_evaluation(evaluation: &FederationEvaluation) -> Self {
        let mut bundle = Self {
            ids: Vec::with_capacity(evaluation.ledger_contributors.len()),
            paths: Vec::with_capacity(evaluation.ledger_contributors.len()),
            roles: Vec::with_capacity(evaluation.ledger_contributors.len()),
            dialects: Vec::with_capacity(evaluation.ledger_contributors.len()),
            modes: Vec::with_capacity(evaluation.ledger_contributors.len()),
            priorities: Vec::with_capacity(evaluation.ledger_contributors.len()),
            lanes: Vec::with_capacity(evaluation.ledger_contributors.len()),
            federation_version: evaluation.federation_version,
            precedence_applied: evaluation.precedence_applied.as_str(),
        };
        for contributor in &evaluation.ledger_contributors {
            bundle.ids.push(contributor.id.clone());
            bundle.paths.push(contributor.path.clone());
            bundle.roles.push(contributor.role.as_str().to_string());
            bundle.dialects.push(contributor.dialect.clone());
            bundle.modes.push(contributor.mode.as_str().to_string());
            bundle.priorities.push(contributor.priority);
            bundle.lanes.push(contributor.lanes.clone());
        }
        bundle
    }

    pub(crate) fn with_context<R>(&self, f: impl FnOnce(FederationReportContext<'_>) -> R) -> R {
        let summaries = self
            .ids
            .iter()
            .zip(&self.paths)
            .zip(&self.roles)
            .zip(&self.dialects)
            .zip(&self.modes)
            .zip(&self.priorities)
            .zip(&self.lanes)
            .map(
                |((((((id, path), role), dialect), mode), priority), lanes)| {
                    LedgerContributorSummary {
                        id: id.as_str(),
                        path: path.as_str(),
                        role: role.as_str(),
                        dialect: dialect.as_str(),
                        mode: mode.as_str(),
                        priority: *priority,
                        lanes: lanes.as_slice(),
                    }
                },
            )
            .collect::<Vec<_>>();
        let context = FederationReportContext {
            federation_version: Some(self.federation_version),
            precedence_applied: Some(self.precedence_applied),
            ledger_contributors: if summaries.is_empty() {
                None
            } else {
                Some(summaries.as_slice())
            },
        };
        f(context)
    }
}
