use allow_core::{Finding, FindingKind, Selector, normalize_path};

pub(crate) fn selector_from_finding(finding: &Finding) -> Selector {
    Selector {
        ast_kind: Some(finding.identity.ast_kind.clone()),
        container: finding.identity.container.clone(),
        callee: finding.identity.callee.clone(),
        macro_name: finding.identity.macro_name.clone(),
        lint: finding.identity.lint.clone(),
        symbol: finding.identity.symbol.clone(),
        receiver_fingerprint: finding.identity.receiver_fingerprint.clone(),
        target_fingerprint: finding.identity.target_fingerprint.clone(),
        normalized_snippet_hash: finding.identity.normalized_snippet_hash.clone(),
        // line_hint is deliberately None: into_selector also forces None on
        // load, so setting it here would make the in-memory fingerprint differ
        // from the on-disk fingerprint after the next reload (#2503).
        line_hint: None,
        glob: matches!(
            finding.kind,
            FindingKind::NonRustFile | FindingKind::GeneratedCode
        )
        .then(|| normalize_path(&finding.path)),
    }
}

#[cfg(test)]
mod tests {
    use super::selector_from_finding;
    use allow_core::{Finding, FindingKind, Selector, Span, StructuralIdentity};
    use std::path::PathBuf;

    fn finding(kind: FindingKind, path: &str, identity: StructuralIdentity) -> Finding {
        Finding {
            kind,
            family: Some("fixture".to_string()),
            path: PathBuf::from(path),
            span: Some(Span {
                line: 42,
                column: 7,
            }),
            identity,
            message: "fixture".to_string(),
            ledger: None,
        }
    }

    #[test]
    fn selector_from_finding_preserves_structural_identity_fields() {
        let mut identity = StructuralIdentity::new("rust", "method_call");
        identity.container = Some("Parser::parse".to_string());
        identity.callee = Some("unwrap".to_string());
        identity.macro_name = Some("debug_assert".to_string());
        identity.lint = Some("clippy::unwrap_used".to_string());
        identity.symbol = Some("value.unwrap()".to_string());
        identity.receiver_fingerprint = Some("recv:value".to_string());
        identity.target_fingerprint = Some("target:policy".to_string());
        identity.normalized_snippet_hash = Some("fnv1a64:abc".to_string());
        identity.line_hint = Some(41);

        let selector = selector_from_finding(&finding(FindingKind::Panic, "src/lib.rs", identity));

        assert_eq!(
            selector,
            Selector {
                ast_kind: Some("method_call".to_string()),
                container: Some("Parser::parse".to_string()),
                callee: Some("unwrap".to_string()),
                macro_name: Some("debug_assert".to_string()),
                lint: Some("clippy::unwrap_used".to_string()),
                symbol: Some("value.unwrap()".to_string()),
                receiver_fingerprint: Some("recv:value".to_string()),
                target_fingerprint: Some("target:policy".to_string()),
                normalized_snippet_hash: Some("fnv1a64:abc".to_string()),
                line_hint: None,
                glob: None,
            }
        );
    }

    #[test]
    fn selector_from_finding_adds_glob_only_for_file_inventory_findings() {
        let cases = [
            (
                FindingKind::NonRustFile,
                "docs/README.md",
                Some("docs/README.md"),
            ),
            (
                FindingKind::GeneratedCode,
                "target/generated.rs",
                Some("target/generated.rs"),
            ),
            (FindingKind::Unsafe, "src/lib.rs", None),
        ];

        for (kind, path, expected_glob) in cases {
            let selector = selector_from_finding(&finding(
                kind,
                path,
                StructuralIdentity::new("rust", "file"),
            ));

            assert_eq!(selector.ast_kind.as_deref(), Some("file"));
            assert_eq!(selector.line_hint, None);
            assert_eq!(selector.glob.as_deref(), expected_glob);
        }
    }
}
