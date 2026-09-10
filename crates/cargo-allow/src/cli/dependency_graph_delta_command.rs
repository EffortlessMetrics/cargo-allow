//! Compile one exact base/head manifest/lockfile pair into the typed
//! dependency graph delta receipt (#3920 PR D producer).
//!
//! Instrument-failure exit contract: unreadable, empty, or malformed
//! required inputs exit nonzero immediately; a well-formed pair always
//! produces a receipt (whose rows may be empty).

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use allow_report::{DependencyGraphDeltaIdentityV1, compile_dependency_graph_delta};
use clap::{Parser, Subcommand};

/// Compile a dependency graph delta receipt (hidden automation
/// tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct DependencyGraphDeltaArgs {
    #[command(subcommand)]
    pub(crate) command: DependencyGraphDeltaSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum DependencyGraphDeltaSubcommand {
    /// Compile one exact base/head pair into the delta receipt.
    #[command(hide = true)]
    Compile(DependencyGraphDeltaCompileArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct DependencyGraphDeltaCompileArgs {
    /// Base-side workspace manifest text (e.g. from the merge base).
    #[arg(long)]
    pub(crate) base_manifest: PathBuf,
    /// Head-side workspace manifest text (e.g. the worktree file).
    #[arg(long)]
    pub(crate) head_manifest: PathBuf,
    /// Base-side lockfile text.
    #[arg(long)]
    pub(crate) base_lock: PathBuf,
    /// Head-side lockfile text.
    #[arg(long)]
    pub(crate) head_lock: PathBuf,
    #[arg(long)]
    pub(crate) base_commit: String,
    #[arg(long)]
    pub(crate) head_commit: String,
    #[arg(long, default_value = "cargo-allow")]
    pub(crate) product: String,
    #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
    pub(crate) target: String,
    /// Write the receipt JSON here instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

fn read_input(path: &PathBuf, what: &str) -> CargoAllowResult<String> {
    let bytes = std::fs::read(path).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{what} read {}: {error}", path.display()),
        )
    })?;
    // Invalid UTF-8 is malformed input: lossy replacement would
    // collapse distinct malformed sources onto one digest.
    let owned = String::from_utf8(bytes).map_err(|_| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{what} {} is not valid UTF-8", path.display()),
        )
    })?;
    let text = owned.replace('\r', "");
    if text.trim().is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "{what} {} is empty; a required input is missing",
                path.display()
            ),
        ));
    }
    Ok(text)
}

