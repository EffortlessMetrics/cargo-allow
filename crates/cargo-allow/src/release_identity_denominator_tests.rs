//! Checked denominator of release-identity consumers (#3752).
//!
//! One Rust authority owns release-version channel semantics:
//! `allow-report::ReleaseIdentityV1` projected by the hidden
//! `cargo-allow release-identity` command. Every non-Rust site that parses,
//! infers, or projects release-version semantics must be declared in
//! `policy/release-identity-consumers.toml` with a disposition. The checks
//! here fail when release-semantic parsing appears in a scanned surface
//! without a declaring row, when a declared row no longer matches the file,
//! or when release-channel inference appears outside the Rust owner.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const CONSUMER_DENOMINATOR: &str = "policy/release-identity-consumers.toml";
const DENOMINATOR_SCHEMA: &str = "cargo-allow.release-identity-consumers.v1";
const RELEASE_IDENTITY_AUTHORITY: &str = "crates/allow-report/src/artifacts/release_identity_v1.rs";
const RELEASE_IDENTITY_COMMAND: &str = "crates/cargo-allow/src/cli/release_identity_command.rs";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum MarkerClass {
    /// Shell tag-to-version or version-to-tag construction (`#v}`,
    /// `#{prefix}`, `%${suffix}`).
    TagStrip,
    /// Escaped-dot plus digit-class regex fragments (`\.` together with
    /// `\d`, `[0-9]`, or `[1-9]`).
    VersionRegex,
    /// Raw `[workspace.package]` section readers.
    WorkspaceReader,
    /// `prerelease` posture tokens (GitHub prerelease booleans, prerelease
    /// ordering comparators, prerelease shape checks).
    PrereleasePosture,
    /// Changie `VersionNoPrefix` tag-prefix delegation.
    VersionNoPrefix,
    /// Release-channel inference from an `-rc` marker (`*-rc` globs,
    /// quoted `-rc` comparisons, `rc\.` regex fragments).
    RcChannelInference,
}

impl MarkerClass {
    fn detected_in_line(self, lower_line: &str, line: &str) -> bool {
        match self {
            Self::TagStrip => {
                line.contains("#v}") || line.contains("#${prefix}") || line.contains("%${suffix}")
            }
            Self::VersionRegex => {
                line.contains("\\.")
                    && (line.contains("\\d") || line.contains("[0-9]") || line.contains("[1-9]"))
            }
            Self::WorkspaceReader => line.contains("[workspace"),
            Self::PrereleasePosture => lower_line.contains("prerelease"),
            Self::VersionNoPrefix => line.contains("VersionNoPrefix"),
            Self::RcChannelInference => {
                line.contains("*-rc")
                    || line.contains("'-rc'")
                    || line.contains("\"-rc\"")
                    || line.contains("rc\\.")
            }
        }
    }

