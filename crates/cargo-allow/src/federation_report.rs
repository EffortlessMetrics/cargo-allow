use allow_policy::federation::FederationEvaluation;
use allow_report::{
    FederationDivergenceKindCount, FederationDivergenceRecordSummary, FederationDivergenceSummary,
    FederationReportContext, LedgerContributorSummary,
};
use std::collections::BTreeMap;

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
    registry_validity: &'static str,
    divergence_kinds: Vec<String>,
    divergence_messages: Vec<String>,
    divergence_canonical_ids: Vec<String>,
    divergence_mirror_ids: Vec<String>,
    divergence_canonical_paths: Vec<String>,
    divergence_mirror_paths: Vec<String>,
    divergence_sample_ids: Vec<Vec<String>>,
    divergence_canonical_fingerprints: Vec<Option<String>>,
    divergence_mirror_fingerprints: Vec<Option<String>>,
    divergence_actions: Vec<&'static str>,
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
            registry_validity: evaluation.registry_validity.as_str(),
            divergence_kinds: Vec::with_capacity(evaluation.divergences.len()),
            divergence_messages: Vec::with_capacity(evaluation.divergences.len()),
            divergence_canonical_ids: Vec::with_capacity(evaluation.divergences.len()),
            divergence_mirror_ids: Vec::with_capacity(evaluation.divergences.len()),
            divergence_canonical_paths: Vec::with_capacity(evaluation.divergences.len()),
            divergence_mirror_paths: Vec::with_capacity(evaluation.divergences.len()),
            divergence_sample_ids: Vec::with_capacity(evaluation.divergences.len()),
            divergence_canonical_fingerprints: Vec::with_capacity(evaluation.divergences.len()),
            divergence_mirror_fingerprints: Vec::with_capacity(evaluation.divergences.len()),
            divergence_actions: Vec::with_capacity(evaluation.divergences.len()),
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
        for record in &evaluation.divergences {
            bundle
                .divergence_kinds
                .push(record.kind.as_str().to_string());
            bundle.divergence_messages.push(record.message.clone());
            bundle
                .divergence_canonical_ids
                .push(record.canonical_ledger_id.clone());
            bundle
                .divergence_mirror_ids
                .push(record.mirror_ledger_id.clone());
            bundle
                .divergence_canonical_paths
                .push(record.canonical_path.clone());
            bundle
                .divergence_mirror_paths
                .push(record.mirror_path.clone());
            bundle
                .divergence_sample_ids
                .push(record.sample_entry_ids.clone());
            bundle
                .divergence_canonical_fingerprints
                .push(record.canonical_fingerprint.clone());
            bundle
                .divergence_mirror_fingerprints
                .push(record.mirror_fingerprint.clone());
            bundle.divergence_actions.push(record.recommended_action);
        }
        bundle
    }

    pub(crate) fn mirror_divergence_advisory_count(&self) -> usize {
        self.divergence_kinds
            .iter()
            .filter(|kind| matches!(kind.as_str(), "mirror_divergence" | "mirror_stale"))
            .count()
    }

    /// Count of blocking divergences (`drain_expired`). These fail the run, so
    /// the receipt records them separately from the advisory mirror count
    /// rather than leaving the operator to read a zero while CI fails (#1945).
    pub(crate) fn blocking_divergence_count(&self) -> usize {
        self.divergence_kinds
            .iter()
            .filter(|kind| kind.as_str() == "drain_expired")
            .count()
    }

    pub(crate) fn has_blocking_divergence(&self) -> bool {
        self.blocking_divergence_count() > 0
    }

    /// True when a parsed `.allow/config.toml` registry failed validation and
    /// the run fell back to the discovered ledger (#4655). That combination is
    /// a silent fail-open — a stricter registered ledger stops being enforced
    /// while the gate stays green — so `check` treats it as blocking.
    ///
    /// An explicit `--config` override is deliberately excluded: the operator
    /// named the ledger themselves, so nothing about the selection is silent.
    /// This matches `doctor`, which only flags the invalid registry at
    /// `discovery_fallback` precedence.
    pub(crate) fn registry_invalid_fallback(&self) -> bool {
        self.precedence_applied == "discovery_fallback" && self.registry_validity == "invalid"
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
        let divergence_records = (0..self.divergence_kinds.len())
            .filter_map(|index| {
                Some(FederationDivergenceRecordSummary {
                    kind: self.divergence_kinds.get(index)?.as_str(),
                    message: self.divergence_messages.get(index)?.as_str(),
                    canonical_ledger_id: self.divergence_canonical_ids.get(index)?.as_str(),
                    mirror_ledger_id: self.divergence_mirror_ids.get(index)?.as_str(),
                    canonical_path: self.divergence_canonical_paths.get(index)?.as_str(),
                    mirror_path: self.divergence_mirror_paths.get(index)?.as_str(),
                    sample_entry_ids: self.divergence_sample_ids.get(index)?.as_slice(),
                    canonical_fingerprint: self
                        .divergence_canonical_fingerprints
                        .get(index)?
                        .as_deref(),
                    mirror_fingerprint: self.divergence_mirror_fingerprints.get(index)?.as_deref(),
                    recommended_action: self.divergence_actions.get(index)?,
                })
            })
            .collect::<Vec<_>>();
        let mut counts = BTreeMap::<&str, usize>::new();
        for kind in &self.divergence_kinds {
            *counts.entry(kind.as_str()).or_default() += 1;
        }
        let kind_counts = counts
            .into_iter()
            .map(|(kind, count)| FederationDivergenceKindCount { kind, count })
            .collect::<Vec<_>>();
        let divergence_summary = if divergence_records.is_empty() {
            None
        } else {
            Some(FederationDivergenceSummary {
                records: Some(divergence_records.as_slice()),
                counts_by_kind: Some(kind_counts.as_slice()),
            })
        };
        let context = FederationReportContext {
            federation_version: Some(self.federation_version),
            precedence_applied: Some(self.precedence_applied),
            registry_validity: Some(self.registry_validity),
            ledger_contributors: if summaries.is_empty() {
                None
            } else {
                Some(summaries.as_slice())
            },
            divergence_summary,
        };
        f(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_policy::federation::{FederationRegistryValidity, PrecedenceTier};

    fn evaluation(
        precedence: PrecedenceTier,
        validity: FederationRegistryValidity,
    ) -> FederationEvaluation {
        FederationEvaluation {
            federation_version: allow_policy::federation::FEDERATION_VERSION,
            precedence_applied: precedence,
            registry_validity: validity,
            active_provenance: None,
            ledger_contributors: Vec::new(),
            divergences: Vec::new(),
        }
    }

    #[test]
    fn registry_invalid_fallback_flags_discovery_fallback_with_invalid_registry() {
        // #4655: exactly the combination the issue pins — the registry parsed
        // but failed validation, so the gate silently reverted to discovery.
        let bundle = FederationReportBundle::from_evaluation(&evaluation(
            PrecedenceTier::DiscoveryFallback,
            FederationRegistryValidity::Invalid,
        ));
        assert!(
            bundle.registry_invalid_fallback(),
            "invalid-registry discovery fallback must be flagged"
        );
    }

    #[test]
    fn registry_invalid_fallback_ignores_absent_and_valid_registries() {
        for validity in [
            FederationRegistryValidity::Absent,
            FederationRegistryValidity::Valid,
        ] {
            let bundle = FederationReportBundle::from_evaluation(&evaluation(
                PrecedenceTier::DiscoveryFallback,
                validity,
            ));
            assert!(
                !bundle.registry_invalid_fallback(),
                "validity {validity:?} at discovery fallback must not be flagged"
            );
        }
    }

    #[test]
    fn registry_invalid_fallback_ignores_cli_override_despite_invalid_registry() {
        // An explicit --config names the ledger itself, so nothing about the
        // selection is silent; doctor draws the same boundary.
        let bundle = FederationReportBundle::from_evaluation(&evaluation(
            PrecedenceTier::CliOverride,
            FederationRegistryValidity::Invalid,
        ));
        assert!(
            !bundle.registry_invalid_fallback(),
            "a CLI override must not be flagged as a silent invalid-registry fallback"
        );
    }

    #[test]
    fn with_context_threads_registry_validity_into_the_report_context() {
        let bundle = FederationReportBundle::from_evaluation(&evaluation(
            PrecedenceTier::DiscoveryFallback,
            FederationRegistryValidity::Invalid,
        ));
        bundle.with_context(|context| {
            assert_eq!(
                context.registry_validity,
                Some(FederationRegistryValidity::Invalid.as_str()),
                "report context must carry the registry validity"
            );
            assert_eq!(
                context.precedence_applied,
                Some(PrecedenceTier::DiscoveryFallback.as_str())
            );
        });
    }
}
