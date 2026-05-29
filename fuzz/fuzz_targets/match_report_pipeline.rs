#![no_main]

use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchOutcome, MatchStatus,
    Selector, Span, StructuralIdentity,
};
use allow_match::{CheckMode, evaluate};
use allow_report::{
    ReportContext, render_human_with_context, render_json_with_context,
    render_markdown_with_context, render_sarif_with_context,
};
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

fuzz_target!(|data: &[u8]| {
    let mut cursor = Cursor::new(data);
    let mut config = AllowConfig::empty();
    config.owner = cursor.option_string(16);
    config.status = cursor.option_string(12);
    config.requirements.owner_required = cursor.bool();
    config.requirements.reason_required = cursor.bool();
    config.requirements.classification_required = cursor.bool();
    config.requirements.evidence_required = cursor.bool();
    config.requirements.expires_or_review_after_required = cursor.bool();
    config.requirements.allow_bare_allow_attributes = cursor.bool();
    config.requirements.lint_policy_id_required = cursor.bool();
    config.requirements.stale_entries_fail = cursor.bool();
    config.requirements.unsafe_evidence_required = cursor.bool();
    config.requirements.unsafe_safety_comment_required = cursor.bool();

    let finding_count = cursor.count(6);
    let mut findings = Vec::with_capacity(finding_count);
    for index in 0..finding_count {
        findings.push(cursor.finding(index));
    }

    let entry_count = cursor.count(6);
    for index in 0..entry_count {
        config
            .allow
            .push(cursor.entry(index, findings.get(index % finding_count.max(1))));
    }

    let mode = match cursor.byte() % 4 {
        0 => CheckMode::Audit,
        1 => CheckMode::NoNew,
        2 => CheckMode::Strict,
        _ => CheckMode::Release,
    };
    let outcomes = evaluate(&config, &findings, mode);
    let failed = outcomes.iter().any(|outcome| mode.fails(outcome.status));
    render_all(&findings, &outcomes, failed);

    // Also cover report rendering for arbitrary standalone outcome/status mixes,
    // including statuses evaluate() only emits for validated policies.
    let extra_count = cursor.count(8);
    let mut extra = Vec::with_capacity(extra_count);
    for _ in 0..extra_count {
        extra.push(cursor.outcome(finding_count));
    }
    render_all(&findings, &extra, cursor.bool());
});

