//! Parity fixture discovery for repo-edit (#2602-A / #2602-B).

use std::path::{Path, PathBuf};

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("tests/fixtures/repo-edit/parity-mutation-lock-alias-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-path-containment-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-atomic-write-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-apply-receipt-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-init-command-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-refresh-command-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-prune-command-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-apply-backup-mode-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-add-command-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-migrate-command-v1.toml"),
        root.join("tests/fixtures/repo-edit/parity-propose-command-v1.toml"),
    ]
}
