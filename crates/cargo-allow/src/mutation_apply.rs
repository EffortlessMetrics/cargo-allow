//! Cargo-allow's adapter boundary for neutral single-target apply receipts.
//!
//! The adapter keeps product error classification at the cargo-allow edge
//! while exposing the repository-owned receipt produced by `repo-edit`.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use effortless_repo_edit::{ApplyReceiptV1, SingleTargetApplyRequest, apply_single_target};

pub(crate) struct CargoAllowApplyResponse {
    pub receipt: ApplyReceiptV1,
    pub result: CargoAllowResult<()>,
}

/// Apply one cargo-allow mutation target and retain both product and neutral
/// outcomes. The receipt remains filesystem evidence; the projected result
/// remains cargo-allow's command error boundary.
pub(crate) fn apply_target(request: SingleTargetApplyRequest<'_>) -> CargoAllowApplyResponse {
    let response = apply_single_target(request);
    let receipt = response.receipt.clone();
    let result = response.into_result().map(|_| ()).map_err(|error| {
        CargoAllowError::with_kind(CargoAllowErrorKind::Artifact, error.to_string())
    });
    CargoAllowApplyResponse { receipt, result }
}

#[cfg(test)]
mod tests {
    use super::*;
    use effortless_repo_edit::{ApplyOperation, SingleTargetApplyMode, render_apply_receipt_json};
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn adapter_preserves_success_receipt_and_product_result() -> Result<(), String> {
        let root = TempRoot::new("success")?;
        let response = apply_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/allow.toml"),
            contents: "schema_version = 1\n",
            caller_reference: Some("cargo-allow:test"),
            lock_identity: Some("policy/allow.toml".to_string()),
            mode: SingleTargetApplyMode::CreateNewOnly,
        });

        response.result.map_err(|error| error.to_string())?;
        if response.receipt.operation != ApplyOperation::Create || !response.receipt.applied() {
            return Err("successful adapter apply did not preserve receipt outcome".to_string());
        }
        Ok(())
    }

    #[test]
    fn adapter_preserves_failed_receipt_without_private_root() -> Result<(), String> {
        let root = TempRoot::new("failure")?;
        let target = root.path().join("policy/allow.toml");
        fs::create_dir_all(target.parent().ok_or("missing target parent")?)
            .map_err(|error| error.to_string())?;
        fs::write(&target, "existing\n").map_err(|error| error.to_string())?;
        let response = apply_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/allow.toml"),
            contents: "replacement\n",
            caller_reference: Some("cargo-allow:test"),
            lock_identity: Some("policy/allow.toml".to_string()),
            mode: SingleTargetApplyMode::CreateNewOnly,
        });

        if response.result.is_ok() || response.receipt.applied() {
            return Err("existing target was incorrectly reported as applied".to_string());
        }
        let json = render_apply_receipt_json(&response.receipt, "");
        if json.contains(&root.path().to_string_lossy().to_string()) {
            return Err("failed receipt leaked its private repository root".to_string());
        }
        Ok(())
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Result<Self, String> {
            let path = std::env::temp_dir().join(format!(
                "cargo-allow-apply-receipt-{label}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
