use allow_core::{
    Finding, PresenceMovement, StructuralIdentity,
    finding_identity_key as core_finding_identity_key, normalize_path,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingPostureChange {
    pub kind: FindingPostureKind,
    pub key: String,
    pub finding_kind: String,
    pub family: Option<String>,
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub source_package: Option<String>,
    pub identity: StructuralIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingPostureKind {
    New,
    Removed,
}

impl FindingPostureKind {
    pub const ALL: &[Self] = &[Self::New, Self::Removed];

    pub const fn presence_movement(self) -> PresenceMovement {
        match self {
            Self::New => PresenceMovement::Introduced,
            Self::Removed => PresenceMovement::Removed,
        }
    }

    pub fn as_str(self) -> &'static str {
        self.presence_movement().finding_change_label()
    }
}

pub fn finding_posture_changes(base: &[Finding], head: &[Finding]) -> Vec<FindingPostureChange> {
    let base_by_key = findings_by_key(base);
    let head_by_key = findings_by_key(head);
    let mut changes = Vec::new();
    for (key, counted) in &head_by_key {
        let base_count = base_by_key
            .get(key)
            .map(|counted| counted.count())
            .unwrap_or(0);
        if counted.count() > base_count {
            for _ in 0..(counted.count() - base_count) {
                changes.push(finding_posture_change(
                    FindingPostureKind::New,
                    key,
                    counted.finding(),
                ));
            }
        }
    }
    for (key, counted) in &base_by_key {
        let head_count = head_by_key
            .get(key)
            .map(|counted| counted.count())
            .unwrap_or(0);
        if counted.count() > head_count {
            for _ in 0..(counted.count() - head_count) {
                changes.push(finding_posture_change(
                    FindingPostureKind::Removed,
                    key,
                    counted.finding(),
                ));
            }
        }
    }
    changes
}

#[derive(Debug, Clone)]
struct CountedFinding<'a> {
    findings: Vec<&'a Finding>,
}

impl<'a> CountedFinding<'a> {
    fn finding(&self) -> &'a Finding {
        // Representative finding for message construction (the first).
        self.findings[0]
    }

    fn count(&self) -> usize {
        self.findings.len()
    }
}

fn findings_by_key(findings: &[Finding]) -> BTreeMap<String, CountedFinding<'_>> {
    let mut by_key: BTreeMap<String, CountedFinding<'_>> = BTreeMap::new();
    for finding in findings {
        by_key
            .entry(finding_identity_key(finding))
            .and_modify(|counted: &mut CountedFinding<'_>| counted.findings.push(finding))
            .or_insert(CountedFinding {
                findings: vec![finding],
            });
    }
    by_key
}

fn finding_posture_change(
    kind: FindingPostureKind,
    key: &str,
    finding: &Finding,
) -> FindingPostureChange {
    FindingPostureChange {
        kind,
        key: key.to_string(),
        finding_kind: finding.kind.as_str().to_string(),
        family: finding.family.clone(),
        path: normalize_path(&finding.path),
        line: finding.span.as_ref().map(|span| span.line),
        column: finding.span.as_ref().map(|span| span.column),
        source_package: finding.source_package_name().map(ToOwned::to_owned),
        identity: finding.identity.clone(),
    }
}

pub fn finding_identity_key(finding: &Finding) -> String {
    core_finding_identity_key(finding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span};
    use std::path::PathBuf;

    #[test]
    fn findings_by_key_call_presence_observer() {
        let first = finding("src/lib.rs", 10, "load");
        let same_identity = finding("src/lib.rs", 99, "load");
        let different = finding("src/other.rs", 10, "store");

        let findings = vec![first.clone(), same_identity, different.clone()];
        let by_key = findings_by_key(&findings);
        let first_key = finding_identity_key(&first);
        let different_key = finding_identity_key(&different);

        assert_eq!(by_key.len(), 2);
        let counted_first = by_key
            .get(&first_key)
            .unwrap_or_else(|| std::panic::panic_any("expected counted first finding"));
        assert_eq!(counted_first.count(), 2);
        assert_eq!(counted_first.finding().path, PathBuf::from("src/lib.rs"));

        let counted_different = by_key
            .get(&different_key)
            .unwrap_or_else(|| std::panic::panic_any("expected counted different finding"));
        assert_eq!(counted_different.count(), 1);
        assert_eq!(
            counted_different.finding().path,
            PathBuf::from("src/other.rs")
        );
    }

    #[test]
    fn findings_by_key_field_discriminator() {
        let finding = finding("src/lib.rs", 10, "load");
        let findings = vec![finding.clone()];
        let by_key = findings_by_key(&findings);
        let counted = by_key
            .get(&finding_identity_key(&finding))
            .unwrap_or_else(|| std::panic::panic_any("expected counted finding"));

        assert_eq!(counted.count(), 1);
        assert_eq!(
            counted.finding().identity.container.as_deref(),
            Some("load")
        );
    }

    #[test]
    fn findings_by_key_return_value_discriminator() {
        assert!(findings_by_key(&[]).is_empty());
        let findings = vec![finding("src/lib.rs", 10, "load")];
        assert!(
            findings_by_key(&findings).contains_key(&finding_identity_key(&finding(
                "src/lib.rs",
                88,
                "load"
            )))
        );
    }

    #[test]
    fn finding_posture_change_call_presence_observer() {
        let mut finding = finding(r"crates\allow-core\src\lib.rs", 42, "load");
        finding.identity.crate_name = Some("allow-core".to_string());
        finding.family = Some("unsafe_fn".to_string());

        let key = finding_identity_key(&finding);
        let change = finding_posture_change(FindingPostureKind::New, &key, &finding);

        assert_eq!(change.kind, FindingPostureKind::New);
        assert_eq!(change.key, key);
        assert_eq!(change.finding_kind, FindingKind::Unsafe.as_str());
        assert_eq!(change.family.as_deref(), Some("unsafe_fn"));
        assert_eq!(change.path, "crates/allow-core/src/lib.rs");
        assert_eq!(change.line, Some(42));
        assert_eq!(change.column, Some(1));
        assert_eq!(change.source_package.as_deref(), Some("allow-core"));
        assert_eq!(change.identity, finding.identity);
    }

    #[test]
    fn finding_identity_key_call_presence_observer() {
        let left = finding("src/lib.rs", 10, "load");
        let moved = finding("src/lib.rs", 99, "load");
        let different = finding("src/lib.rs", 10, "store");

        assert_eq!(finding_identity_key(&left), finding_identity_key(&moved));
        assert_ne!(
            finding_identity_key(&left),
            finding_identity_key(&different)
        );
    }

    fn finding(path: &str, line: u32, container: &str) -> Finding {
        let mut identity = StructuralIdentity::new("rust", "unsafe_fn");
        identity.container = Some(container.to_string());
        identity.normalized_snippet_hash = Some(format!("fnv1a64:{container}"));
        Finding {
            kind: FindingKind::Unsafe,
            family: Some("unsafe_fn".to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line, column: 1 }),
            identity,
            message: "test finding".to_string(),
            ledger: None,
        }
    }
}
