use allow_core::{Finding, FindingKind};

use crate::finding_builder::push_finding;
use crate::line_context::LineContext;
use crate::syntax_kinds::{SafetyCommentAssociation, UnsafeAttribute, UnsafeSyntaxConstruct, UnsafeSyntaxKind};

pub(crate) fn scan_unsafe_constructs(
    context: UnsafeLineContext<'_>,
    unsafe_constructs: &[UnsafeSyntaxConstruct],
    unsafe_attributes: &[UnsafeAttribute],
    findings: &mut Vec<Finding>,
) {
    for unsafe_construct in unsafe_constructs {
        push_finding(
            context.line.site(unsafe_construct.column),
            FindingKind::Unsafe,
            unsafe_construct.kind.family(),
            unsafe_construct.kind.ast_kind(),
            |id| {
                id.symbol = unsafe_construct.symbol.clone();
                if id.container.is_none()
                    && matches!(
                        unsafe_construct.kind,
                        UnsafeSyntaxKind::Impl | UnsafeSyntaxKind::Trait
                    )
                {
                    id.container = unsafe_construct.symbol.clone();
                }
                apply_safety_comment_fingerprint(
                    id,
                    context.safety_comment_association(unsafe_construct.column),
                );
            },
            findings,
        );
    }
    for attribute in unsafe_attributes {
        push_finding(
            context.line.site(attribute.column),
            FindingKind::Unsafe,
            "unsafe_attr",
            "unsafe_attr",
            |id| {
                id.symbol = attribute.symbol.clone();
                apply_safety_comment_fingerprint(
                    id,
                    context.safety_comment_association(attribute.column),
                );
            },
            findings,
        );
    }
}

fn apply_safety_comment_fingerprint(
    id: &mut allow_core::StructuralIdentity,
    association: Option<SafetyCommentAssociation>,
) {
    match association {
        Some(SafetyCommentAssociation::Attached) => {
            id.target_fingerprint = Some("safety-comment:present".to_string());
        }
        Some(SafetyCommentAssociation::NearbyAmbiguous) => {
            id.target_fingerprint = Some("safety-comment:nearby-ambiguous".to_string());
        }
        None => {}
    }
}

pub(crate) struct UnsafeLineContext<'a> {
    pub(crate) line: LineContext<'a>,
    pub(crate) line_no: u32,
    pub(crate) safety_comment_associations: &'a std::collections::BTreeMap<
        (u32, u32),
        SafetyCommentAssociation,
    >,
}

