mod support;

use std::fs;
use std::process::Command;

use support::{remove_temp_root, temp_root};

#[test]
fn saved_json_artifact_commands_are_quiet() {
    let root = temp_root("artifact-output");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

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

        let result = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
            .args(args)
            .output()
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("run cargo-allow {}: {err}", command.name))
            });

        assert!(
            result.status.success(),
            "{} should pass: stdout=`{}` stderr=`{}`",
            command.name,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.stdout.is_empty(),
            "{} --output should not emit artifact JSON to stdout",
            command.name
        );
        assert!(
            result.stderr.is_empty(),
            "{} --output should not emit side-channel status to stderr: `{}`",
            command.name,
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            fs::read_to_string(&output)
                .unwrap_or_else(|err| std::panic::panic_any(format!(
                    "read {} output: {err}",
                    command.name
                )))
                .contains(&format!("\"schema_id\": \"{}\"", command.schema_id)),
            "{} output should be a saved JSON artifact",
            command.name
        );
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
