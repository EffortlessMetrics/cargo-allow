use allow_core::{CargoAllowError, CargoAllowResult};
use std::fs;
use std::path::Path;

pub(crate) fn read_policy(path: &Path) -> CargoAllowResult<String> {
    fs::read_to_string(path).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to read legacy policy {}: {e}",
            path.display()
        ))
    })
}

pub(crate) fn legacy_table(input: &str) -> CargoAllowResult<Option<toml::Table>> {
    toml::from_str::<toml::Table>(input)
        .map(Some)
        .map_err(|e| CargoAllowError::new(format!("failed to parse legacy policy TOML: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn read_policy_returns_file_text_or_contextual_error() {
        let path = temp_policy_path("read-policy");
        fs::write(&path, "policy = \"non-rust-allowlist\"\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

        let text = read_policy(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy reads: {err}")));

        assert_eq!(text, "policy = \"non-rust-allowlist\"\n");
        fs::remove_file(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture cleanup: {err}")));

        let err = read_policy(&path).expect_err("missing policy should produce read error");
        assert!(err.to_string().contains("failed to read legacy policy"));
        assert!(err.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn legacy_table_parses_toml_or_reports_parse_error() {
        let table = legacy_table("policy = \"workflow-allowlist\"\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("legacy TOML parses: {err}")))
            .unwrap_or_else(|| std::panic::panic_any("legacy table exists"));

        assert_eq!(
            table.get("policy").and_then(toml::Value::as_str),
            Some("workflow-allowlist")
        );

        let err = legacy_table("policy = [").expect_err("invalid TOML should fail");
        assert!(
            err.to_string()
                .contains("failed to parse legacy policy TOML")
        );
    }

    fn temp_policy_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|err| std::panic::panic_any(format!("system clock ok: {err}")))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cargo-allow-{name}-{}-{unique}.toml",
            std::process::id()
        ))
    }
}
