use super::*;

#[test]
fn finding_posture_ignores_line_movement_for_same_identity() {
    let base = vec![finding("src/lib.rs", 10, "load")];
    let head = vec![finding("src/lib.rs", 99, "load")];

    let changes = finding_posture_changes(&base, &head);

    assert!(changes.is_empty());
}

#[test]
fn finding_posture_reports_new_and_removed_findings() {
    let base = vec![finding("src/old.rs", 10, "old")];
    let head = vec![finding("src/new.rs", 10, "new")];

    let changes = finding_posture_changes(&base, &head);

    assert!(
        changes.iter().any(|change| {
            change.kind == FindingPostureKind::New && change.path == "src/new.rs"
        })
    );
    assert!(changes.iter().any(|change| {
        change.kind == FindingPostureKind::Removed && change.path == "src/old.rs"
    }));
}

#[test]
fn finding_posture_reports_count_changes_for_same_identity() {
    let base = vec![finding("src/lib.rs", 10, "load")];
    let head = vec![
        finding("src/lib.rs", 10, "load"),
        finding("src/lib.rs", 20, "load"),
    ];

    let changes = finding_posture_changes(&base, &head);

    assert_eq!(changes.len(), 1);
    let change = changes
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one posture change"));
    assert_eq!(change.kind, FindingPostureKind::New);
    assert_eq!(change.path, "src/lib.rs");
}

#[test]
fn finding_posture_preserves_source_package_context() {
    let mut head = finding("src/lib.rs", 10, "load");
    head.identity.crate_name = Some("parser".to_string());

    let changes = finding_posture_changes(&[], &[head]);

    let change = changes
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one posture change"));
    assert_eq!(change.source_package.as_deref(), Some("parser"));
}
