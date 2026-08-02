#[cfg(test)]
use allow_core::CargoAllowErrorKind;
use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, Requirements};
use std::collections::BTreeSet;

use crate::evidence_reference::{EvidenceReference, recognized_evidence_prefixes};
use crate::source_tree_scope::validate_path_scope;
use crate::text_validation::{validate_no_surrounding_whitespace, validate_required_text};

/// Hard ceiling for `occurrence_limit` on allow entries.
///
/// Values above this defeat counted no-new semantics (for example a typo of
/// `999999999` or `u32::MAX`) and are rejected at policy load. Real counted
/// baselines in this repository stay in the low single digits; the ceiling is
/// intentionally generous for unusual monorepos while still failing closed on
/// implausible limits.
pub const OCCURRENCE_LIMIT_MAX: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkScopeValidation {
    Strict,
    ReportOnly,
}

pub(crate) fn validate_allow_entry_identity(
    entry: &AllowEntry,
    ids: &mut BTreeSet<String>,
) -> CargoAllowResult<()> {
    validate_allow_id(&entry.id)?;
    if let Some(family) = entry.family.as_deref() {
        validate_required_text(&format!("{} family", entry.id), family)?;
    }
    if !ids.insert(entry.id.clone()) {
        return Err(CargoAllowError::new(format!(
            "duplicate allow id `{}`",
            entry.id
        )));
    }
    Ok(())
}

pub(crate) fn validate_allow_entry_requirements(
    entry: &AllowEntry,
    requirements: &Requirements,
    link_scope_validation: LinkScopeValidation,
) -> CargoAllowResult<()> {
    if !entry.owner.is_empty() {
        validate_no_surrounding_whitespace(&format!("{} owner", entry.id), &entry.owner)?;
    }
    if !entry.classification.is_empty() {
        validate_no_surrounding_whitespace(
            &format!("{} classification", entry.id),
            &entry.classification,
        )?;
        // #2661: "baseline_debt" is the only classification with structural
        // semantics (requires expires + created, caps at 120 days, blocks in
        // Strict/Release). A typo like "baseline-debt" or "BaselineDebt"
        // silently bypasses all of these. Reject near-miss spellings so the
        // typo fails closed instead of creating an uncontrolled immortal
        // entry.
        if entry.classification != "baseline_debt"
            && looks_like_baseline_debt_typo(&entry.classification)
        {
            return Err(CargoAllowError::new(format!(
                "{} classification `{}` looks like a typo of `baseline_debt`; use the exact underscore spelling to get baseline_debt lifecycle enforcement, or pick a different classification",
                entry.id, entry.classification
            )));
        }
    }
    if !entry.reason.is_empty() {
        validate_no_surrounding_whitespace(&format!("{} reason", entry.id), &entry.reason)?;
    }
    if requirements.owner_required && entry.owner.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{} missing owner", entry.id)));
    }
    if requirements.owner_required
        && entry.owner.trim() == "unowned"
        && entry.classification != "baseline_debt"
    {
        return Err(CargoAllowError::new(format!(
            "{} missing concrete owner",
            entry.id
        )));
    }
    if requirements.reason_required && entry.reason.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{} missing reason", entry.id)));
    }
    if requirements.classification_required && entry.classification.trim().is_empty() {
        return Err(CargoAllowError::new(format!(
            "{} missing classification",
            entry.id
        )));
    }
    validate_non_empty_values(&entry.id, "evidence", &entry.evidence)?;
    validate_non_empty_values(&entry.id, "link", &entry.links)?;
    validate_unique_values(&entry.id, "evidence", &entry.evidence)?;
    validate_unique_values(&entry.id, "link", &entry.links)?;
    // Typed evidence references (doc:, spec:, adr:, etc.) must have a
    // non-empty target after the prefix — "doc:" with no path is invalid
    // (#1832).
    for evidence in &entry.evidence {
        if let Some(reference) = EvidenceReference::parse(evidence)
            && reference.kind.is_local_file()
            && reference.value.as_os_str().is_empty()
        {
            return Err(CargoAllowError::new(format!(
                "{} evidence reference `{}` has an empty target",
                entry.id, evidence
            )));
        }
    }
    if link_scope_validation == LinkScopeValidation::Strict {
        validate_local_evidence_scopes(entry)?;
        validate_local_link_scopes(entry)?;
    }
    Ok(())
}

