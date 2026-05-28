use allow_core::{
    Finding, FindingKind, Span, StructuralIdentity, normalize_snippet, stable_hash_hex,
};
use std::path::PathBuf;

pub(super) fn process_policy_finding(path: &str, symbol: &str) -> Finding {
    let mut identity = StructuralIdentity::new("policy", "process_spawn");
    identity.symbol = Some(symbol.to_string());
    identity.target_fingerprint = Some(format!("process:{symbol}"));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("process_spawn".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn network_policy_finding(symbol: &str) -> Finding {
    let mut identity = StructuralIdentity::new("policy", "network_destination");
    identity.symbol = Some(symbol.to_string());
    identity.target_fingerprint = Some(format!("network:{symbol}"));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("network_destination".to_string()),
        path: PathBuf::from("policy/network-allowlist.toml"),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn panic_finding(
    path: &str,
    family: &str,
    ast_kind: &str,
    callee: Option<&str>,
    macro_name: Option<&str>,
    snippet: &str,
) -> Finding {
    let mut identity = StructuralIdentity::new("rust", ast_kind);
    identity.callee = callee.map(str::to_string);
    identity.macro_name = macro_name.map(str::to_string);
    identity.normalized_snippet_hash = Some(stable_hash_hex(&normalize_snippet(snippet)));
    Finding {
        kind: FindingKind::Panic,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn lint_finding(
    path: &str,
    family: &str,
    lint: &str,
    policy_id: Option<&str>,
) -> Finding {
    let mut identity = StructuralIdentity::new("rust", "attribute");
    identity.lint = Some(lint.to_string());
    identity.symbol = Some(format!(
        "#[expect({lint}, reason = \"policy:{}\")]",
        policy_id.unwrap_or("unlinked")
    ));
    identity.target_fingerprint = policy_id.map(|id| format!("policy:{id}"));
    Finding {
        kind: FindingKind::LintException,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn unsafe_finding(path: &str, family: &str, container: Option<&str>) -> Finding {
    let mut identity = StructuralIdentity::new("rust", family);
    identity.container = container.map(str::to_string);
    Finding {
        kind: FindingKind::Unsafe,
        family: Some(family.to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: String::new(),
    }
}

pub(super) fn finding(path: &str, ast_kind: &str) -> Finding {
    Finding {
        kind: FindingKind::NonRustFile,
        family: Some("configuration".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: String::new(),
    }
}
