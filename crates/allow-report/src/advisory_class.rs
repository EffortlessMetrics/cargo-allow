use crate::ReportContext;
use allow_core::MatchStatus;

use super::{ReviewSignals, Summary};

/// Canonical advisory signal emitted in receipt `advisory` counts and accepted by
/// `check --deny <status>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdvisoryClass {
    ReviewItems,
    New,
    Expired,
    ReviewDue,
    LocationDrift,
    Stale,
    Ambiguous,
    InvalidSelector,
    MissingRequiredField,
    EvidenceMissing,
    BaselineDebt,
    PolicyMissingEvidence,
    BrokenEvidenceLinks,
    WeakEvidenceReferences,
    OccurrenceHeadroom,
    MirrorDivergence,
}

impl AdvisoryClass {
    pub const ALL: &[Self] = &[
        Self::ReviewItems,
        Self::New,
        Self::Expired,
        Self::ReviewDue,
        Self::LocationDrift,
        Self::Stale,
        Self::Ambiguous,
        Self::InvalidSelector,
        Self::MissingRequiredField,
        Self::EvidenceMissing,
        Self::BaselineDebt,
        Self::PolicyMissingEvidence,
        Self::BrokenEvidenceLinks,
        Self::WeakEvidenceReferences,
        Self::OccurrenceHeadroom,
        Self::MirrorDivergence,
    ];

    pub const fn field_name(self) -> &'static str {
        match self {
            Self::ReviewItems => "review_items",
            Self::New => "new",
            Self::Expired => "expired",
            Self::ReviewDue => "review_due",
            Self::LocationDrift => "location_drift",
            Self::Stale => "stale",
            Self::Ambiguous => "ambiguous",
            Self::InvalidSelector => "invalid_selector",
            Self::MissingRequiredField => "missing_required_field",
            Self::EvidenceMissing => "evidence_missing",
            Self::BaselineDebt => "baseline_debt",
            Self::PolicyMissingEvidence => "policy_missing_evidence",
            Self::BrokenEvidenceLinks => "broken_evidence_links",
            Self::WeakEvidenceReferences => "weak_evidence_references",
            Self::OccurrenceHeadroom => "occurrence_headroom",
            Self::MirrorDivergence => "mirror_divergence",
        }
    }

    pub fn parse_field_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|class| class.field_name() == name)
    }

    pub(crate) fn count(self, summary: &Summary, signals: &ReviewSignals) -> usize {
        match self {
            Self::ReviewItems => signals.review_items,
            Self::New => summary.count(MatchStatus::New),
            Self::Expired => summary.count(MatchStatus::Expired),
            Self::ReviewDue => summary.count(MatchStatus::ReviewDue),
            Self::LocationDrift => summary.count(MatchStatus::LocationDrift),
            Self::Stale => summary.count(MatchStatus::Stale),
            Self::Ambiguous => summary.count(MatchStatus::Ambiguous),
            Self::InvalidSelector => summary.count(MatchStatus::InvalidSelector),
            Self::MissingRequiredField => summary.count(MatchStatus::MissingRequiredField),
            Self::EvidenceMissing => summary.count(MatchStatus::EvidenceMissing),
            Self::BaselineDebt => signals.baseline_debt,
            Self::PolicyMissingEvidence => signals.policy_missing_evidence,
            Self::BrokenEvidenceLinks => signals.broken_evidence_links,
            Self::WeakEvidenceReferences => signals.weak_evidence_references,
            Self::OccurrenceHeadroom => signals.occurrence_headroom,
            Self::MirrorDivergence => signals.mirror_divergence,
        }
    }

    pub(crate) fn include_in_receipt(self, summary: &Summary, signals: &ReviewSignals) -> bool {
        let count = self.count(summary, signals);
        match self {
            Self::PolicyMissingEvidence => count > summary.count(MatchStatus::EvidenceMissing),
            Self::BrokenEvidenceLinks
            | Self::WeakEvidenceReferences
            | Self::OccurrenceHeadroom
            | Self::MirrorDivergence => count > 0,
            _ => true,
        }
    }

    pub fn receipt_fields(summary: &Summary, context: ReportContext<'_>) -> Vec<(Self, usize)> {
        let signals = ReviewSignals::from_summary(summary, context);
        Self::ALL
            .iter()
            .copied()
            .filter(|class| class.include_in_receipt(summary, &signals))
            .map(|class| (class, class.count(summary, &signals)))
            .collect()
    }
}

