//! Cross-cutting check for the bare-allow selector conflict (#2057).
//!
//! When `requirements.allow_bare_allow_attributes = false`, any `lint_exception`
//! entry that receipts a bare `#[allow(...)]` attribute (entry family
//! `allow_attribute`) is a configuration conflict: `check` will mark every such
//! match `invalid_selector`, while `doctor`/`audit` previously reported the
//! same policy as valid/passing. This validator surfaces the conflict at policy
//! validation time so all three commands agree, and includes the next safe
//! action.

use allow_core::{AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, FindingKind};

/// `true` when `entry` receipts a bare `#[allow(...)]` attribute occurrence —
/// i.e. it is a `lint_exception` scoped to the `allow_attribute` family. This
/// is the static proxy for the runtime `finding.family == "allow_attribute"`
/// test in `allow_match::classification`.
pub(crate) fn entry_receipts_bare_allow(entry: &AllowEntry) -> bool {
    entry.kind == FindingKind::LintException && entry.family.as_deref() == Some("allow_attribute")
}

/// Why `requirements` forbid persisting `entry`, or `None` when the entry is
/// legal to write.
///
/// Generators (`propose`) must consult this before adding a candidate entry.
/// Producing an entry the same policy rejects turns a preview into a hard
/// validation error naming an id the operator cannot find in their file, which
/// is what made `init` followed by `propose` unusable on any tree containing a
/// bare `#[allow(...)]` (#3023).
pub fn generated_entry_rejection(
    requirements: &allow_core::Requirements,
    entry: &AllowEntry,
) -> Option<&'static str> {
    if !requirements.allow_bare_allow_attributes && entry_receipts_bare_allow(entry) {
        return Some(
            "requirements.allow_bare_allow_attributes = false forbids receipting bare #[allow(...)] attributes",
        );
    }
    None
}

/// Detect the bare-allow configuration conflict and return a single,
/// actionable diagnostic listing every offending entry id and the next safe
/// action. Returns `Ok(())` when the config is internally consistent.
pub(crate) fn detect_bare_allow_conflict(cfg: &AllowConfig) -> CargoAllowResult<()> {
    if cfg.requirements.allow_bare_allow_attributes {
        return Ok(());
    }
    let offenders: Vec<&str> = cfg
        .allow
        .iter()
        .filter(|entry| entry_receipts_bare_allow(entry))
        .map(|entry| entry.id.as_str())
        .collect();
    if offenders.is_empty() {
        return Ok(());
    }
    let listed = offenders
        .iter()
        .map(|id| format!("  - {id}"))
        .collect::<Vec<_>>()
        .join("\n");
    Err(CargoAllowError::new(format!(
        "configuration conflict: {} lint_exception entry/entries receipt bare #[allow(...)] \
         attributes while requirements.allow_bare_allow_attributes = false:\n{listed}\n\
         Next safe action: set requirements.allow_bare_allow_attributes = true if this repository \
         intentionally receipts bare #[allow(...)] occurrences, or remove/re-scope the listed \
         entries to non-bare selectors.",
        offenders.len()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Lifecycle, Selector};

    fn bare_allow_entry(id: &str) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::LintException,
            family: Some("allow_attribute".to_string()),
            path: None,
            glob: Some("src/lib.rs".to_string()),
            owner: "core".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "bare allow".to_string(),
            evidence: vec!["test:fixture".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector {
                ast_kind: Some("attribute".to_string()),
                lint: Some("clippy::expect_used".to_string()),
                glob: Some("src/lib.rs".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn config_with(entries: Vec<AllowEntry>, allow_bare: bool) -> AllowConfig {
        let mut cfg = AllowConfig::empty();
        cfg.requirements.allow_bare_allow_attributes = allow_bare;
        cfg.allow = entries;
        cfg
    }

    #[test]
    fn generated_entry_rejection_matches_the_conflict_validator() {
        let forbidding = config_with(Vec::new(), false).requirements;
        let permitting = config_with(Vec::new(), true).requirements;
        let bare = bare_allow_entry("allow-0001");

        // A generator asking "may I write this?" must get the same answer the
        // validator would give after the fact, or `propose` emits a policy it
        // then rejects (#3023).
        let rejection = generated_entry_rejection(&forbidding, &bare);
        assert!(
            rejection.is_some_and(|reason| reason.contains("allow_bare_allow_attributes")),
            "a bare-allow entry must be refused while the requirement is false: {rejection:?}"
        );
        assert!(detect_bare_allow_conflict(&config_with(vec![bare.clone()], false)).is_err());

        assert!(generated_entry_rejection(&permitting, &bare).is_none());
        assert!(detect_bare_allow_conflict(&config_with(vec![bare], true)).is_ok());
    }

    #[test]
    fn generated_entry_rejection_permits_unrelated_entries() {
        let forbidding = config_with(Vec::new(), false).requirements;
        let mut unsafe_entry = bare_allow_entry("allow-0002");
        unsafe_entry.kind = FindingKind::Unsafe;
        unsafe_entry.family = Some("unsafe_block".to_string());

        assert!(generated_entry_rejection(&forbidding, &unsafe_entry).is_none());
    }

    #[test]
    fn no_conflict_when_bare_allows_allowed() {
        let cfg = config_with(vec![bare_allow_entry("allow-bare")], true);
        assert!(detect_bare_allow_conflict(&cfg).is_ok());
    }

    #[test]
    fn no_conflict_when_no_bare_allow_entries() {
        let cfg = config_with(Vec::new(), false);
        assert!(detect_bare_allow_conflict(&cfg).is_ok());
    }

    #[test]
    fn conflict_lists_offending_ids_and_next_safe_action() {
        let cfg = config_with(
            vec![
                bare_allow_entry("allow-0001"),
                bare_allow_entry("allow-0002"),
            ],
            false,
        );
        let err = detect_bare_allow_conflict(&cfg).expect_err("conflict should be detected");
        let msg = err.to_string();
        assert!(msg.contains("configuration conflict"), "{msg}");
        assert!(msg.contains("allow-0001"), "{msg}");
        assert!(msg.contains("allow-0002"), "{msg}");
        assert!(
            msg.contains("allow_bare_allow_attributes = true"),
            "should state the next safe action: {msg}"
        );
    }

    #[test]
    fn ignores_non_allow_attribute_lint_exceptions() {
        let mut other = bare_allow_entry("allow-other");
        other.family = Some("clippy".to_string());
        let cfg = config_with(vec![other], false);
        assert!(
            detect_bare_allow_conflict(&cfg).is_ok(),
            "non-allow_attribute lint exceptions are not bare-allow conflicts"
        );
    }
}