pub(crate) fn validate_allow_entry_evidence_and_limit(
    entry: &AllowEntry,
    requirements: &Requirements,
) -> CargoAllowResult<()> {
    if requirements.unsafe_evidence_required
        && entry.kind == FindingKind::Unsafe
        && entry.evidence.is_empty()
    {
        return Err(CargoAllowError::new(format!(
            "{} unsafe entry missing evidence",
            entry.id
        )));
    }
    if let Some(label) = typed_evidence_required_label(entry)
        && !entry
            .evidence
            .iter()
            .any(|evidence| evidence_is_typed(evidence))
    {
        return Err(CargoAllowError::new(format!(
            "{} {label} entry requires at least one typed evidence reference",
            entry.id,
        )));
    }
    if requirements.evidence_required {
        if entry.evidence.is_empty() {
            return Err(CargoAllowError::new(format!(
                "{} missing evidence",
                entry.id
            )));
        }
        if entry.classification != "baseline_debt"
            && !entry
                .evidence
                .iter()
                .any(|evidence| evidence_is_typed(evidence))
        {
            return Err(CargoAllowError::new(format!(
                "{} evidence_required entries require at least one typed evidence reference",
                entry.id
            )));
        }
    }
    if let Some(limit) = entry.occurrence_limit {
        if limit == 0 {
            return Err(CargoAllowError::new(format!(
                "{} occurrence_limit must be greater than zero",
                entry.id
            )));
        }
        if limit > OCCURRENCE_LIMIT_MAX {
            return Err(CargoAllowError::new(format!(
                "{} occurrence_limit must be at most {OCCURRENCE_LIMIT_MAX}",
                entry.id
            )));
        }
    }
    Ok(())
}

fn typed_evidence_required_label(entry: &AllowEntry) -> Option<String> {
    if entry.classification == "baseline_debt" {
        return None;
    }
    match (entry.kind, entry.family.as_deref()) {
        (FindingKind::Unsafe, _) => Some("unsafe".to_string()),
        (FindingKind::PolicyException, Some("process_spawn" | "network_destination")) => entry
            .family
            .as_deref()
            .map(|family| format!("policy_exception.{family}")),
        _ => None,
    }
}

fn evidence_is_typed(evidence: &str) -> bool {
    let Some((prefix, target)) = evidence.split_once(':') else {
        return false;
    };
    let prefix = prefix.trim();
    let target = target.trim();
    !target.is_empty() && recognized_evidence_prefixes().any(|known| known == prefix)
}

fn validate_allow_id(id: &str) -> CargoAllowResult<()> {
    if id.trim().is_empty() {
        return Err(CargoAllowError::new("allow entry has empty id"));
    }
    if id.trim() != id {
        return Err(CargoAllowError::new(format!(
            "allow id `{id}` must not have leading or trailing whitespace"
        )));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(CargoAllowError::new(format!(
            "allow id `{id}` may contain only ASCII letters, digits, hyphen, or underscore"
        )));
    }
    Ok(())
}

fn validate_non_empty_values(id: &str, label: &str, values: &[String]) -> CargoAllowResult<()> {
    for (index, value) in values.iter().enumerate() {
        validate_required_text(&format!("{id} {label} entry {}", index + 1), value)?;
    }
    Ok(())
}

fn validate_unique_values(id: &str, label: &str, values: &[String]) -> CargoAllowResult<()> {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value.as_str()) {
            return Err(CargoAllowError::new(format!(
                "{} duplicate {} entry `{}` at position {}",
                id,
                label,
                value,
                index + 1
            )));
        }
    }
    Ok(())
}

fn validate_local_evidence_scopes(entry: &AllowEntry) -> CargoAllowResult<()> {
    for (index, evidence) in entry.evidence.iter().enumerate() {
        let Some(reference) = EvidenceReference::parse(evidence) else {
            continue;
        };
        if !reference.kind.is_local_file() {
            continue;
        }
        validate_path_scope(
            &format!("{} evidence entry {}", entry.id, index + 1),
            &reference.value,
        )?;
    }
    Ok(())
}

fn validate_local_link_scopes(entry: &AllowEntry) -> CargoAllowResult<()> {
    for (index, link) in entry.links.iter().enumerate() {
        let Some(reference) = EvidenceReference::parse(link) else {
            continue;
        };
        if !reference.kind.is_local_file() {
            continue;
        }
        validate_path_scope(
            &format!("{} link entry {}", entry.id, index + 1),
            &reference.value,
        )?;
    }
    Ok(())
}

