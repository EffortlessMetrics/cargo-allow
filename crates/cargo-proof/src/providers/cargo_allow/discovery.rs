//! Public process discovery for cargo-allow (#2554).

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::digest::sha256_v1_bytes;
use super::provider_contract::default_cargo_allow_provider_contract;

pub const PROOF_DELEGATION_CONFIG_SCHEMA_ID: &str = "proof.cargo-allow-delegation.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoAllowDiscoveryMode {
    ExplicitEnvironment,
    ExplicitConfig,
    PathLookup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoAllowProviderFailureClass {
    Absent,
    NotExecutable,
    ForbiddenWorkspaceTarget,
    ForbiddenWorkspaceCrate,
    WrongProductName,
    MalformedConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowProviderFailure {
    pub class: CargoAllowProviderFailureClass,
    pub message: String,
}

impl CargoAllowProviderFailure {
    pub fn new(class: CargoAllowProviderFailureClass, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowProviderResolution {
    pub executable: PathBuf,
    pub discovery_mode: CargoAllowDiscoveryMode,
    pub executable_digest: String,
}

#[derive(Debug, Clone)]
pub struct CargoAllowProviderRequest<'a> {
    pub root: &'a Path,
    pub config_path: Option<&'a Path>,
    pub explicit_executable: Option<&'a Path>,
}

#[derive(Debug, Deserialize)]
struct ProofDelegationConfigV1 {
    schema_id: String,
    executable: Option<String>,
}

pub fn discover_cargo_allow_provider(
    request: &CargoAllowProviderRequest<'_>,
) -> Result<CargoAllowProviderResolution, CargoAllowProviderFailure> {
    let contract = default_cargo_allow_provider_contract();
    if let Some(explicit) = request.explicit_executable {
        return resolve_candidate(
            request.root,
            explicit,
            CargoAllowDiscoveryMode::ExplicitEnvironment,
            &contract.product_name,
        );
    }
    if let Ok(value) = env::var(&contract.environment_variable)
        && !value.trim().is_empty()
    {
        return resolve_candidate(
            request.root,
            Path::new(value.trim()),
            CargoAllowDiscoveryMode::ExplicitEnvironment,
            &contract.product_name,
        );
    }
    if let Some(path) = discover_from_config(request.root, request.config_path, &contract)? {
        return resolve_candidate(
            request.root,
            &path,
            CargoAllowDiscoveryMode::ExplicitConfig,
            &contract.product_name,
        );
    }
    if let Some(path) = discover_on_path()? {
        return resolve_candidate(
            request.root,
            &path,
            CargoAllowDiscoveryMode::PathLookup,
            &contract.product_name,
        );
    }
    Err(CargoAllowProviderFailure::new(
        CargoAllowProviderFailureClass::Absent,
        format!(
            "no {} provider discovered via env, config, or PATH",
            contract.product_name
        ),
    ))
}

fn discover_from_config(
    root: &Path,
    config_path: Option<&Path>,
    contract: &crate::provider_contract::CargoAllowProviderContractV1,
) -> Result<Option<PathBuf>, CargoAllowProviderFailure> {
    let path = config_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(&contract.config_relative_path));
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|err| {
        CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::MalformedConfig,
            format!("read {}: {err}", path.display()),
        )
    })?;
    let config: ProofDelegationConfigV1 = toml::from_str(&text).map_err(|err| {
        CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::MalformedConfig,
            format!("parse {}: {err}", path.display()),
        )
    })?;
    if config.schema_id != PROOF_DELEGATION_CONFIG_SCHEMA_ID {
        return Err(CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::MalformedConfig,
            format!(
                "expected schema_id {PROOF_DELEGATION_CONFIG_SCHEMA_ID}, got {} in {}",
                config.schema_id,
                path.display()
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

fn discover_on_path() -> Result<Option<PathBuf>, CargoAllowProviderFailure> {
    let path_var = env::var_os("PATH").ok_or_else(|| {
        CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::Absent,
            "PATH is unset; cannot discover cargo-allow",
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
        &["cargo-allow.exe", "cargo-allow"]
    } else {
        &["cargo-allow"]
    }
}

fn resolve_candidate(
    root: &Path,
    candidate: &Path,
    mode: CargoAllowDiscoveryMode,
    product_name: &str,
) -> Result<CargoAllowProviderResolution, CargoAllowProviderFailure> {
    let executable = fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());
    reject_workspace_leaks(root, &executable)?;
    reject_wrong_product_name(&executable, product_name)?;
    if !executable.is_file() {
        return Err(CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::NotExecutable,
            format!("provider executable missing: {}", executable.display()),
        ));
    }
    let digest = sha256_v1_bytes(&fs::read(&executable).map_err(|err| {
        CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::NotExecutable,
            format!("read {}: {err}", executable.display()),
        )
    })?);
    Ok(CargoAllowProviderResolution {
        executable,
        discovery_mode: mode,
        executable_digest: digest,
    })
}

fn reject_workspace_leaks(root: &Path, executable: &Path) -> Result<(), CargoAllowProviderFailure> {
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let target_root = fs::canonicalize(root.join("target")).unwrap_or_else(|_| root.join("target"));
    let crates_root = fs::canonicalize(root.join("crates")).unwrap_or_else(|_| root.join("crates"));
    if path_within(executable, &target_root) {
        return Err(CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::ForbiddenWorkspaceTarget,
            format!(
                "refusing workspace target provider path {}",
                executable.display()
            ),
        ));
    }
    if path_within(executable, &crates_root) {
        return Err(CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::ForbiddenWorkspaceCrate,
            format!(
                "refusing workspace crates provider path {}",
                executable.display()
            ),
        ));
    }
    Ok(())
}

