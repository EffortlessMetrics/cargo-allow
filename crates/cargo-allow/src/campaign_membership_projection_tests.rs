//! Membership-projection tests for the #3846 campaign issue closeout
//! guard: the checked denominator in `policy/campaign-issue-closeout.toml`
//! is the only scope authority, and a stale or malformed projection
//! fails closed for the affected issue without touching unrelated
//! repository issues.

use serde::Deserialize;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel))
        .expect("the campaign denominator is retained in the tree")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipChild {
    issue: u64,
    required: String,
    #[serde(default)]
    accepted: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignTable {
    id: u64,
    base_branch: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipProjection {
    campaign: CampaignTable,
    #[serde(default)]
    children: Vec<MembershipChild>,
}

/// Known closeout verdict vocabulary; anything else cannot be a
/// required or accepted result.
const KNOWN_RESULTS: [&str; 3] = ["Complete", "NotPlanned", "Duplicate"];

fn validate(projection: &MembershipProjection) -> Vec<String> {
    let mut failures = Vec::new();
    if projection.campaign.id != 3768 {
        failures.push(format!("campaign_id_mismatch: {}", projection.campaign.id));
    }
    if projection.campaign.base_branch.trim().is_empty() {
        failures.push("base_branch_missing".to_string());
    }
    let mut seen: Vec<u64> = Vec::new();
    for child in &projection.children {
        if seen.contains(&child.issue) {
            failures.push(format!("duplicate_issue: {}", child.issue));
        } else {
            seen.push(child.issue);
        }
        let accepted = child
            .accepted
            .clone()
            .unwrap_or_else(|| vec![child.required.clone()]);
        if accepted.is_empty() {
            failures.push(format!("invalid_accepted_results: {}", child.issue));
        }
        if !KNOWN_RESULTS.contains(&child.required.as_str()) {
            failures.push(format!("unknown_required_result: {}", child.issue));
        }
        if !accepted.contains(&child.required) {
            failures.push(format!("required_not_accepted: {}", child.issue));
        }
        for result in &accepted {
            if !KNOWN_RESULTS.contains(&result.as_str()) {
                failures.push(format!("unknown_accepted_result: {}", child.issue));
            }
        }
    }
    failures
}

#[test]
fn campaign_membership_projection_loads_the_checked_denominator() {
    let root = workspace_root();
    let text = read_workspace_file(&root, "policy/campaign-issue-closeout.toml");
    let projection: MembershipProjection =
        toml::from_str(&text).expect("the checked denominator parses");
    assert_eq!(projection.campaign.id, 3768);
    assert_eq!(projection.campaign.base_branch, "main");
    assert!(
        projection.children.len() >= 10,
        "the 0.2.0 campaign denominator is explicit and non-empty"
    );
    let validation = validate(&projection);
    assert!(
        validation.is_empty(),
        "the checked denominator is coherent: {validation:?}"
    );
}

#[test]
fn campaign_membership_projection_fails_closed_on_incoherent_rows() {
    // Duplicate issue: one row must not shadow another.
    let duplicated: MembershipProjection = toml::from_str(
        r#"
[campaign]
id = 3768
base_branch = "main"
[[children]]
issue = 3846
required = "Complete"
[[children]]
issue = 3846
required = "Complete"
"#,
    )
    .expect("rows parse");
    assert!(
        validate(&duplicated)
            .iter()
            .any(|failure| failure.starts_with("duplicate_issue"))
    );

    // A required result outside its accepted set is incoherent.
    let unaccepted: MembershipProjection = toml::from_str(
        r#"
[campaign]
id = 3768
base_branch = "main"
[[children]]
issue = 3846
required = "Complete"
accepted = ["NotPlanned"]
"#,
    )
    .expect("rows parse");
    assert!(
        validate(&unaccepted)
            .iter()
            .any(|failure| failure.starts_with("required_not_accepted"))
    );

    // Unknown verdicts cannot be required or accepted.
    let unknown: MembershipProjection = toml::from_str(
        r#"
[campaign]
id = 3768
base_branch = "main"
[[children]]
issue = 3846
required = "Eventually"
"#,
    )
    .expect("rows parse");
    assert!(
        validate(&unknown)
            .iter()
            .any(|failure| failure.starts_with("unknown_required_result"))
    );

    // A wrong campaign id fails the whole projection.
    let wrong_campaign: MembershipProjection = toml::from_str(
        r#"
[campaign]
id = 9999
base_branch = "main"
[[children]]
issue = 3846
required = "Complete"
"#,
    )
    .expect("rows parse");
    assert!(
        validate(&wrong_campaign)
            .iter()
            .any(|failure| failure.starts_with("campaign_id_mismatch"))
    );

    // Unknown fields fail closed (deny_unknown_fields).
    let hostile: Result<MembershipProjection, _> = toml::from_str(
        r#"
[campaign]
id = 3768
base_branch = "main"
priority_ranking = [{issue = 3846, rank = 1}]
"#,
    );
    assert!(
        hostile.is_err(),
        "the projection is scope authority, not a priority system"
    );
}

#[test]
fn campaign_membership_projection_scopes_the_guard_to_the_denominator() {
    // Negative controls 7 and 8: an issue outside the denominator is
    // never touched, and the runtime loader consumes exactly this file
    // with the same coherence law.
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/verify-campaign-issue-closeout.py");
    assert!(
        script.contains("policy/campaign-issue-closeout.toml"),
        "the runtime guard consumes the checked denominator"
    );
    assert!(
        script.contains("not isinstance(number, int) or number not in membership"),
        "an issue outside the denominator gets no action"
    );
    assert!(
        script.contains("duplicates issue"),
        "the runtime loader rejects duplicate membership rows"
    );
    assert!(
        script.contains("required result is not accepted"),
        "the runtime loader rejects incoherent required/accepted rows"
    );
}
