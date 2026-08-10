//! cargo-intent provider discovery for #2601 one-way process delegation.
//!
//! Discovery order: explicit environment override, compatibility config, then PATH.
//! Never resolves monorepo workspace `target/` or `crates/` paths.

use allow_core::sha256_v1_bytes;
use serde::Deserialize;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

pub const INTENT_DELEGATION_CONFIG_SCHEMA_ID: &str = "cargo-allow.intent-delegation.v1";
pub const DEFAULT_INTENT_DELEGATION_CONFIG: &str = ".allow/compatibility/intent-delegation.toml";
pub const INTENT_PROVIDER_ENV_VAR: &str = "CARGO_INTENT_BIN";
pub const INTENT_PROVIDER_PRODUCT_NAME: &str = "cargo-intent";
pub const INTENT_PROVIDER_REQUIRED_VERSION_RANGE: &str = "0.1.x";
pub const INTENT_PROVIDER_REQUIRED_PROTOCOL: &str = "repo.analysis-receipt.v1";
pub const INTENT_PROVIDER_CANONICAL_COMMAND: &str =
    "cargo-intent --format json change status --staged --phase precommit";
pub const INTENT_PROVIDER_SUPPORT_REFERENCE: &str =
    "https://github.com/EffortlessMetrics/cargo-allow/blob/main/docs/status/SUPPORT_TIERS.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentProviderDiscoveryMode {
    ExplicitEnvironment,
    ExplicitConfig,
    PathLookup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentProviderFailureClass {
    Absent,
    ForbiddenWorkspaceTarget,
    ForbiddenWorkspaceCrate,
    WrongProductName,
    NotExecutable,
    MalformedConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentProviderFailure {
    pub class: IntentProviderFailureClass,
    pub detail: String,
}

impl IntentProviderFailure {
    fn new(class: IntentProviderFailureClass, detail: impl Into<String>) -> Self {
        Self {
            class,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for IntentProviderFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.class, self.detail)
    }
}

impl IntentProviderFailure {
    /// Render a bounded user-facing action for provider absence/incompatibility (#3367).
    ///
    /// Includes all required fields: legacy-surface framing, required version range,
    /// detected binary (if any), canonical command, support link, and explicit
    /// "no intent evaluation was performed" statement.
    pub(crate) fn bounded_action(&self) -> String {
        let mut out = String::new();
        out.push_str("This is a legacy compatibility surface.\n");
        out.push_str(&format!(
            "Current intent evaluation is owned by cargo-intent (required: version {}, protocol {}).\n",
            INTENT_PROVIDER_REQUIRED_VERSION_RANGE,
            INTENT_PROVIDER_REQUIRED_PROTOCOL
        ));
        match self.class {
            IntentProviderFailureClass::Absent => {
                out.push_str("cargo-intent was not found.\n");
            }
            IntentProviderFailureClass::ForbiddenWorkspaceTarget
            | IntentProviderFailureClass::ForbiddenWorkspaceCrate => {
                out.push_str("cargo-intent was found at a workspace path, which is forbidden.\n");
            }
            IntentProviderFailureClass::WrongProductName => {
                out.push_str(&format!("Detected binary: {}\n", self.detail));
            }
            IntentProviderFailureClass::NotExecutable => {
                out.push_str(&format!(
                    "Binary found but not executable: {}\n",
                    self.detail
                ));
            }
            IntentProviderFailureClass::MalformedConfig => {
                out.push_str(&format!("Delegation config error: {}\n", self.detail));
            }
        }
        out.push_str(&format!(
            "Canonical command: {}\n",
            INTENT_PROVIDER_CANONICAL_COMMAND
        ));
        out.push_str(&format!("Support: {}\n", INTENT_PROVIDER_SUPPORT_REFERENCE));
        out.push_str(
            "No intent evaluation was performed — repository intent posture is NOT confirmed clean.\n",
        );
        out
    }
}

impl std::error::Error for IntentProviderFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentProviderResolution {
    pub executable: PathBuf,
    pub discovery_mode: IntentProviderDiscoveryMode,
    pub executable_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentProviderRequest<'a> {
    pub root: &'a Path,
    pub config_path: Option<&'a Path>,
    pub explicit_executable: Option<&'a Path>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDelegationConfigV1 {
    schema_id: String,
    #[serde(default)]
    executable: Option<String>,
    #[serde(default)]
    delegate_staged_precommit: bool,
    #[serde(default)]
    delegate_spec_system: bool,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentDelegationSettings {
    pub delegate_staged_precommit: bool,
    pub delegate_spec_system: bool,
    pub timeout_secs: u64,
    pub config_path: PathBuf,
}

const DEFAULT_DELEGATION_TIMEOUT_SECS: u64 = 30;

pub fn load_intent_delegation_settings(
    root: &Path,
    explicit_config: Option<&Path>,
) -> Result<Option<IntentDelegationSettings>, IntentProviderFailure> {
    let config_path = explicit_config
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(DEFAULT_INTENT_DELEGATION_CONFIG));
    if !config_path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&config_path).map_err(|err| {
        IntentProviderFailure::new(
            IntentProviderFailureClass::MalformedConfig,
            format!("read {}: {err}", config_path.display()),
        )
    })?;
    let config: IntentDelegationConfigV1 = toml::from_str(&text).map_err(|err| {
        IntentProviderFailure::new(
            IntentProviderFailureClass::MalformedConfig,
            format!("parse {}: {err}", config_path.display()),
        )
    })?;
    if config.schema_id != INTENT_DELEGATION_CONFIG_SCHEMA_ID {
        return Err(IntentProviderFailure::new(
            IntentProviderFailureClass::MalformedConfig,
            format!(
                "unexpected schema_id {} in {}",
                config.schema_id,
                config_path.display()
            ),
        ));
    }
    Ok(Some(IntentDelegationSettings {
        delegate_staged_precommit: config.delegate_staged_precommit,
        delegate_spec_system: config.delegate_spec_system,
        timeout_secs: config
            .timeout_secs
            .unwrap_or(DEFAULT_DELEGATION_TIMEOUT_SECS),
        config_path,
    }))
}

