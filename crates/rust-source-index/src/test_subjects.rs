//! Structural Rust test-subject DTOs (#2587-B).
//!
//! Discovery and selector resolution remain in `allow-rust` until #2587-C.

use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustTestTargetKind {
    Library,
    Binary,
    IntegrationTest,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RustTestTargetIdentity {
    pub kind: RustTestTargetKind,
    pub name: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RustTestSelector {
    pub package: String,
    pub target: RustTestTargetIdentity,
    pub module_path: Vec<String>,
    pub function: String,
}

impl RustTestSelector {
    pub fn display_name(&self) -> String {
        let mut parts = self.module_path.clone();
        parts.push(self.function.clone());
        format!(
            "{}:{}:{}::{}",
            self.package,
            target_kind_name(self.target.kind),
            self.target.name,
            parts.join("::")
        )
    }

    pub fn validate(&self) -> bool {
        !self.package.trim().is_empty()
            && !self.target.name.trim().is_empty()
            && !self.function.trim().is_empty()
            && self
                .module_path
                .iter()
                .all(|segment| !segment.trim().is_empty())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestSourceRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestSubject {
    pub selector: RustTestSelector,
    pub source_path: String,
    pub source_range: RustTestSourceRange,
    pub body_identity: String,
    pub attributes: Vec<String>,
    pub generated_or_parameterized: bool,
    pub cfg_or_feature_unknown: bool,
    pub ignored: bool,
    pub limitations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustTestInventoryStatus {
    Complete,
    Partial,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustTestInventoryDiagnosticKind {
    ManifestReadFailed,
    ManifestMalformed,
    SourceReadFailed,
    SourceParseFailed,
    TargetUnresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestInventoryDiagnostic {
    pub kind: RustTestInventoryDiagnosticKind,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestInventory {
    pub subjects: Vec<RustTestSubject>,
    pub status: RustTestInventoryStatus,
    pub diagnostics: Vec<RustTestInventoryDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustTestResolution {
    ResolvedExact(RustTestSubject),
    Ambiguous(Vec<RustTestSelector>),
    NotFound,
    Ignored(RustTestSubject),
    GeneratedOrParameterized(RustTestSubject),
    CfgOrFeatureUnknown(RustTestSubject),
    PartialInventory,
    MalformedSelector,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RustTestInventoryOptions {
    pub additional_test_attributes: BTreeSet<String>,
}

fn target_kind_name(kind: RustTestTargetKind) -> &'static str {
    match kind {
        RustTestTargetKind::Library => "lib",
        RustTestTargetKind::Binary => "bin",
        RustTestTargetKind::IntegrationTest => "test",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_display_name_includes_target_kind() {
        let selector = RustTestSelector {
            package: "demo".into(),
            target: RustTestTargetIdentity {
                kind: RustTestTargetKind::IntegrationTest,
                name: "alpha".into(),
            },
            module_path: vec![],
            function: "roundtrip".into(),
        };
        assert_eq!(selector.display_name(), "demo:test:alpha::roundtrip");
    }

    #[test]
    fn selector_validate_rejects_empty_function() {
        let selector = RustTestSelector {
            package: "demo".into(),
            target: RustTestTargetIdentity {
                kind: RustTestTargetKind::Library,
                name: "demo".into(),
            },
            module_path: vec![],
            function: "   ".into(),
        };
        assert!(!selector.validate());
    }
}
