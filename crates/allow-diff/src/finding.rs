use allow_core::{Finding, finding_identity_key as core_finding_identity_key, normalize_path};
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingPostureKind {
    New,
    Removed,
}

impl FindingPostureKind {
    pub const ALL: &[Self] = &[Self::New, Self::Removed];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Removed => "removed",
        }
    }
}

pub fn finding_posture_changes(base: &[Finding], head: &[Finding]) -> Vec<FindingPostureChange> {
    let base_by_key = findings_by_key(base);
    let head_by_key = findings_by_key(head);
    let mut changes = Vec::new();
    for (key, counted) in &head_by_key {
        let base_count = base_by_key
            .get(key)
            .map(|counted| counted.count)
            .unwrap_or(0);
        if counted.count > base_count {
            for _ in 0..(counted.count - base_count) {
                changes.push(finding_posture_change(
                    FindingPostureKind::New,
                    key,
                    counted.finding,
                ));
            }
        }
    }
    for (key, counted) in &base_by_key {
        let head_count = head_by_key
            .get(key)
            .map(|counted| counted.count)
            .unwrap_or(0);
        if counted.count > head_count {
            for _ in 0..(counted.count - head_count) {
                changes.push(finding_posture_change(
                    FindingPostureKind::Removed,
                    key,
                    counted.finding,
                ));
            }
        }
    }
    changes
}

#[derive(Debug, Clone, Copy)]
struct CountedFinding<'a> {
    finding: &'a Finding,
    count: usize,
}

fn findings_by_key(findings: &[Finding]) -> BTreeMap<String, CountedFinding<'_>> {
    let mut by_key = BTreeMap::new();
    for finding in findings {
        by_key
            .entry(finding_identity_key(finding))
            .and_modify(|counted: &mut CountedFinding<'_>| counted.count += 1)
            .or_insert(CountedFinding { finding, count: 1 });
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
    }
}

pub fn finding_identity_key(finding: &Finding) -> String {
    core_finding_identity_key(finding)
}