pub fn discover_intent_provider(
    request: &IntentProviderRequest<'_>,
) -> Result<IntentProviderResolution, IntentProviderFailure> {
    if let Some(path) = request.explicit_executable {
        return resolve_candidate(
            request.root,
            path,
            IntentProviderDiscoveryMode::ExplicitEnvironment,
        );
    }
    if let Ok(path) = env::var(INTENT_PROVIDER_ENV_VAR)
        && !path.trim().is_empty()
    {
        return resolve_candidate(
            request.root,
            Path::new(path.trim()),
            IntentProviderDiscoveryMode::ExplicitEnvironment,
        );
    }
    if let Some(path) = read_config_executable(request.root, request.config_path)? {
        return resolve_candidate(
            request.root,
            &path,
            IntentProviderDiscoveryMode::ExplicitConfig,
        );
    }
    if let Some(path) = discover_on_path()? {
        return resolve_candidate(request.root, &path, IntentProviderDiscoveryMode::PathLookup);
    }
    Err(IntentProviderFailure::new(
        IntentProviderFailureClass::Absent,
        "cargo-intent provider not found via environment, compatibility config, or PATH",
    ))
}

fn read_config_executable(
    root: &Path,
    explicit_config: Option<&Path>,
) -> Result<Option<PathBuf>, IntentProviderFailure> {
    let config_path = explicit_config
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(DEFAULT_INTENT_DELEGATION_CONFIG));
    if !config_path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&config_path).map_err(|err| {
        IntentProviderFailure::new(
            IntentProviderFailureClass::MalformedConfig,
            format!("read {}: {err}", config_path.display()),
        )
    })?;
    let config: IntentDelegationConfigV1 = toml::from_str(&text).map_err(|err| {
        IntentProviderFailure::new(
            IntentProviderFailureClass::MalformedConfig,
            format!("parse {}: {err}", config_path.display()),
        )
    })?;
    if config.schema_id != INTENT_DELEGATION_CONFIG_SCHEMA_ID {
        return Err(IntentProviderFailure::new(
            IntentProviderFailureClass::MalformedConfig,
            format!(
                "unexpected schema_id {} in {}",
                config.schema_id,
                config_path.display()
            ),
        ));
    }
    Ok(config
        .executable
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            let path = PathBuf::from(value.trim());
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        }))
}