fn render_all(findings: &[Finding], outcomes: &[MatchOutcome], failed: bool) {
    let context = ReportContext::source_syntax(
        "fuzz",
        Some("."),
        Some(findings.len()),
        Some(outcomes.len()),
    );
    let json = render_json_with_context("fuzz", findings, outcomes, failed, context);
    serde_json::from_str::<serde_json::Value>(&json).expect("report JSON must parse");
    let sarif = render_sarif_with_context("fuzz", findings, outcomes, failed, context);
    serde_json::from_str::<serde_json::Value>(&sarif).expect("SARIF JSON must parse");
    let _ = render_human_with_context("fuzz", findings, outcomes, failed, context);
    let _ = render_markdown_with_context("fuzz", findings, outcomes, failed, context);
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn byte(&mut self) -> u8 {
        let value = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        value
    }

    fn bool(&mut self) -> bool {
        self.byte() & 1 == 1
    }

    fn count(&mut self, max: usize) -> usize {
        usize::from(self.byte()) % (max + 1)
    }

    fn small_u32(&mut self) -> u32 {
        u32::from(self.byte()) % 128
    }

    fn string(&mut self, max_len: usize) -> String {
        let len = self.count(max_len);
        let mut out = String::with_capacity(len);
        for _ in 0..len {
            let byte = self.byte();
            let ch = match byte % 40 {
                0..=9 => char::from(b'0' + (byte % 10)),
                10..=35 => char::from(b'a' + ((byte - 10) % 26)),
                36 => '-',
                37 => '_',
                38 => '/',
                _ => '.',
            };
            out.push(ch);
        }
        if out.is_empty() { "x".to_string() } else { out }
    }

    fn option_string(&mut self, max_len: usize) -> Option<String> {
        self.bool().then(|| self.string(max_len))
    }

    fn path(&mut self) -> PathBuf {
        let base = self.string(18).trim_matches('/').to_string();
        let file = if base.ends_with(".rs") {
            base
        } else {
            format!("{base}.rs")
        };
        PathBuf::from(file)
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

    fn identity(&mut self) -> StructuralIdentity {
        let mut identity = StructuralIdentity::new(self.string(10), self.string(14));
        identity.crate_name = self.option_string(12);
        identity.module = self.option_string(16);
        identity.container = self.option_string(16);
        identity.symbol = self.option_string(16);
        identity.callee = self.option_string(16);
        identity.macro_name = self.option_string(16);
        identity.lint = self.option_string(16);
        identity.receiver_fingerprint = self.option_string(20);
        identity.target_fingerprint = self.option_string(20);
        identity.normalized_snippet_hash = self.option_string(20);
        identity.line_hint = self.bool().then(|| self.small_u32());
        identity.column_hint = self.bool().then(|| self.small_u32());
        identity
    }

    fn finding(&mut self, index: usize) -> Finding {
        let span = self.bool().then(|| Span {
            line: self.small_u32() + 1,
            column: self.small_u32() + 1,
        });
        Finding {
            kind: self.kind(),
            family: self.option_string(14),
            path: self.path(),
            span,
            identity: self.identity(),
            message: format!("fuzz finding {index}: {}", self.string(24)),
        }
    }

    fn entry(&mut self, index: usize, seed: Option<&Finding>) -> AllowEntry {
        let kind = if self.bool() {
            seed.map(|f| f.kind).unwrap_or_else(|| self.kind())
        } else {
            self.kind()
        };
        let path = if self.bool() {
            seed.map(|f| f.path.clone()).or_else(|| Some(self.path()))
        } else {
            None
        };
        let mut selector = Selector::default();
        if let Some(seed) = seed.filter(|_| self.bool()) {
            selector.ast_kind = Some(seed.identity.ast_kind.clone());
            selector.container = seed.identity.container.clone();
            selector.callee = seed.identity.callee.clone();
            selector.macro_name = seed.identity.macro_name.clone();
            selector.lint = seed.identity.lint.clone();
            selector.symbol = seed.identity.symbol.clone();
            selector.receiver_fingerprint = seed.identity.receiver_fingerprint.clone();
            selector.target_fingerprint = seed.identity.target_fingerprint.clone();
            selector.normalized_snippet_hash = seed.identity.normalized_snippet_hash.clone();
            selector.line_hint = seed.identity.line_hint;
        } else {
            selector.ast_kind = self.option_string(14);
            selector.container = self.option_string(16);
            selector.callee = self.option_string(16);
            selector.macro_name = self.option_string(16);
            selector.lint = self.option_string(16);
            selector.symbol = self.option_string(16);
            selector.receiver_fingerprint = self.option_string(20);
            selector.target_fingerprint = self.option_string(20);
            selector.normalized_snippet_hash = self.option_string(20);
            selector.line_hint = self.bool().then(|| self.small_u32());
            selector.glob = self.option_string(20);
        }
        AllowEntry {
            id: format!("allow-fuzz-{index}-{}", self.string(8)),
            kind,
            family: self
                .option_string(14)
                .or_else(|| seed.and_then(|f| f.family.clone())),
            path,
            glob: self.option_string(20),
            owner: self.string(12),
            classification: self.string(14),
            reason: self.string(32),
            evidence: (0..self.count(3)).map(|_| self.string(24)).collect(),
            links: (0..self.count(3)).map(|_| self.string(24)).collect(),
            occurrence_limit: self.bool().then(|| self.small_u32()),
            lifecycle: Lifecycle {
                created: self.option_date(),
                review_after: self.option_date(),
                expires: self.option_date(),
            },
            selector,
            last_seen: self.bool().then(|| LastSeen {
                line: self.small_u32(),
                column: self.small_u32(),
            }),
        }
    }

    fn option_date(&mut self) -> Option<String> {
        self.bool().then(|| {
            format!(
                "20{:02}-{:02}-{:02}",
                self.byte() % 40,
                (self.byte() % 12) + 1,
                (self.byte() % 28) + 1
            )
        })
    }

    fn status(&mut self) -> MatchStatus {
        match self.byte() % 10 {
            0 => MatchStatus::Matched,
            1 => MatchStatus::New,
            2 => MatchStatus::Stale,
            3 => MatchStatus::Expired,
            4 => MatchStatus::ReviewDue,
            5 => MatchStatus::Ambiguous,
            6 => MatchStatus::InvalidSelector,
            7 => MatchStatus::MissingRequiredField,
            8 => MatchStatus::EvidenceMissing,
            _ => MatchStatus::BaselineDebt,
        }
    }

    fn outcome(&mut self, finding_count: usize) -> MatchOutcome {
        MatchOutcome {
            status: self.status(),
            allow_id: self.option_string(18),
            finding_index: (finding_count > 0 && self.bool())
                .then(|| usize::from(self.byte()) % finding_count),
            message: self.string(40),
            score: self.small_u32(),
        }
    }
}