pub(super) fn cmd_dependency_graph_delta(args: &DependencyGraphDeltaArgs) -> CargoAllowResult<()> {
    let DependencyGraphDeltaSubcommand::Compile(compile) = &args.command;
    let base_manifest = read_input(&compile.base_manifest, "base manifest")?;
    let head_manifest = read_input(&compile.head_manifest, "head manifest")?;
    let base_lock = read_input(&compile.base_lock, "base lock")?;
    let head_lock = read_input(&compile.head_lock, "head lock")?;
    // The compiler's fixtures may be tolerant, but a producer must
    // validate: malformed documents fail immediately instead of
    // compiling into an apparently-empty complete receipt.
    for (name, text) in [
        ("base manifest", &base_manifest),
        ("head manifest", &head_manifest),
        ("base lock", &base_lock),
        ("head lock", &head_lock),
    ] {
        let outcome = if name.contains("lock") {
            allow_report::validate_lock_document(text)
        } else {
            allow_report::validate_manifest_document(text)
        };
        if let Err(reason) = outcome {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("{name}: {reason}"),
            ));
        }
    }

    let identity = DependencyGraphDeltaIdentityV1 {
        base_commit: compile.base_commit.clone(),
        head_commit: compile.head_commit.clone(),
        // Manifest-set digests bind the exact inputs handed to the
        // compiler; CR bytes are stripped so Windows autocrlf
        // checkouts agree with the upstream bytes.
        base_manifest_set_digest: sha256_v1_bytes(base_manifest.as_bytes()),
        head_manifest_set_digest: sha256_v1_bytes(head_manifest.as_bytes()),
        base_lock_digest: sha256_v1_bytes(base_lock.as_bytes()),
        head_lock_digest: sha256_v1_bytes(head_lock.as_bytes()),
        product: compile.product.clone(),
        target: compile.target.clone(),
    };
    let receipt = compile_dependency_graph_delta(
        &identity,
        &base_manifest,
        &head_manifest,
        &base_lock,
        &head_lock,
    )
    .map_err(|reason| CargoAllowError::with_kind(CargoAllowErrorKind::InstrumentFailure, reason))?;
    let json = serde_json::to_string_pretty(&receipt).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("delta receipt serializes: {error}"),
        )
    })?;
    if let Some(output) = &compile.output {
        std::fs::write(output, json).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("receipt write {}: {error}", output.display()),
            )
        })?;
    } else {
        println!("{json}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DependencyGraphDeltaArgs, DependencyGraphDeltaCompileArgs, DependencyGraphDeltaSubcommand,
        cmd_dependency_graph_delta,
    };
    use std::path::PathBuf;

    fn unique_temp(name: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "dep-graph-delta-{name}-{}-{}.json",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ))
    }

    fn write(name: &str, text: &str) -> PathBuf {
        let path = unique_temp(name);
        std::fs::write(&path, text).expect("fixture writes");
        path
    }

    fn compile_args(
        base_manifest: PathBuf,
        head_manifest: PathBuf,
        base_lock: PathBuf,
        head_lock: PathBuf,
        output: Option<PathBuf>,
    ) -> DependencyGraphDeltaArgs {
        DependencyGraphDeltaArgs {
            command: DependencyGraphDeltaSubcommand::Compile(DependencyGraphDeltaCompileArgs {
                base_manifest,
                head_manifest,
                base_lock,
                head_lock,
                base_commit: "aaa111".to_string(),
                head_commit: "bbb222".to_string(),
                product: "cargo-allow".to_string(),
                target: "x86_64-unknown-linux-gnu".to_string(),
                output,
            }),
        }
    }

    #[test]
    fn compile_emits_a_versioned_receipt_for_wellformed_inputs() {
        let base_manifest = write("bm", "[dependencies]\nserde = \"1\"\n");
        let head_manifest = write("hm", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write(
            "bl",
            "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\nsource = \"registry\"\nchecksum = \"a\"\n",
        );
        let head_lock = write(
            "hl",
            "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\nsource = \"registry\"\nchecksum = \"b\"\n",
        );
        let output = unique_temp("out-json");
        let args = compile_args(
            base_manifest.clone(),
            head_manifest.clone(),
            base_lock.clone(),
            head_lock.clone(),
            Some(output.clone()),
        );
        let outcome = cmd_dependency_graph_delta(&args);
        for path in [&base_manifest, &head_manifest, &base_lock, &head_lock] {
            let _ = std::fs::remove_file(path);
        }
        assert!(outcome.is_ok(), "well-formed inputs compile: {outcome:?}");
        let receipt = std::fs::read_to_string(&output).expect("receipt written");
        let _ = std::fs::remove_file(&output);
        assert!(
            receipt.contains("dependency-graph-delta-compiler.v1"),
            "the receipt carries the compiler schema identity"
        );
        assert!(receipt.contains("lock_only_resolution_changed"));
    }

    #[test]
    fn compile_prints_the_receipt_to_stdout_without_an_output_path() {
        // The producer lane's default rendering covers the stdout
        // branch; cargo test captures the output.
        let base_manifest = write("bm-stdout", "[dependencies]\nserde = \"1\"\n");
        let head_manifest = write("hm-stdout", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write(
            "bl-stdout",
            "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\n",
        );
        let head_lock = write(
            "hl-stdout",
            "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\n",
        );
        let args = compile_args(base_manifest, head_manifest, base_lock, head_lock, None);
        let outcome = cmd_dependency_graph_delta(&args);
        assert!(outcome.is_ok(), "the stdout path compiles: {outcome:?}");
    }

    #[test]
    fn compile_fails_closed_on_a_missing_input_file() {
        let base_manifest = unique_temp("missing");
        let head_manifest = write("hm-missing", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write(
            "bl-missing",
            "[[package]]\nname = \"serde\"\nversion = \"1\"\n",
        );
        let head_lock = write(
            "hl-missing",
            "[[package]]\nname = \"serde\"\nversion = \"1\"\n",
        );
        let args = compile_args(base_manifest, head_manifest, base_lock, head_lock, None);
        let outcome = cmd_dependency_graph_delta(&args);
        assert!(outcome.is_err(), "an unreadable input fails immediately");
    }

    #[test]
    fn compile_fails_closed_on_a_malformed_head_lock() {
        let base_manifest = write("bm-hl", "[dependencies]\nserde = \"1\"\n");
        let head_manifest = write("hm-hl", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write("bl-hl", "[[package]]\nname = \"serde\"\nversion = \"1\"\n");
        let head_lock = write("hl-hl", "prose, not a lockfile");
        let args = compile_args(base_manifest, head_manifest, base_lock, head_lock, None);
        let outcome = cmd_dependency_graph_delta(&args);
        assert!(outcome.is_err(), "a malformed head lock fails immediately");
    }

    #[test]
    fn compile_fails_closed_on_empty_required_input() {
        let base_manifest = write("bm-empty", "");
        let head_manifest = write("hm", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write("bl", "[[package]]\nname = \"serde\"\nversion = \"1\"\n");
        let head_lock = write("hl", "[[package]]\nname = \"serde\"\nversion = \"1\"\n");
        let args = compile_args(base_manifest, head_manifest, base_lock, head_lock, None);
        let outcome = cmd_dependency_graph_delta(&args);
        assert!(
            outcome.is_err(),
            "an empty required input fails immediately"
        );
    }

    #[test]
    fn compile_fails_closed_on_malformed_lock() {
        let base_manifest = write("bm2", "[dependencies]\nserde = \"1\"\n");
        let head_manifest = write("hm2", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write("bl2", "not a lockfile, just prose");
        let head_lock = write("hl2", "[[package]]\nname = \"serde\"\nversion = \"1\"\n");
        let args = compile_args(base_manifest, head_manifest, base_lock, head_lock, None);
        let outcome = cmd_dependency_graph_delta(&args);
        assert!(
            outcome.is_err(),
            "a lock without package entries is malformed input"
        );
    }
    #[test]
    fn compile_fails_closed_on_invalid_utf8_input() {
        // Lossy replacement would collapse distinct malformed inputs
        // onto one digest; invalid bytes are rejected outright.
        let base_manifest = unique_temp("bm-utf8");
        std::fs::write(
            &base_manifest,
            b"[dependencies]\nserde = \"1\"\n# \xff\xfe\n",
        )
        .expect("fixture writes");
        let head_manifest = write("hm-utf8", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write(
            "bl-utf8",
            "[[package]]\nname = \"serde\"\nversion = \"1\"\n",
        );
        let head_lock = write(
            "hl-utf8",
            "[[package]]\nname = \"serde\"\nversion = \"1\"\n",
        );
        let args = compile_args(base_manifest, head_manifest, base_lock, head_lock, None);
        let outcome = cmd_dependency_graph_delta(&args);
        assert!(outcome.is_err(), "invalid UTF-8 fails closed");
    }

    #[test]
    fn compile_fails_closed_on_a_malformed_manifest_document() {
        // TOML-shaped but unparseable: the substring era accepted
        // this; strict document validation rejects it.
        let base_manifest = write("bm-malformed", "[[package]]\ninvalid =");
        let head_manifest = write("hm-malformed", "[dependencies]\nserde = \"1\"\n");
        let base_lock = write(
            "bl-malformed",
            "[[package]]\nname = \"serde\"\nversion = \"1\"\n",
        );
        let head_lock = write(
            "hl-malformed",
            "[[package]]\nname = \"serde\"\nversion = \"1\"\n",
        );
        let args = compile_args(base_manifest, head_manifest, base_lock, head_lock, None);
        let outcome = cmd_dependency_graph_delta(&args);
        assert!(outcome.is_err(), "a malformed manifest fails closed");
    }
}
