pub fn starter_policy(strict: bool) -> String {
    let stale = if strict { "true" } else { "false" };
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "{}"
ignored = [".git/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = {stale}

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
        if strict { "strict" } else { "no-new" }
    )
}