fn discover_on_path() -> Result<Option<PathBuf>, IntentProviderFailure> {
    let path_var = env::var_os("PATH").ok_or_else(|| {
        IntentProviderFailure::new(
            IntentProviderFailureClass::Absent,
            "PATH is unset; cannot discover cargo-intent",
        )
    })?;
    for dir in env::split_paths(&path_var) {
        for name in candidate_names() {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(Some(candidate));
            }
        }
    }
    Ok(None)
}

fn candidate_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["cargo-intent.exe", "cargo-intent"]
    } else {
        &["cargo-intent"]
    }
}

fn resolve_candidate(
    root: &Path,
    candidate: &Path,
    mode: IntentProviderDiscoveryMode,
) -> Result<IntentProviderResolution, IntentProviderFailure> {
    let executable = fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());
    reject_workspace_leaks(root, &executable)?;
    reject_wrong_product_name(&executable)?;
    if !executable.is_file() {
        return Err(IntentProviderFailure::new(
            IntentProviderFailureClass::NotExecutable,
            format!("provider executable missing: {}", executable.display()),
        ));
    }
    let digest = sha256_v1_bytes(&fs::read(&executable).map_err(|err| {
        IntentProviderFailure::new(
            IntentProviderFailureClass::NotExecutable,
            format!("read {}: {err}", executable.display()),
        )
    })?);
    Ok(IntentProviderResolution {
        executable,
        discovery_mode: mode,
        executable_digest: digest,
    })
}

fn reject_workspace_leaks(root: &Path, executable: &Path) -> Result<(), IntentProviderFailure> {
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let target_root = fs::canonicalize(root.join("target")).unwrap_or_else(|_| root.join("target"));
    let crates_root = fs::canonicalize(root.join("crates")).unwrap_or_else(|_| root.join("crates"));
    if path_within(executable, &target_root) {
        return Err(IntentProviderFailure::new(
            IntentProviderFailureClass::ForbiddenWorkspaceTarget,
            format!(
                "refusing workspace target provider path {}",
                executable.display()
            ),
        ));
    }
    if path_within(executable, &crates_root) {
        return Err(IntentProviderFailure::new(
            IntentProviderFailureClass::ForbiddenWorkspaceCrate,
            format!(
                "refusing workspace crates provider path {}",
                executable.display()
            ),
        ));
    }
    Ok(())
}

fn reject_wrong_product_name(executable: &Path) -> Result<(), IntentProviderFailure> {
    let Some(file_name) = executable.file_name().and_then(OsStr::to_str) else {
        return Err(IntentProviderFailure::new(
            IntentProviderFailureClass::WrongProductName,
            format!("provider path has no file name: {}", executable.display()),
        ));
    };
    let normalized = file_name.strip_suffix(".exe").unwrap_or(file_name);
    if normalized != INTENT_PROVIDER_PRODUCT_NAME {
        return Err(IntentProviderFailure::new(
            IntentProviderFailureClass::WrongProductName,
            format!(
                "expected {INTENT_PROVIDER_PRODUCT_NAME}, got {file_name} at {}",
                executable.display()
            ),
        ));
    }
    Ok(())
}