pub const ADVISORY_DENY_FIELD_NAMES: &[&str] = &[
    AdvisoryClass::ReviewItems.field_name(),
    AdvisoryClass::New.field_name(),
    AdvisoryClass::Expired.field_name(),
    AdvisoryClass::ReviewDue.field_name(),
    AdvisoryClass::LocationDrift.field_name(),
    AdvisoryClass::Stale.field_name(),
    AdvisoryClass::Ambiguous.field_name(),
    AdvisoryClass::InvalidSelector.field_name(),
    AdvisoryClass::MissingRequiredField.field_name(),
    AdvisoryClass::EvidenceMissing.field_name(),
    AdvisoryClass::BaselineDebt.field_name(),
    AdvisoryClass::PolicyMissingEvidence.field_name(),
    AdvisoryClass::BrokenEvidenceLinks.field_name(),
    AdvisoryClass::WeakEvidenceReferences.field_name(),
    AdvisoryClass::OccurrenceHeadroom.field_name(),
    AdvisoryClass::MirrorDivergence.field_name(),
];

pub fn advisory_count_for_deny_field(
    summary: &Summary,
    context: ReportContext<'_>,
    field: &str,
) -> Option<usize> {
    let class = AdvisoryClass::parse_field_name(field)?;
    let signals = ReviewSignals::from_summary(summary, context);
    Some(class.count(summary, &signals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::MatchOutcome;

    fn outcome(status: MatchStatus, allow_id: Option<&str>) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: allow_id.map(str::to_string),
            candidate_ids: Vec::new(),
            finding_index: None,
            message: status.as_str().to_string(),
            score: 0,
        }
    }

    #[test]
    fn registry_covers_all_prior_deny_field_names() {
        let prior = [
            "review_items",
            "new",
            "expired",
            "review_due",
            "location_drift",
            "stale",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt",
            "policy_missing_evidence",
            "broken_evidence_links",
            "weak_evidence_references",
            "occurrence_headroom",
            "mirror_divergence",
        ];
        for name in prior {
            assert!(
                AdvisoryClass::parse_field_name(name).is_some(),
                "missing advisory class for `{name}`"
            );
        }
        assert_eq!(ADVISORY_DENY_FIELD_NAMES.len(), prior.len());
        for (index, name) in prior.iter().enumerate() {
            assert_eq!(ADVISORY_DENY_FIELD_NAMES[index], *name);
        }
    }

    #[test]
    fn receipt_fields_omit_zero_optional_signals() {
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some("matched")),
            outcome(MatchStatus::EvidenceMissing, Some("evidence")),
        ];
        let summary = Summary::from_outcomes(&outcomes);
        let context = ReportContext::default();
        let fields = AdvisoryClass::receipt_fields(&summary, context);
        let names = fields
            .iter()
            .map(|(class, _)| class.field_name())
            .collect::<Vec<_>>();
        assert!(!names.contains(&"broken_evidence_links"));
        assert!(!names.contains(&"weak_evidence_references"));
        assert!(!names.contains(&"occurrence_headroom"));
    }

    #[test]
    fn receipt_fields_include_occurrence_headroom_when_present() {
        let summary = Summary::from_outcomes(&[outcome(MatchStatus::Matched, Some("matched"))]);
        let context = ReportContext {
            occurrence_headroom_entries: Some(2),
            ..ReportContext::default()
        };
        let fields = AdvisoryClass::receipt_fields(&summary, context);
        assert!(
            fields
                .iter()
                .any(|(class, count)| *class == AdvisoryClass::OccurrenceHeadroom && *count == 2)
        );
    }
}
