mod support;

use std::fs;

use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn saved_json_artifact_commands_are_quiet() {
    let root = temp_root("artifact-output");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| panic!("write policy: {err}"));

    for command in [
        ArtifactCommand {
            name: "doctor",
            args: vec!["doctor", "--format", "json"],
            schema_id: "cargo-allow.doctor.v1",
        },
        ArtifactCommand {
            name: "list",
            args: vec!["list", "--format", "json"],
            schema_id: "cargo-allow.list.v1",
        },
        ArtifactCommand {
            name: "worklist",
            args: vec!["worklist", "--format", "json"],
            schema_id: "cargo-allow.worklist.v1",
        },
        ArtifactCommand {
            name: "prune",
            args: vec!["prune", "--stale", "--format", "json"],
            schema_id: "cargo-allow.prune.v1",
        },
        ArtifactCommand {
            name: "explain",
            args: vec!["explain", "allow-policy", "--format", "json"],
            schema_id: "cargo-allow.explain.v1",
        },
    ] {
        let output = root.join(format!("{}.json", command.name));
        let mut args = command.args.clone();
        args.extend(["--root", root.to_str().unwrap_or(""), "--output"]);
        let output_text = output.to_string_lossy().to_string();
        args.push(&output_text);

        let result = cargo_allow_command()
            .args(args)
            .output()
            .unwrap_or_else(|err| {
                panic!("run cargo-allow {}: {err}", command.name)
            });

        assert_status(command.name, &result, true);
        assert_stdout_empty(
            command.name,
            &result,
            "--output should not emit artifact JSON to stdout",
        );
        assert_stderr_empty(
            command.name,
            &result,
            "--output should not emit side-channel status to stderr",
        );
        assert_saved_json_artifact(&output, command.name, command.schema_id, command.name);
    }

    remove_temp_root(root);
}

#[derive(Clone)]
struct ArtifactCommand {
    name: &'static str,
    args: Vec<&'static str>,
    schema_id: &'static str,
}

fn policy() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"

[[allow]]
id = "allow-stale"
kind = "non_rust_file"
family = "documentation"
path = "docs/missing.md"
owner = "core"
classification = "fixture"
reason = "fixture stale entry"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/missing.md"
target_fingerprint = "md"
glob = "docs/missing.md"
"#
}