fn path_within(path: &Path, ancestor: &Path) -> bool {
    path.starts_with(ancestor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cargo-allow-intent-provider-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn write_fake_executable(path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, b"#!/fake\n")
    }

    #[test]
    fn discovers_explicit_request_override() -> std::io::Result<()> {
        let root = temp_root("explicit");
        fs::create_dir_all(&root)?;
        let bin = root.join("bin/cargo-intent");
        write_fake_executable(&bin)?;
        let resolution = discover_intent_provider(&IntentProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: Some(&bin),
        })
        .map_err(|err| std::io::Error::other(err.to_string()))?;
        assert_eq!(
            resolution.discovery_mode,
            IntentProviderDiscoveryMode::ExplicitEnvironment
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn discovers_compatibility_config_executable() -> std::io::Result<()> {
        let root = temp_root("config");
        fs::create_dir_all(&root)?;
        let bin = root.join("install/bin/cargo-intent");
        write_fake_executable(&bin)?;
        let config_dir = root.join(".allow/compatibility");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("intent-delegation.toml"),
            format!(
                r#"schema_id = "{INTENT_DELEGATION_CONFIG_SCHEMA_ID}"
executable = "install/bin/cargo-intent"
"#
            ),
        )?;
        let resolution = discover_intent_provider(&IntentProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: None,
        })
        .map_err(|err| std::io::Error::other(err.to_string()))?;
        assert_eq!(
            resolution.discovery_mode,
            IntentProviderDiscoveryMode::ExplicitConfig
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn rejects_workspace_target_provider() -> std::io::Result<()> {
        let root = temp_root("target-leak");
        fs::create_dir_all(&root)?;
        let bin = root.join("target/debug/cargo-intent");
        write_fake_executable(&bin)?;
        let failure = discover_intent_provider(&IntentProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: Some(&bin),
        })
        .expect_err("workspace target must be rejected");
        assert_eq!(
            failure.class,
            IntentProviderFailureClass::ForbiddenWorkspaceTarget
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn rejects_workspace_crates_provider() -> std::io::Result<()> {
        let root = temp_root("crates-leak");
        fs::create_dir_all(&root)?;
        let bin = root.join("crates/cargo-intent/target/cargo-intent");
        write_fake_executable(&bin)?;
        let failure = discover_intent_provider(&IntentProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: Some(&bin),
        })
        .expect_err("workspace crates path must be rejected");
        assert_eq!(
            failure.class,
            IntentProviderFailureClass::ForbiddenWorkspaceCrate
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn rejects_wrong_product_name() -> std::io::Result<()> {
        let root = temp_root("wrong-product");
        fs::create_dir_all(&root)?;
        let bin = root.join("bin/cargo-allow");
        write_fake_executable(&bin)?;
        let failure = discover_intent_provider(&IntentProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: Some(&bin),
        })
        .expect_err("wrong product must be rejected");
        assert_eq!(failure.class, IntentProviderFailureClass::WrongProductName);
        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}

#[cfg(test)]
mod absence_ux_tests {
    use super::*;

    #[test]
    fn absent_action_includes_all_required_fields() {
        let failure = IntentProviderFailure::new(
            IntentProviderFailureClass::Absent,
            "cargo-intent was not found in PATH",
        );
        let action = failure.bounded_action();
        assert!(
            action.contains("legacy compatibility surface"),
            "missing framing"
        );
        assert!(action.contains("0.1.x"), "missing required version range");
        assert!(
            action.contains("repo.analysis-receipt.v1"),
            "missing required protocol"
        );
        assert!(
            action.contains("cargo-intent was not found"),
            "missing absence detail"
        );
        assert!(
            action.contains("cargo-intent --format json"),
            "missing canonical command"
        );
        assert!(action.contains("SUPPORT_TIERS"), "missing support link");
        assert!(
            action.contains("NOT confirmed clean"),
            "missing no-clean claim"
        );
    }

    #[test]
    fn incompatible_action_does_not_claim_clean() {
        let failure = IntentProviderFailure::new(
            IntentProviderFailureClass::WrongProductName,
            "found 'other-tool' instead of 'cargo-intent'",
        );
        let action = failure.bounded_action();
        assert!(!action.contains("intent is clean"), "must not claim clean");
        assert!(
            !action.contains("no findings"),
            "must not claim no findings"
        );
        assert!(
            action.contains("NOT confirmed clean"),
            "missing no-clean claim"
        );
    }

    #[test]
    fn forbidden_workspace_action_includes_workspace_detail() {
        let failure = IntentProviderFailure::new(
            IntentProviderFailureClass::ForbiddenWorkspaceTarget,
            "target/debug/cargo-intent",
        );
        let action = failure.bounded_action();
        assert!(
            action.contains("workspace path"),
            "missing workspace framing"
        );
        assert!(action.contains("forbidden"), "missing forbidden detail");
    }
}