/// Detect near-miss spellings of `baseline_debt` — the only classification
/// with structural lifecycle semantics. Returns true for case variants
/// (`BaselineDebt`, `BASELINE_DEBT`), hyphen variants (`baseline-debt`),
/// whitespace variants (`baseline _debt`), and camelCase variants
/// (`BaselineDebt`). Does NOT match unrelated classifications like
/// `reviewed_exception` or free-form text.
fn looks_like_baseline_debt_typo(classification: &str) -> bool {
    // Exact match with different separators/case.
    let normalized = classification.to_ascii_lowercase().replace(['-', ' '], "_");
    if normalized == "baseline_debt" {
        return true;
    }
    // Fuzzy match: strip all non-alphanumeric characters and compare the
    // letter sequence. Catches camelCase (BaselineDebt → baselinedebt)
    // and other separator variations.
    let letters: String = classification
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    letters == "baselinedebt"
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Lifecycle, Selector};
    use std::path::PathBuf;

    fn entry(id: &str) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "repo-infra".to_string(),
            classification: "reviewed".to_string(),
            reason: "fixture reason".to_string(),
            evidence: vec!["test:panic_path_is_covered".to_string()],
            links: vec!["issue:123".to_string()],
            occurrence_limit: Some(1),
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    fn err_text(result: CargoAllowResult<()>) -> String {
        result.err().map(|err| err.to_string()).unwrap_or_default()
    }

    fn required() -> Requirements {
        Requirements::default()
    }

    #[test]
    fn validate_allow_entry_identity_accepts_unique_id_and_rejects_duplicates() {
        let mut ids = BTreeSet::new();
        let first = entry("allow-1");
        let duplicate = entry("allow-1");

        assert!(validate_allow_entry_identity(&first, &mut ids).is_ok());
        assert!(ids.contains("allow-1"));

        let message = err_text(validate_allow_entry_identity(&duplicate, &mut ids));
        assert!(message.contains("duplicate allow id `allow-1`"));
    }

    #[test]
    fn validate_allow_entry_identity_checks_id_and_family_text() {
        let mut ids = BTreeSet::new();
        let blank_id = entry("");
        let mut padded_id = entry(" allow-1 ");
        let mut invalid_id = entry("allow:1");
        let mut blank_family = entry("allow-blank-family");
        blank_family.family = Some("   ".to_string());

        assert!(
            err_text(validate_allow_entry_identity(&blank_id, &mut ids))
                .contains("allow entry has empty id")
        );
        assert!(
            err_text(validate_allow_entry_identity(&padded_id, &mut ids))
                .contains("must not have leading or trailing whitespace")
        );
        assert!(
            err_text(validate_allow_entry_identity(&invalid_id, &mut ids))
                .contains("may contain only ASCII letters")
        );
        assert!(
            err_text(validate_allow_entry_identity(&blank_family, &mut ids))
                .contains("allow-blank-family family must not be empty")
        );

        padded_id.id = "allow_1-ok".to_string();
        invalid_id.id = "allow-2".to_string();
        assert!(validate_allow_entry_identity(&padded_id, &mut ids).is_ok());
        assert!(validate_allow_entry_identity(&invalid_id, &mut ids).is_ok());
    }

    #[test]
    fn validate_allow_entry_requirements_enforces_required_owner_reason_and_classification() {
        let requirements = required();
        let mut missing_owner = entry("missing-owner");
        let mut unowned_reviewed = entry("unowned-reviewed");
        let mut unowned_baseline = entry("unowned-baseline");
        let mut missing_reason = entry("missing-reason");
        let mut missing_classification = entry("missing-classification");

        missing_owner.owner.clear();
        unowned_reviewed.owner = "unowned".to_string();
        unowned_baseline.owner = "unowned".to_string();
        unowned_baseline.classification = "baseline_debt".to_string();
        missing_reason.reason.clear();
        missing_classification.classification.clear();

        assert!(
            err_text(validate_allow_entry_requirements(
                &missing_owner,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains("missing-owner missing owner")
        );
        assert!(
            err_text(validate_allow_entry_requirements(
                &unowned_reviewed,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains("unowned-reviewed missing concrete owner")
        );
        assert!(
            validate_allow_entry_requirements(
                &unowned_baseline,
                &requirements,
                LinkScopeValidation::Strict
            )
            .is_ok()
        );
        assert!(
            err_text(validate_allow_entry_requirements(
                &missing_reason,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains("missing-reason missing reason")
        );
        assert!(
            err_text(validate_allow_entry_requirements(
                &missing_classification,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains("missing-classification missing classification")
        );
    }

    #[test]
    fn validate_allow_entry_requirements_checks_whitespace_duplicates_and_link_scope_mode() {
        let requirements = required();
        let mut padded = entry("padded-fields");
        let mut duplicate_values = entry("duplicate-values");
        let mut invalid_local_link = entry("invalid-local-link");

        padded.owner = " repo ".to_string();
        assert!(
            err_text(validate_allow_entry_requirements(
                &padded,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains("padded-fields owner must not have leading or trailing whitespace")
        );

        duplicate_values.evidence = vec!["test:a".to_string(), "test:a".to_string()];
        duplicate_values.links = vec!["issue:1".to_string(), "issue:1".to_string()];
        assert!(
            err_text(validate_allow_entry_requirements(
                &duplicate_values,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains("duplicate evidence entry `test:a` at position 2")
        );

        duplicate_values.evidence = vec!["test:a".to_string()];
        assert!(
            err_text(validate_allow_entry_requirements(
                &duplicate_values,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains("duplicate link entry `issue:1` at position 2")
        );

        invalid_local_link.links = vec!["doc:docs/../safety.md".to_string()];
        assert!(
            err_text(validate_allow_entry_requirements(
                &invalid_local_link,
                &requirements,
                LinkScopeValidation::Strict
            ))
            .contains(
                "invalid-local-link link entry 1 path must not contain parent directory segments"
            )
        );
        assert!(
            validate_allow_entry_requirements(
                &invalid_local_link,
                &requirements,
                LinkScopeValidation::ReportOnly
            )
            .is_ok()
        );
    }

    #[test]
    fn validate_classification_rejects_baseline_debt_typo() {
        // #2661: "baseline_debt" is the only classification with structural
        // semantics. Typos must fail closed rather than silently bypassing
        // lifecycle enforcement.
        let requirements = required();
        for typo in [
            "baseline-debt",
            "BaselineDebt",
            "BASELINE_DEBT",
            "baseline _debt",
        ] {
            let mut entry = entry("allow-typo");
            entry.classification = typo.to_string();
            let err = err_text(validate_allow_entry_requirements(
                &entry,
                &requirements,
                LinkScopeValidation::ReportOnly,
            ));
            assert!(
                err.contains("looks like a typo of `baseline_debt`"),
                "classification `{typo}` should be rejected as a typo: {err}"
            );
        }
    }

    #[test]
    fn validate_classification_accepts_exact_baseline_debt_and_unrelated_values() {
        let requirements = required();
        // Exact spelling passes.
        let mut baseline = entry("allow-baseline");
        baseline.classification = "baseline_debt".to_string();
        baseline.owner = "unowned".to_string();
        assert!(
            validate_allow_entry_requirements(
                &baseline,
                &requirements,
                LinkScopeValidation::ReportOnly
            )
            .is_ok(),
            "exact `baseline_debt` should pass"
        );
        // Unrelated classifications pass (the controlled vocabulary is NOT
        // enforced — only near-misses of baseline_debt are caught).
        for classification in ["reviewed_exception", "accepted_risk", "whatever"] {
            let mut entry = entry("allow-other");
            entry.classification = classification.to_string();
            assert!(
                validate_allow_entry_requirements(
                    &entry,
                    &requirements,
                    LinkScopeValidation::ReportOnly
                )
                .is_ok(),
                "classification `{classification}` should pass (not a baseline_debt typo)"
            );
        }
    }

    #[test]
    fn validate_allow_entry_evidence_and_limit_checks_required_evidence_and_limits() {
        let requirements = required();
        let mut unsafe_missing = entry("unsafe-missing");
        let mut unsafe_weak = entry("unsafe-weak");
        let mut unsafe_typed = entry("unsafe-typed");
        let mut evidence_required = entry("missing-general-evidence");
        let mut zero_limit = entry("zero-limit");
        let mut strict_requirements = required();

        unsafe_missing.kind = FindingKind::Unsafe;
        unsafe_missing.evidence.clear();
        unsafe_weak.kind = FindingKind::Unsafe;
        unsafe_weak.evidence = vec!["TODO add proof".to_string()];
        unsafe_typed.kind = FindingKind::Unsafe;
        unsafe_typed.evidence = vec!["test:unsafe_path_is_covered".to_string()];
        evidence_required.evidence.clear();
        zero_limit.occurrence_limit = Some(0);
        strict_requirements.unsafe_evidence_required = false;
        strict_requirements.evidence_required = true;

        assert!(
            err_text(validate_allow_entry_evidence_and_limit(
                &unsafe_missing,
                &requirements
            ))
            .contains("unsafe-missing unsafe entry missing evidence")
        );
        assert!(
            err_text(validate_allow_entry_evidence_and_limit(
                &unsafe_weak,
                &requirements
            ))
            .contains("unsafe-weak unsafe entry requires at least one typed evidence reference")
        );
        assert!(validate_allow_entry_evidence_and_limit(&unsafe_typed, &requirements).is_ok());
        assert!(
            err_text(validate_allow_entry_evidence_and_limit(
                &evidence_required,
                &strict_requirements
            ))
            .contains("missing-general-evidence missing evidence")
        );
        assert!(
            err_text(validate_allow_entry_evidence_and_limit(
                &zero_limit,
                &requirements
            ))
            .contains("zero-limit occurrence_limit must be greater than zero")
        );

        let mut oversized_limit = entry("oversized-limit");
        oversized_limit.occurrence_limit = Some(OCCURRENCE_LIMIT_MAX + 1);
        assert!(
            err_text(validate_allow_entry_evidence_and_limit(
                &oversized_limit,
                &requirements
            ))
            .contains(&format!(
                "oversized-limit occurrence_limit must be at most {OCCURRENCE_LIMIT_MAX}"
            ))
        );

        let mut ceiling_limit = entry("ceiling-limit");
        ceiling_limit.occurrence_limit = Some(OCCURRENCE_LIMIT_MAX);
        assert!(validate_allow_entry_evidence_and_limit(&ceiling_limit, &requirements).is_ok());
    }

    #[test]
    fn typed_evidence_required_label_identifies_sensitive_entry_types() {
        let mut baseline = entry("baseline");
        let mut unsafe_entry = entry("unsafe-entry");
        let mut process_entry = entry("process-entry");
        let mut network_entry = entry("network-entry");
        let normal = entry("normal");

        baseline.kind = FindingKind::Unsafe;
        baseline.classification = "baseline_debt".to_string();
        unsafe_entry.kind = FindingKind::Unsafe;
        process_entry.kind = FindingKind::PolicyException;
        process_entry.family = Some("process_spawn".to_string());
        network_entry.kind = FindingKind::PolicyException;
        network_entry.family = Some("network_destination".to_string());

        assert_eq!(typed_evidence_required_label(&baseline), None);
        assert_eq!(
            typed_evidence_required_label(&unsafe_entry).as_deref(),
            Some("unsafe")
        );
        assert_eq!(
            typed_evidence_required_label(&process_entry).as_deref(),
            Some("policy_exception.process_spawn")
        );
        assert_eq!(
            typed_evidence_required_label(&network_entry).as_deref(),
            Some("policy_exception.network_destination")
        );
        assert_eq!(typed_evidence_required_label(&normal), None);
    }

    #[test]
    fn evidence_is_typed_requires_recognized_prefix_and_non_empty_target() {
        assert!(evidence_is_typed("test:panic_path_is_covered"));
        assert!(evidence_is_typed(" doc : docs/safety.md "));
        assert!(!evidence_is_typed("test:"));
        assert!(!evidence_is_typed("unknown:target"));
        assert!(!evidence_is_typed("manual review note"));
    }

    #[test]
    fn low_level_value_validators_report_positioned_errors() {
        assert!(validate_allow_id("allow_test-1").is_ok());
        assert!(err_text(validate_allow_id("")).contains("allow entry has empty id"));
        assert!(
            err_text(validate_allow_id(" allow-1 "))
                .contains("must not have leading or trailing whitespace")
        );
        assert!(err_text(validate_allow_id("allow:1")).contains("may contain only ASCII letters"));

        assert!(
            err_text(validate_non_empty_values(
                "allow-1",
                "evidence",
                &["".to_string()]
            ))
            .contains("allow-1 evidence entry 1 must not be empty")
        );
        assert!(
            err_text(validate_unique_values(
                "allow-1",
                "link",
                &["issue:1".to_string(), "issue:1".to_string()]
            ))
            .contains("allow-1 duplicate link entry `issue:1` at position 2")
        );
    }

    #[test]
    fn direct_error_discriminators_match_entry_validation_messages() {
        let requirements = required();
        let mut ids = BTreeSet::new();
        let first = entry("duplicate-id");
        let duplicate = entry("duplicate-id");
        assert!(validate_allow_entry_identity(&first, &mut ids).is_ok());
        let err = validate_allow_entry_identity(&duplicate, &mut ids)
            .expect_err("duplicate allow ids should fail identity validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut missing_owner = entry("missing-owner");
        missing_owner.owner.clear();
        let err = validate_allow_entry_requirements(
            &missing_owner,
            &requirements,
            LinkScopeValidation::Strict,
        )
        .expect_err("missing owner should fail requirements validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut unowned_reviewed = entry("unowned-reviewed");
        unowned_reviewed.owner = "unowned".to_string();
        let err = validate_allow_entry_requirements(
            &unowned_reviewed,
            &requirements,
            LinkScopeValidation::Strict,
        )
        .expect_err("unowned non-baseline owner should fail requirements validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut missing_reason = entry("missing-reason");
        missing_reason.reason.clear();
        let err = validate_allow_entry_requirements(
            &missing_reason,
            &requirements,
            LinkScopeValidation::Strict,
        )
        .expect_err("missing reason should fail requirements validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut missing_classification = entry("missing-classification");
        missing_classification.classification.clear();
        let err = validate_allow_entry_requirements(
            &missing_classification,
            &requirements,
            LinkScopeValidation::Strict,
        )
        .expect_err("missing classification should fail requirements validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut unsafe_missing = entry("unsafe-missing");
        unsafe_missing.kind = FindingKind::Unsafe;
        unsafe_missing.evidence.clear();
        let err = validate_allow_entry_evidence_and_limit(&unsafe_missing, &requirements)
            .expect_err("unsafe entry without evidence should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut unsafe_weak = entry("unsafe-weak");
        unsafe_weak.kind = FindingKind::Unsafe;
        unsafe_weak.evidence = vec!["TODO add proof".to_string()];
        let err = validate_allow_entry_evidence_and_limit(&unsafe_weak, &requirements)
            .expect_err("unsafe entry with weak evidence should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut evidence_required = entry("missing-general-evidence");
        let mut strict_requirements = required();
        strict_requirements.unsafe_evidence_required = false;
        strict_requirements.evidence_required = true;
        evidence_required.evidence.clear();
        let err = validate_allow_entry_evidence_and_limit(&evidence_required, &strict_requirements)
            .expect_err("evidence-required entry without evidence should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut zero_limit = entry("zero-limit");
        zero_limit.occurrence_limit = Some(0);
        let err = validate_allow_entry_evidence_and_limit(&zero_limit, &requirements)
            .expect_err("zero occurrence_limit should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let mut oversized_limit = entry("oversized-limit");
        oversized_limit.occurrence_limit = Some(OCCURRENCE_LIMIT_MAX + 1);
        let err = validate_allow_entry_evidence_and_limit(&oversized_limit, &requirements)
            .expect_err("oversized occurrence_limit should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let err = validate_allow_id("").expect_err("empty allow id should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let padded_id = " allow-1 ";
        let err = validate_allow_id(padded_id).expect_err("padded allow id should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

        let err = validate_unique_values(
            "allow-1",
            "link",
            &["issue:1".to_string(), "issue:1".to_string()],
        )
        .expect_err("duplicate link values should fail validation");
        assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
    }

    #[test]
    fn validate_local_link_scopes_only_checks_local_file_links() {
        let mut entry = entry("links");
        entry.links = vec![
            "issue:123".to_string(),
            "not typed".to_string(),
            "doc:docs/safety.md".to_string(),
        ];
        assert!(validate_local_link_scopes(&entry).is_ok());

        entry.links = vec!["issue:123".to_string(), "doc:/absolute.md".to_string()];
        assert!(
            err_text(validate_local_link_scopes(&entry))
                .contains("links link entry 2 path must be source-tree-relative")
        );
    }
}
