//! The receipt the source-exception ledger carries for itself (#3032).
//!
//! `inventory = git-tracked` includes the policy file once it is committed, so
//! the ledger shows up in its own `non_rust_file` inventory. Without a receipt
//! the first CI run after adoption fails on `policy/allow.toml` itself, for a
//! reason unrelated to the adopter's code.
//!
//! cargo-allow's own ledger has carried this entry since `allow-0077`. This
//! module is that entry, generated for whatever path the operator asked
//! cargo-allow to write, so `init` and `propose --write` produce a ledger that
//! is honest about its own presence in the source tree.

use allow_core::{AllowEntry, FindingKind, LastSeen, Lifecycle, Selector, SimpleDate};
use std::path::PathBuf;

/// The ledger is durable governance, not debt, so its receipt carries a review
/// horizon rather than an expiry. A year keeps it off the routine worklist
/// while still forcing an eventual look.
pub const LEDGER_SELF_RECEIPT_REVIEW_DAYS: i64 = 365;

/// Classification for the ledger's own receipt. Deliberately not
/// `baseline_debt`: the ledger is not retained debt queued for removal, and an
/// expiring receipt on the policy file would put every adopter on a renewal
/// treadmill for their own governance record.
pub const LEDGER_SELF_RECEIPT_CLASSIFICATION: &str = "source_exception_policy";

/// `true` when `entry` already receipts the ledger at `policy_rel_path`, so
/// callers never write a duplicate over an operator's own entry.
pub fn receipts_ledger_at(entry: &AllowEntry, policy_rel_path: &str) -> bool {
    let normalized = normalize(policy_rel_path);
    entry.kind == FindingKind::NonRustFile
        && (entry
            .path
            .as_deref()
            .is_some_and(|path| normalize(&path.to_string_lossy()) == normalized)
            || entry
                .glob
                .as_deref()
                .is_some_and(|glob| normalize(glob) == normalized))
}

/// Build the ledger's self-receipt for a policy file at `policy_rel_path`
/// (repo-relative, forward-slashed), owned by `owner`.
pub fn ledger_self_receipt(id: &str, policy_rel_path: &str, owner: &str) -> AllowEntry {
    let path = normalize(policy_rel_path);
    let today = SimpleDate::today_utc_approx();
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::NonRustFile,
        family: Some("configuration".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: owner.to_string(),
        classification: LEDGER_SELF_RECEIPT_CLASSIFICATION.to_string(),
        reason:
            "Canonical source-exception ledger for this repository's own source-tree policy receipts."
                .to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some(today.to_string()),
            review_after: Some(today.add_days(LEDGER_SELF_RECEIPT_REVIEW_DAYS).to_string()),
            expires: None,
        },
        selector: Selector {
            // A path/glob alone is not structural identity, so the selector
            // must carry the tracked-file kind and the file's fingerprint.
            ast_kind: Some("tracked_file".to_string()),
            target_fingerprint: file_fingerprint(&path),
            glob: Some(path.clone()),
            ..Selector::default()
        },
        // Tracked-file findings are reported at the head of the file.
        last_seen: Some(LastSeen { line: 1, column: 1 }),
    }
}

/// Canonical repo-relative shape for a receipt path: forward slashes, no `.`
/// segments, no empty segments.
///
/// `--config policy/./allow.toml` writes to `policy/allow.toml`, and entry
/// validation rejects a stored path containing current-directory segments — so
/// carrying the operator's spelling through would emit a ledger that no
/// cargo-allow command can parse. Normalizing here keeps every caller honest
/// rather than relying on each one to pre-canonicalize.
fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn file_fingerprint(path: &str) -> Option<String> {
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_receipt_is_durable_not_debt() {
        let entry = ledger_self_receipt("allow-0001", "policy/allow.toml", "core/policy");

        assert_eq!(entry.classification, LEDGER_SELF_RECEIPT_CLASSIFICATION);
        assert_ne!(entry.classification, "baseline_debt");
        // A review horizon, never an expiry: an expiring receipt on the ledger
        // would turn the gate red on a timer (#3032).
        assert!(entry.lifecycle.expires.is_none());
        assert!(entry.lifecycle.review_after.is_some());
    }

    #[test]
    fn self_receipt_selector_carries_structural_identity() {
        let entry = ledger_self_receipt("allow-0001", "policy/allow.toml", "core/policy");

        // Path/glob scope alone is rejected by entry validation.
        assert!(entry.selector.has_structural_identity());
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("tracked_file"));
        assert_eq!(entry.selector.target_fingerprint.as_deref(), Some("toml"));
        assert_eq!(entry.selector.glob.as_deref(), Some("policy/allow.toml"));
    }

    #[test]
    fn self_receipt_normalizes_the_written_path() {
        let entry = ledger_self_receipt("allow-0001", ".\\config\\allow.toml", "core/policy");

        assert_eq!(entry.selector.glob.as_deref(), Some("config/allow.toml"));
        assert_eq!(
            entry.path.as_deref(),
            Some(std::path::Path::new("config/allow.toml"))
        );
    }

    #[test]
    fn self_receipt_drops_current_directory_segments() {
        // Entry validation rejects a stored path containing `.` segments, so
        // carrying the operator's spelling through would emit a ledger no
        // cargo-allow command can parse.
        for spelling in [
            "policy/./allow.toml",
            "./policy/allow.toml",
            "policy//allow.toml",
            ".\\policy\\.\\allow.toml",
        ] {
            let entry = ledger_self_receipt("allow-0001", spelling, "core/policy");
            assert_eq!(
                entry.selector.glob.as_deref(),
                Some("policy/allow.toml"),
                "{spelling} should normalize to the canonical repo-relative path"
            );
            assert!(
                !entry
                    .path
                    .as_deref()
                    .is_some_and(|path| path.to_string_lossy().contains("/./")),
                "{spelling} left a current-directory segment in the stored path"
            );
        }
    }

    #[test]
    fn receipts_ledger_at_matches_path_or_glob_and_ignores_others() {
        let by_path = ledger_self_receipt("allow-0001", "policy/allow.toml", "core/policy");
        assert!(receipts_ledger_at(&by_path, "policy/allow.toml"));
        assert!(receipts_ledger_at(&by_path, "./policy/allow.toml"));
        assert!(!receipts_ledger_at(&by_path, "policy/other.toml"));

        let mut by_glob = by_path.clone();
        by_glob.path = None;
        by_glob.glob = Some("policy/allow.toml".to_string());
        assert!(receipts_ledger_at(&by_glob, "policy/allow.toml"));

        // An unrelated non-Rust receipt must not be mistaken for the ledger's.
        let mut other = by_path.clone();
        other.path = Some(PathBuf::from("Cargo.toml"));
        other.glob = None;
        assert!(!receipts_ledger_at(&other, "policy/allow.toml"));
    }
}
