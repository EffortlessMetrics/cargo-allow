use allow_core::{AllowEntry, Finding, FindingKind, Lifecycle, Selector, StructuralIdentity};
use allow_match::classify_match;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

fn entry(index: usize) -> AllowEntry {
    AllowEntry {
        id: format!("allow-{index}"),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from(format!("src/module_{}/lib.rs", index % 100))),
        glob: None,
        owner: "bench".to_string(),
        classification: "benchmark".to_string(),
        reason: "benchmark fixture".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn finding(index: usize) -> Finding {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.callee = Some("unwrap".to_string());
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from(format!("src/module_{}/lib.rs", index % 100)),
        span: None,
        identity,
        message: String::new(),
        ledger: None,
    }
}

fn matching() {
    let entries: Vec<_> = (0..2_000).map(entry).collect();
    let findings: Vec<_> = (0..500).map(finding).collect();
    let start = Instant::now();
    let mut matches = 0;
    for _ in 0..10 {
        matches += findings
            .iter()
            .map(|finding| {
                entries
                    .iter()
                    .filter_map(|entry| classify_match(entry, finding))
                    .count()
            })
            .sum::<usize>();
    }
    println!("classify_match_500x2000: {:?} ({matches} matches)", start.elapsed());
}

fn main() {
    black_box(matching());
}