    fn all() -> [Self; 6] {
        [
            Self::TagStrip,
            Self::VersionRegex,
            Self::WorkspaceReader,
            Self::PrereleasePosture,
            Self::VersionNoPrefix,
            Self::RcChannelInference,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ConsumerDisposition {
    TypedProducer,
    TypedConsumer,
    ExactValuePassThrough,
    HistoricalFixtureOnly,
    NonSemanticFormatting,
    SupersededParserToDelete,
    DeferredWithNamedOwner,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumerDenominator {
    schema: String,
    schema_version: u32,
    controlling_issue: u32,
    guard: String,
    scan_roots: Vec<String>,
    consumers: Vec<ConsumerRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumerRow {
    id: String,
    path: String,
    role: String,
    disposition: ConsumerDisposition,
    owner: Option<String>,
    #[serde(default)]
    marker_classes: Vec<MarkerClass>,
    #[serde(default)]
    exact_counts: Vec<ExactCount>,
    #[serde(default)]
    sites: Vec<String>,
    claim_boundary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExactCount {
    class: MarkerClass,
    count: usize,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn read_workspace_file(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn parse_denominator(root: &Path) -> ConsumerDenominator {
    let source = read_workspace_file(root, CONSUMER_DENOMINATOR);
    toml::from_str(&source).unwrap_or_else(|error| panic!("parse {CONSUMER_DENOMINATOR}: {error}"))
}

fn scanned_files(root: &Path, scan_roots: &[String]) -> Vec<String> {
    let mut files = BTreeSet::new();
    for scan_root in scan_roots {
        let path = root.join(scan_root);
        if path.is_file() {
            files.insert(scan_root.replace('\\', "/"));
        } else {
            collect_files(&path, &mut files, scan_root);
        }
    }
    files.into_iter().collect()
}

fn collect_files(directory: &Path, files: &mut BTreeSet<String>, relative: &str) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("scan {}: {error}", directory.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("scan entry: {error}"));
        let entry_path = entry.path();
        let entry_relative = format!("{relative}/{}", entry.file_name().to_string_lossy());
        if entry_path.is_dir() {
            collect_files(&entry_path, files, &entry_relative);
        } else if matches!(
            entry_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("sh") | Some("py") | Some("yml") | Some("yaml")
        ) {
            files.insert(entry_relative);
        }
    }
}

fn detected_classes_per_line(content: &str) -> Vec<BTreeSet<MarkerClass>> {
    content
        .lines()
        .map(|line| {
            let lower_line = line.to_ascii_lowercase();
            MarkerClass::all()
                .into_iter()
                .filter(|class| class.detected_in_line(&lower_line, line))
                .collect()
        })
        .collect()
}

fn detected_classes(content: &str) -> BTreeSet<MarkerClass> {
    detected_classes_per_line(content)
        .into_iter()
        .flat_map(|line_classes| line_classes.into_iter())
        .collect()
}

fn count_marker_lines(content: &str, class: MarkerClass) -> usize {
    detected_classes_per_line(content)
        .into_iter()
        .filter(|line_classes| line_classes.contains(&class))
        .count()
}

fn denominator_rows_by_path(denominator: &ConsumerDenominator) -> BTreeMap<&str, &ConsumerRow> {
    denominator
        .consumers
        .iter()
        .map(|row| (row.path.as_str(), row))
        .collect()
}

#[test]
fn denominator_loads_with_canonical_identity() {
    let root = workspace_root();
    let denominator = parse_denominator(&root);
    assert_eq!(denominator.schema, DENOMINATOR_SCHEMA);
    assert_eq!(denominator.schema_version, 1);
    assert_eq!(denominator.controlling_issue, 3752);
    assert_eq!(
        denominator.guard, "crates/cargo-allow/src/release_identity_denominator_tests.rs",
        "the denominator must name this guard as its checked enforcement"
    );
    assert!(
        denominator.consumers.len() >= 20,
        "the consumer denominator should cover the surveyed release surfaces"
    );
    let owned = denominator
        .consumers
        .iter()
        .filter(|row| row.disposition == ConsumerDisposition::TypedConsumer)
        .count();
    assert!(
        owned >= 1,
        "at least one consumer must already route through the typed authority"
    );
}

#[test]
fn denominator_rows_carry_complete_attribution() {
    let root = workspace_root();
    let denominator = parse_denominator(&root);

    let mut seen_ids = BTreeSet::new();
    for row in &denominator.consumers {
        assert!(
            !row.id.is_empty() && !row.role.is_empty() && !row.claim_boundary.is_empty(),
            "{} rows must carry a non-empty id, role, and claim boundary",
            row.path
        );
        assert!(
            seen_ids.insert(row.id.as_str()),
            "duplicate denominator row id {}",
            row.id
        );
        if matches!(
            row.disposition,
            ConsumerDisposition::DeferredWithNamedOwner
                | ConsumerDisposition::SupersededParserToDelete
        ) {
            let owner = row.owner.as_deref().unwrap_or_default();
            assert!(
                owner.starts_with('#'),
                "{} defers or deletes a parser without a named owning issue",
                row.path
            );
        }
    }
}

#[test]
fn rust_authority_owns_release_channel_semantics() {
    let root = workspace_root();
    let authority = read_workspace_file(&root, RELEASE_IDENTITY_AUTHORITY);
    assert!(
        authority.contains("fn parse(value: &str) -> Result<Self, ReleaseIdentityErrorV1>"),
        "ReleaseVersionV1::parse must remain the release grammar entry point"
    );
    assert!(
        authority.contains("fn parse_rc_ordinal"),
        "RC ordinal semantics must remain in the typed authority"
    );
    let command = read_workspace_file(&root, RELEASE_IDENTITY_COMMAND);
    assert!(
        command.contains("release-identity.v1"),
        "the projection command must keep its versioned schema identity"
    );
}

#[test]
fn declared_rows_match_detected_release_semantics() {
    let root = workspace_root();
    let denominator = parse_denominator(&root);
    let rows = denominator_rows_by_path(&denominator);
    let scanned: BTreeSet<String> = scanned_files(&root, &denominator.scan_roots)
        .into_iter()
        .collect();

    for (path, row) in &rows {
        let content = read_workspace_file(&root, path);

        for anchor in &row.sites {
            assert!(
                content.contains(anchor.as_str()),
                "{path} no longer contains the inventoried site anchor {anchor:?}; the denominator row is stale"
            );
        }

        // Rows outside the guarded scan surface (e.g. Rust-side
        // documentation rows) carry anchors only; marker-class equality is
        // only defined where the guard can detect constructs.
        if !scanned.contains(*path) {
            continue;
        }

        let detected = detected_classes(&content);
        let declared: BTreeSet<MarkerClass> = row.marker_classes.iter().copied().collect();
        assert_eq!(
            declared, detected,
            "{path} declares marker classes that no longer match the detected release semantics"
        );

        for exact in &row.exact_counts {
            let actual = count_marker_lines(&content, exact.class);
            assert_eq!(
                actual, exact.count,
                "{path} exact count for {:?} drifted from the denominator",
                exact.class
            );
        }
    }
}

#[test]
fn scanned_surfaces_have_no_undeclared_release_semantics() {
    let root = workspace_root();
    let denominator = parse_denominator(&root);
    let rows = denominator_rows_by_path(&denominator);

    for path in scanned_files(&root, &denominator.scan_roots) {
        let content = read_workspace_file(&root, &path);
        let detected = detected_classes(&content);
        match rows.get(path.as_str()) {
            Some(row) => {
                let declared: BTreeSet<MarkerClass> = row.marker_classes.iter().copied().collect();
                assert_eq!(
                    declared, detected,
                    "{path} gained or lost release-semantic constructs without a denominator update"
                );
            }
            None => assert!(
                detected.is_empty(),
                "{path} contains release-semantic parsing but has no row in {CONSUMER_DENOMINATOR}; declare the site or route it through `cargo-allow release-identity`"
            ),
        }
    }

    for path in rows.keys() {
        let exists = root.join(path).is_file();
        assert!(exists, "denominator row references missing file {path}");
    }
}

#[test]
fn release_channel_inference_stays_outside_scanned_surfaces() {
    let root = workspace_root();
    let denominator = parse_denominator(&root);
    for path in scanned_files(&root, &denominator.scan_roots) {
        let content = read_workspace_file(&root, &path);
        let detected = detected_classes(&content);
        assert!(
            !detected.contains(&MarkerClass::RcChannelInference),
            "{path} infers release channel from an -rc marker; consume `cargo-allow release-identity` channel projection instead"
        );
    }
}
