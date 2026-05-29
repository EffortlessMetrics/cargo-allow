#![no_main]

use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Requirements, Selector,
    Span, StructuralIdentity, WorkspaceConfig, finding_identity_key,
};
use allow_match::{CheckMode, evaluate, score_match};
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

struct Cursor<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn byte(&mut self) -> u8 {
        let byte = self.data.get(self.offset).copied().unwrap_or(0);
        self.offset = self.offset.saturating_add(1);
        byte
    }

    fn bool(&mut self) -> bool {
        self.byte() & 1 == 1
    }

    fn string(&mut self, max: usize) -> String {
        let len = usize::from(self.byte()) % (max + 1);
        let end = self.offset.saturating_add(len).min(self.data.len());
        let s = String::from_utf8_lossy(&self.data[self.offset..end]).into_owned();
        self.offset = end;
        s.chars()
            .filter(|ch| !ch.is_control())
            .take(max)
            .collect::<String>()
    }

    fn option_string(&mut self, max: usize) -> Option<String> {
        self.bool().then(|| self.string(max))
    }

    fn kind(&mut self) -> FindingKind {
        match self.byte() % 6 {
            0 => FindingKind::Panic,
            1 => FindingKind::Unsafe,
            2 => FindingKind::LintException,
            3 => FindingKind::NonRustFile,
            4 => FindingKind::GeneratedCode,
            _ => FindingKind::PolicyException,
        }
    }

    fn path(&mut self) -> PathBuf {
        let name = self.string(24);
        if name.is_empty() {
            PathBuf::from("src/lib.rs")
        } else {
            PathBuf::from(name)
        }
    }
}

fn identity(cursor: &mut Cursor<'_>) -> StructuralIdentity {
    let mut identity = StructuralIdentity::new(cursor.string(12), cursor.string(16));
    identity.crate_name = cursor.option_string(12);
    identity.module = cursor.option_string(24);
    identity.container = cursor.option_string(24);
    identity.symbol = cursor.option_string(32);
    identity.callee = cursor.option_string(16);
    identity.macro_name = cursor.option_string(16);
    identity.lint = cursor.option_string(24);
    identity.receiver_fingerprint = cursor.option_string(32);
    identity.target_fingerprint = cursor.option_string(32);
    identity.normalized_snippet_hash = cursor.option_string(24);
    identity.line_hint = cursor.bool().then(|| u32::from(cursor.byte()) + 1);
    identity.column_hint = cursor.bool().then(|| u32::from(cursor.byte()) + 1);
    identity
}

fn finding(cursor: &mut Cursor<'_>) -> Finding {
    Finding {
        kind: cursor.kind(),
        family: cursor.option_string(16),
        path: cursor.path(),
        span: cursor.bool().then(|| Span {
            line: u32::from(cursor.byte()) + 1,
            column: u32::from(cursor.byte()) + 1,
        }),
        identity: identity(cursor),
        message: cursor.string(48),
    }
}

fn selector(cursor: &mut Cursor<'_>, finding: &Finding) -> Selector {
    Selector {
        ast_kind: cursor.bool().then(|| finding.identity.ast_kind.clone()),
        container: cursor
            .bool()
            .then(|| finding.identity.container.clone())
            .flatten(),
        callee: cursor
            .bool()
            .then(|| finding.identity.callee.clone())
            .flatten(),
        macro_name: cursor
            .bool()
            .then(|| finding.identity.macro_name.clone())
            .flatten(),
        lint: cursor
            .bool()
            .then(|| finding.identity.lint.clone())
            .flatten(),
        symbol: cursor
            .bool()
            .then(|| finding.identity.symbol.clone())
            .flatten(),
        receiver_fingerprint: cursor
            .bool()
            .then(|| finding.identity.receiver_fingerprint.clone())
            .flatten(),
        target_fingerprint: cursor
            .bool()
            .then(|| finding.identity.target_fingerprint.clone())
            .flatten(),
        normalized_snippet_hash: cursor
            .bool()
            .then(|| finding.identity.normalized_snippet_hash.clone())
            .flatten(),
        line_hint: cursor.bool().then(|| u32::from(cursor.byte()) + 1),
        glob: cursor.bool().then(|| "**/*".to_string()),
    }
}

fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    let finding = finding(&mut cursor);
    let mut entry = AllowEntry {
        id: "FUZZ-1".to_string(),
        kind: if cursor.bool() {
            finding.kind
        } else {
            cursor.kind()
        },
        family: cursor.bool().then(|| finding.family.clone()).flatten(),
        path: cursor.bool().then(|| finding.path.clone()),
        glob: cursor.bool().then(|| "**/*".to_string()),
        owner: cursor.string(16),
        classification: cursor.string(16),
        reason: cursor.string(32),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: cursor.bool().then(|| u32::from(cursor.byte() % 3)),
        lifecycle: Lifecycle::empty(),
        selector: selector(&mut cursor, &finding),
        last_seen: cursor.bool().then(|| LastSeen {
            line: u32::from(cursor.byte()) + 1,
            column: u32::from(cursor.byte()) + 1,
        }),
    };

    if entry.path.is_none() && entry.glob.is_none() && entry.selector.glob.is_none() {
        entry.path = Some(finding.path.clone());
    }

    let _ = finding_identity_key(&finding);
    let _ = score_match(&entry, &finding);

    let config = AllowConfig {
        schema_version: "0.1".to_string(),
        policy: "cargo-allow".to_string(),
        owner: None,
        status: Some("active".to_string()),
        workspace: WorkspaceConfig::default(),
        requirements: Requirements::default(),
        allow: vec![entry],
    };
    let _ = evaluate(&config, &[finding], CheckMode::NoNew);
});