impl UnsafeLineContext<'_> {
    fn safety_comment_association(&self, column: u32) -> Option<SafetyCommentAssociation> {
        self.safety_comment_associations
            .get(&(self.line_no, column))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn scan_unsafe_constructs_projects_construct_families_and_identity() {
        let container = Some("read".to_string());
        let modules = vec!["ffi".to_string()];
        let associations = associations_with(Some(SafetyCommentAssociation::Attached));
        let context = unsafe_context(&container, &modules, &associations);
        let constructs = [
            unsafe_construct(9, UnsafeSyntaxKind::Fn, Some("read")),
            unsafe_construct(17, UnsafeSyntaxKind::Block, Some("unsafe block")),
        ];
        let mut findings = Vec::new();

        scan_unsafe_constructs(context, &constructs, &[], &mut findings);

        assert_eq!(findings.len(), 2);
        let unsafe_fn = finding_with_family(&findings, "unsafe_fn");
        assert_eq!(unsafe_fn.kind, FindingKind::Unsafe);
        assert_eq!(unsafe_fn.path, Path::new("src/lib.rs"));
        assert_eq!(unsafe_fn.identity.ast_kind, "unsafe_fn");
        assert_eq!(unsafe_fn.identity.symbol.as_deref(), Some("read"));
        assert_eq!(unsafe_fn.identity.container.as_deref(), Some("ffi::read"));
        assert_eq!(unsafe_fn.identity.module.as_deref(), Some("ffi"));
        assert_eq!(unsafe_fn.identity.line_hint, Some(42));
        assert_eq!(unsafe_fn.identity.column_hint, Some(9));
        assert_eq!(
            unsafe_fn.identity.target_fingerprint.as_deref(),
            Some("safety-comment:present")
        );

        let unsafe_block = finding_with_family(&findings, "unsafe_block");
        assert_eq!(unsafe_block.identity.ast_kind, "unsafe_block");
        assert_eq!(
            unsafe_block.identity.symbol.as_deref(),
            Some("unsafe block")
        );
        assert_eq!(unsafe_block.identity.column_hint, Some(17));
        assert_eq!(
            unsafe_block.identity.target_fingerprint.as_deref(),
            Some("safety-comment:present")
        );
    }

    #[test]
    fn scan_unsafe_constructs_uses_impl_and_trait_symbols_as_container_fallbacks() {
        let container = None;
        let modules = Vec::new();
        let associations = associations_with(None);
        let context = unsafe_context(&container, &modules, &associations);
        let constructs = [
            unsafe_construct(5, UnsafeSyntaxKind::Impl, Some("<Handle as Send>")),
            unsafe_construct(11, UnsafeSyntaxKind::Trait, Some("Marker")),
        ];
        let mut findings = Vec::new();

        scan_unsafe_constructs(context, &constructs, &[], &mut findings);

        let unsafe_impl = finding_with_family(&findings, "unsafe_impl");
        assert_eq!(
            unsafe_impl.identity.container.as_deref(),
            Some("<Handle as Send>")
        );
        assert_eq!(
            unsafe_impl.identity.symbol.as_deref(),
            Some("<Handle as Send>")
        );
        assert_eq!(unsafe_impl.identity.target_fingerprint, None);

        let unsafe_trait = finding_with_family(&findings, "unsafe_trait");
        assert_eq!(unsafe_trait.identity.container.as_deref(), Some("Marker"));
        assert_eq!(unsafe_trait.identity.symbol.as_deref(), Some("Marker"));
        assert_eq!(unsafe_trait.identity.target_fingerprint, None);
    }

    #[test]
    fn scan_unsafe_constructs_projects_unsafe_attributes_and_empty_inputs() {
        let container = Some("export".to_string());
        let modules = vec!["ffi".to_string(), "symbols".to_string()];
        let associations = associations_with(Some(SafetyCommentAssociation::Attached));
        let context = unsafe_context(&container, &modules, &associations);
        let attributes = [
            UnsafeAttribute {
                column: 3,
                start_byte: 0,
                symbol: Some("no_mangle".to_string()),
            },
            UnsafeAttribute {
                column: 19,
                start_byte: 0,
                symbol: Some("export_name".to_string()),
            },
        ];
        let mut findings = Vec::new();

        scan_unsafe_constructs(context, &[], &[], &mut findings);
        assert!(findings.is_empty());

        let associations = associations_with(Some(SafetyCommentAssociation::Attached));
        scan_unsafe_constructs(
            unsafe_context(&container, &modules, &associations),
            &[],
            &attributes,
            &mut findings,
        );

        assert_eq!(findings.len(), 2);
        let symbols = findings
            .iter()
            .map(|finding| finding.identity.symbol.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(symbols, vec![Some("no_mangle"), Some("export_name")]);
        assert!(findings.iter().all(|finding| {
            finding.kind == FindingKind::Unsafe
                && finding.family.as_deref() == Some("unsafe_attr")
                && finding.identity.ast_kind == "unsafe_attr"
                && finding.identity.container.as_deref() == Some("ffi::symbols::export")
                && finding.identity.module.as_deref() == Some("ffi::symbols")
                && finding.identity.target_fingerprint.as_deref() == Some("safety-comment:present")
        }));
    }

    fn unsafe_context<'a>(
        container: &'a Option<String>,
        module_stack: &'a [String],
        associations: &'a std::collections::BTreeMap<(u32, u32), SafetyCommentAssociation>,
    ) -> UnsafeLineContext<'a> {
        UnsafeLineContext {
            line: LineContext {
                path: Path::new("src/lib.rs"),
                line: "    unsafe fn read() { unsafe { core::ptr::read(ptr) } }",
                line_no: 42,
                container,
                module_stack,
            },
            line_no: 42,
            safety_comment_associations: associations,
        }
    }

    fn associations_with(
        association: Option<SafetyCommentAssociation>,
    ) -> std::collections::BTreeMap<(u32, u32), SafetyCommentAssociation> {
        let mut associations = std::collections::BTreeMap::new();
        if let Some(association) = association {
            for column in [3, 5, 9, 11, 17, 19] {
                associations.insert((42, column), association);
            }
        }
        associations
    }

    fn unsafe_construct(
        column: u32,
        kind: UnsafeSyntaxKind,
        symbol: Option<&str>,
    ) -> UnsafeSyntaxConstruct {
        UnsafeSyntaxConstruct {
            kind,
            column,
            start_byte: 0,
            symbol: symbol.map(str::to_string),
        }
    }

    fn finding_with_family<'a>(findings: &'a [Finding], family: &str) -> &'a Finding {
        findings
            .iter()
            .find(|finding| finding.family.as_deref() == Some(family))
            .unwrap_or_else(|| std::panic::panic_any(format!("expected {family} finding")))
    }
}