fn reject_wrong_product_name(
    executable: &Path,
    product_name: &str,
) -> Result<(), CargoAllowProviderFailure> {
    let Some(file_name) = executable.file_name().and_then(OsStr::to_str) else {
        return Err(CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::WrongProductName,
            format!("provider path has no file name: {}", executable.display()),
        ));
    };
    let normalized = file_name.strip_suffix(".exe").unwrap_or(file_name);
    if normalized != product_name {
        return Err(CargoAllowProviderFailure::new(
            CargoAllowProviderFailureClass::WrongProductName,
            format!(
                "expected {product_name}, got {file_name} at {}",
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

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "proof-adapter-cargo-allow-discovery-{label}-{}-{}",
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
        let bin = root.join("bin/cargo-allow");
        write_fake_executable(&bin)?;
        let resolution = discover_cargo_allow_provider(&CargoAllowProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: Some(&bin),
        })
        .map_err(|err| std::io::Error::other(err.message))?;
        assert_eq!(
            resolution.discovery_mode,
            CargoAllowDiscoveryMode::ExplicitEnvironment
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn discovers_compatibility_config_executable() -> std::io::Result<()> {
        let root = temp_root("config");
        fs::create_dir_all(&root)?;
        let bin = root.join("install/bin/cargo-allow");
        write_fake_executable(&bin)?;
        let config_dir = root.join(".allow/compatibility");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("proof-delegation.toml"),
            format!(
                r#"schema_id = "{PROOF_DELEGATION_CONFIG_SCHEMA_ID}"
executable = "install/bin/cargo-allow"
"#
            ),
        )?;
        let resolution = discover_cargo_allow_provider(&CargoAllowProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: None,
        })
        .map_err(|err| std::io::Error::other(err.message))?;
        assert_eq!(
            resolution.discovery_mode,
            CargoAllowDiscoveryMode::ExplicitConfig
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn rejects_workspace_target_provider() -> std::io::Result<()> {
        let root = temp_root("target-leak");
        fs::create_dir_all(&root)?;
        let bin = root.join("target/debug/cargo-allow");
        write_fake_executable(&bin)?;
        let failure = discover_cargo_allow_provider(&CargoAllowProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: Some(&bin),
        })
        .expect_err("workspace target must be rejected");
        assert_eq!(
            failure.class,
            CargoAllowProviderFailureClass::ForbiddenWorkspaceTarget
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn rejects_wrong_product_name() -> std::io::Result<()> {
        let root = temp_root("wrong-product");
        fs::create_dir_all(&root)?;
        let bin = root.join("bin/cargo-intent");
        write_fake_executable(&bin)?;
        let failure = discover_cargo_allow_provider(&CargoAllowProviderRequest {
            root: &root,
            config_path: None,
            explicit_executable: Some(&bin),
        })
        .expect_err("wrong product must be rejected");
        assert_eq!(
            failure.class,
            CargoAllowProviderFailureClass::WrongProductName
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}
