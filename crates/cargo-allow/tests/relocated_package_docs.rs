//! Relocated package-docs proof (#3852).
//!
//! Two modes:
//!
//! - Default (no env gate): offline characterization. Pins the
//!   `cargo-allow.relocated-package-docs.v1` schema id, validates the
//!   committed example receipt through the typed model, and asserts the
//!   required negative postures are represented. CI-safe: needs no
//!   harness artifacts.
//! - Decisive (env-gated): with `CARGO_ALLOW_RELOCATED_PACKAGE_DOCS_ROOT`
//!   set (plus `CARGO_ALLOW_RELOCATED_PACKAGES`, a directory holding the
//!   exact final `.crate` files; `CARGO_ALLOW_RELOCATED_SURFACE`, the
//!   consumed final-packaged-surface (#3851) receipt; and
//!   `CARGO_ALLOW_RELOCATED_REGISTRY`, a classic Cargo local-registry
//!   directory serving the full lockfile graph), executes the per-package
//!   relocated steps, emits the receipt JSON via the typed model into
//!   `<ROOT>/relocated-package-docs.receipt.json`, and prints per-package
//!   rows under `--nocapture`.
//!
//! Exit-code contract: the decisive run always writes the receipt when the
//! inputs are mechanically usable. Aggregate `Incomplete` rows still write
//! the receipt and still pass; only mechanical breakage (missing inputs,
//! unpack failure, cargo itself not executable) fails the test as
//! `InstrumentFailure`.

use allow_report::{
    CargoAllowRelocatedPackageDocsReceiptV1, RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1,
    RELOCATED_PACKAGE_DOCS_RECEIPT_SCHEMA_V1, RelocatedPackageDocsBasisV1,
    RelocatedPackageDocsNegativeV1, RelocatedPackageDocsReferenceProjectionV1,
    RelocatedPackageDocsResultV1, RelocatedPackageDocsRowV1,
};

const SCHEMA_ID: &str = RELOCATED_PACKAGE_DOCS_RECEIPT_SCHEMA_V1;
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/relocated-package-docs-pass.example.json");
const SCHEMA_DOC: &str =
    include_str!("../../../docs/dogfood/fixtures/release/relocated-package-docs.v1.schema.json");

#[test]
fn example_relocated_package_docs_matches_schema_constants() {
    assert!(
        SCHEMA_DOC.contains(SCHEMA_ID),
        "schema fixture must pin {SCHEMA_ID}"
    );
    // Schema constraints must not diverge from the typed model unnoticed:
    // compile the committed schema fixture and validate the example.
    let schema: serde_json::Value = serde_json::from_str(SCHEMA_DOC)
        .unwrap_or_else(|err| std::panic::panic_any(format!("schema fixture json: {err}")));
    let validator = jsonschema::validator_for(&schema)
        .unwrap_or_else(|err| std::panic::panic_any(format!("schema fixture invalid: {err}")));
    let example_value: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| std::panic::panic_any(format!("example receipt json: {err}")));
    assert!(
        validator.validate(&example_value).is_ok(),
        "example receipt must validate against the committed schema fixture"
    );
    let example: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| std::panic::panic_any(format!("example receipt json: {err}")));
    assert_eq!(
        example.get("schema").and_then(serde_json::Value::as_str),
        Some(SCHEMA_ID)
    );
    let rows = example
        .get("rows")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("rows missing"));
    assert_eq!(rows.len(), RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1.len());
    for (idx, name) in RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1.iter().enumerate() {
        assert_eq!(
            rows.get(idx)
                .and_then(|row| row.get("name"))
                .and_then(serde_json::Value::as_str),
            Some(*name),
            "row {idx} must be {name} in release order"
        );
    }
    for row in rows {
        assert_eq!(
            row.get("result").and_then(serde_json::Value::as_str),
            Some("complete"),
            "example row must be complete"
        );
        assert!(
            row.get("version")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|version| !version.contains('-')),
            "rc-line inputs are rejected as final identity"
        );
        assert_eq!(
            row.get("doc_posture").and_then(serde_json::Value::as_str),
            Some("clean"),
            "example rows must record clean rustdoc"
        );
        let examples_posture = row
            .get("examples_posture")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| std::panic::panic_any("examples_posture missing"));
        assert!(
            examples_posture == "examples_built" || examples_posture == "NoExampleSelected",
            "NoExampleSelected is an explicit posture, never silent"
        );
    }
    // The typed model must parse and validate the committed example.
    let typed: CargoAllowRelocatedPackageDocsReceiptV1 = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| std::panic::panic_any(format!("example receipt typed parse: {err}")));
    typed
        .validate()
        .unwrap_or_else(|err| std::panic::panic_any(format!("example receipt invalid: {err}")));
    assert_eq!(
        typed.aggregate.result,
        RelocatedPackageDocsResultV1::Complete
    );
    // Required negative postures are represented, not silently skipped.
    let negatives = example
        .get("negative_controls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("negative_controls missing"));
    let ids: Vec<&str> = negatives
        .iter()
        .filter_map(|v| v.get("id").and_then(serde_json::Value::as_str))
        .collect();
    for required in [
        "stale_consumed_digest_rejected",
        "missing_declared_asset_incomplete",
        "rc_line_inputs_rejected_as_final",
        "workspace_path_success_impossible_by_construction",
        "no_example_selected_explicit",
    ] {
        assert!(
            ids.contains(&required),
            "example receipt missing negative control {required}"
        );
    }
    let limitations = example
        .get("limitations")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("limitations missing"));
    assert!(
        limitations
            .iter()
            .any(|v| v.as_str() == Some("fetch_warm_may_use_crates_io")),
        "example must record fetch-warm network limitation"
    );
    assert!(
        example
            .get("claim_boundary")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|boundary| boundary.contains("does not prove")),
        "claim boundary must state what is not proven"
    );
}

// ---------------------------------------------------------------------------
// Decisive env-gated runner.
// ---------------------------------------------------------------------------

/// Checked role/limitation markers per package: (haystack_file, role_marker,
/// limitation_marker). Markers were observed in the exact final `.crate`
/// bytes; the runner fails a row (Incomplete, never fabricated) when a
/// marker is absent.
fn expected_markers(name: &str) -> (&'static str, &'static str, &'static str) {
    match name {
        "allow-core" => (
            "src/lib.rs",
            "Core data model",
            "It does not scan source files",
        ),
        "allow-policy" => (
            "src/lib.rs",
            "Canonical policy loading",
            "without executing linked",
        ),
        "allow-inventory" => (
            "src/lib.rs",
            "Source-tree root discovery and file inventory",
            "does not call `cargo metadata`",
        ),
        "allow-files" => (
            "src/lib.rs",
            "File-surface scanners",
            "rather than required build metadata",
        ),
        "allow-rust" => (
            "src/lib.rs",
            "Source-syntax Rust scanners",
            "It parses source directly without invoking Cargo",
        ),
        "allow-match" => (
            "src/lib.rs",
            "Structural finding-to-policy matching",
            "It only reasons over source-syntax findings",
        ),
        "allow-report" => (
            "src/lib.rs",
            "Human and machine artifact rendering",
            "they do not perform scanning or validation",
        ),
        "allow-policy-legacy" => (
            "src/lib.rs",
            "Legacy policy adapters for cargo-allow migrations",
            "it does not execute legacy xtasks",
        ),
        "allow-diff" => (
            "src/lib.rs",
            "PR-posture and policy-diff helpers",
            "it does not invoke Cargo metadata",
        ),
        "cargo-allow" => (
            "README.md",
            "This package builds the `cargo-allow` command-line interface",
            "without executing project code",
        ),
        _ => std::panic::panic_any(format!("unexpected package {name}")),
    }
}

fn expected_features(name: &str) -> (Vec<&'static str>, &'static str, bool) {
    match name {
        // The packaged README documents the feature-disabled configuration,
        // so the matrix carries the extra --no-default-features check.
        "allow-files" => (vec!["changie"], "documented_optional_changie", true),
        "allow-rust" => (vec!["default", "syntax"], "default_features_only", false),
        _ => (vec![], "no_features", false),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex_nibble(Sha256::digest(bytes).as_slice())
}

fn hex_nibble(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('?'));
        out.push(char::from_digit((byte & 0xf) as u32, 16).unwrap_or('?'));
    }
    out
}

struct CargoRun {
    exit: i32,
    output: String,
}

fn run_cargo(
    cargo_bin: &str,
    args: &[&str],
    cwd: &std::path::Path,
    cargo_home: &std::path::Path,
    target_dir: &std::path::Path,
    log_path: &std::path::Path,
) -> CargoRun {
    let output = std::process::Command::new(cargo_bin)
        .args(args)
        .current_dir(cwd)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("InstrumentFailure: cargo not executable: {err}"))
        });
    let exit = output.status.code().unwrap_or(-1);
    let text = format!(
        "$ {cargo_bin} {}\ncwd={}\nexit={exit}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        args.join(" "),
        cwd.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(log_path, &text).unwrap_or_else(|err| {
        std::panic::panic_any(format!("InstrumentFailure: log write failed: {err}"))
    });
    CargoRun { exit, output: text }
}

fn count_leading_token(text: &str, token: &str) -> u32 {
    text.lines().filter(|line| line.starts_with(token)).count() as u32
}

/// Unpack one exact `.crate` into `dest_dir`, always removing any prior
/// extraction first: same-version/different-bytes archives must never run
/// stale files while the receipt binds the new digest.
fn fresh_unpack(crate_path: &std::path::Path, dest_dir: &std::path::Path, label: &str) {
    if dest_dir.exists() {
        std::fs::remove_dir_all(dest_dir).unwrap_or_else(|err| {
            std::panic::panic_any(format!("InstrumentFailure: unpack clear failed: {err}"))
        });
    }
    std::fs::create_dir_all(dest_dir).unwrap_or_else(|err| {
        std::panic::panic_any(format!("InstrumentFailure: unpack mkdir: {err}"))
    });
    let status = std::process::Command::new("tar")
        .args(["xzf"])
        .arg(crate_path)
        .args(["-C"])
        .arg(dest_dir)
        .status()
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("InstrumentFailure: tar missing: {err}"))
        });
    assert!(
        status.success(),
        "InstrumentFailure: tar unpack failed for {label}"
    );
}

#[test]
fn decisive_relocated_package_docs_from_final_crates() {
    let root = match std::env::var("CARGO_ALLOW_RELOCATED_PACKAGE_DOCS_ROOT") {
        Ok(value) if !value.trim().is_empty() => std::path::PathBuf::from(value),
        _ => return,
    };
    let packages_dir = std::env::var("CARGO_ALLOW_RELOCATED_PACKAGES").unwrap_or_else(|_| {
        std::panic::panic_any(
            "InstrumentFailure: CARGO_ALLOW_RELOCATED_PACKAGES must point at the exact .crate dir",
        )
    });
    let surface_path = std::env::var("CARGO_ALLOW_RELOCATED_SURFACE").unwrap_or_else(|_| {
        std::panic::panic_any(
            "InstrumentFailure: CARGO_ALLOW_RELOCATED_SURFACE must point at the #3851 receipt",
        )
    });
    let registry_dir = std::env::var("CARGO_ALLOW_RELOCATED_REGISTRY").unwrap_or_else(|_| {
        std::panic::panic_any(
            "InstrumentFailure: CARGO_ALLOW_RELOCATED_REGISTRY must point at the local-registry",
        )
    });
    let cargo_bin =
        std::env::var("CARGO_ALLOW_RELOCATED_CARGO").unwrap_or_else(|_| "cargo".to_string());
    let channel_dir = std::env::var("CARGO_ALLOW_RELOCATED_CHANNEL_SOURCE")
        .ok()
        .map_or_else(
            || {
                // Default: the worktree root enclosing this test's crate.
                let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                manifest
                    .ancestors()
                    .nth(2)
                    .unwrap_or_else(|| {
                        std::panic::panic_any("InstrumentFailure: cannot locate worktree root")
                    })
                    .to_path_buf()
            },
            std::path::PathBuf::from,
        );

    if registry_dir.contains('\'') {
        std::panic::panic_any(
            "InstrumentFailure: CARGO_ALLOW_RELOCATED_REGISTRY must not contain a single quote (TOML literal string)",
        );
    }
    let packages_dir = std::path::PathBuf::from(packages_dir);
    let surface: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&surface_path).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "InstrumentFailure: surface receipt unreadable: {err}"
            ))
        }),
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("InstrumentFailure: surface receipt json: {err}"))
    });
    let surface_rows = surface
        .pointer("/package_set/packages")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("InstrumentFailure: surface rows missing"));
    let surface_by_name = |name: &str| {
        surface_rows
            .iter()
            .find(|row| row.get("name").and_then(serde_json::Value::as_str) == Some(name))
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("InstrumentFailure: surface row {name} missing"))
            })
            .clone()
    };
    let surface_digest = sha256_hex(
        std::fs::read(&surface_path)
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("InstrumentFailure: surface re-read: {err}"))
            })
            .as_slice(),
    );
    // Fail closed: the ExactCandidatePackageSetV1 receipt must sit next to
    // the consumed surface receipt, parse, and report Passed. No sentinel.
    let candidate_path = std::path::PathBuf::from(&surface_path)
        .parent()
        .unwrap_or_else(|| {
            std::panic::panic_any("InstrumentFailure: surface receipt path has no parent dir")
        })
        .join("exact-candidate-package-set.receipt.json");
    let candidate_bytes = std::fs::read(&candidate_path).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "InstrumentFailure: candidate receipt unreadable: {err}"
        ))
    });
    let candidate_receipt: serde_json::Value = serde_json::from_slice(&candidate_bytes)
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("InstrumentFailure: candidate receipt json: {err}"))
        });
    assert!(
        candidate_receipt
            .get("schema_id")
            .and_then(serde_json::Value::as_str)
            == Some("cargo-allow.exact-candidate-package-set.v1"),
        "InstrumentFailure: candidate receipt is not ExactCandidatePackageSetV1"
    );
    assert!(
        candidate_receipt
            .get("result")
            .and_then(serde_json::Value::as_str)
            == Some("Passed"),
        "InstrumentFailure: candidate receipt did not Pass"
    );
    let candidate_digest = sha256_hex(&candidate_bytes);
    let git_head = surface
        .pointer("/candidate/git_head")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unrecorded");

    // Channel/support projection from the one canonical repo source.
    let support_matrix = std::fs::read(channel_dir.join("docs/support-matrix.toml"))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "InstrumentFailure: support-matrix unreadable: {err}"
            ))
        });
    let getting_started = std::fs::read(channel_dir.join("docs/getting-started.md"))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "InstrumentFailure: getting-started unreadable: {err}"
            ))
        });
    let getting_started_text = String::from_utf8_lossy(&getting_started);
    for identity in ["0.1.11", "0.2.0-rc.1", "0.2.0"] {
        assert!(
            getting_started_text.contains(identity),
            "InstrumentFailure: getting-started lost the {identity} channel identity"
        );
    }
    let mut channel_bytes = support_matrix.clone();
    channel_bytes.extend_from_slice(&getting_started);
    let channel_digest = sha256_hex(&channel_bytes);
    let toolchain = std::process::Command::new(&cargo_bin)
        .arg("--version")
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .map_or_else(
            |_| "unrecorded".to_string(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_string(),
        );

    let relocated_dir = root.join("relocated");
    let cargo_home = root.join("cargo-home");
    let target_dir = root.join("target");
    let logs_dir = root.join("relocated-logs");
    for dir in [&relocated_dir, &cargo_home, &target_dir, &logs_dir] {
        std::fs::create_dir_all(dir).unwrap_or_else(|err| {
            std::panic::panic_any(format!("InstrumentFailure: mkdir failed: {err}"))
        });
    }
    std::fs::write(
        cargo_home.join("config.toml"),
        format!(
            "# Decisive relocated runs (#3852): crates.io served from the local registry.\n\
             [source.crates-io]\nreplace-with = \"candidate-local-registry\"\n\n\
             [source.candidate-local-registry]\nlocal-registry = '{}'\n",
            registry_dir.as_str(),
        ),
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "InstrumentFailure: cargo-home config failed: {err}"
        ))
    });

    // The product-level support reference projection ships only in the
    // packaged cargo-allow manifest; read it once and project it per row.
    // Every row still carries the channel digest computed from the canonical
    // repo source above.
    let cli_surface = surface_by_name("cargo-allow");
    let cli_version = cli_surface
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("0.2.0");
    let cli_sha = cli_surface
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let cli_bytes = std::fs::read(packages_dir.join(format!("cargo-allow-{cli_version}.crate")))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "InstrumentFailure: cargo-allow .crate unreadable: {err}"
            ))
        });
    assert!(
        sha256_hex(&cli_bytes) == cli_sha,
        "InstrumentFailure: cargo-allow archive disagrees with the #3851 surface row"
    );
    let cli_unpack = relocated_dir.join(format!("cargo-allow-{cli_version}"));
    fresh_unpack(
        &packages_dir.join(format!("cargo-allow-{cli_version}.crate")),
        &cli_unpack,
        "cargo-allow",
    );
    let cli_manifest =
        std::fs::read_to_string(cli_unpack.join(format!("cargo-allow-{cli_version}/Cargo.toml")))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("InstrumentFailure: cli manifest read: {err}"))
            });
    let (product_published, product_candidate) =
        packaged_reference(&cli_manifest).unwrap_or_default();
    let reference_ok = product_published == "0.1.11" && product_candidate == "0.2.0";

    let mut rows: Vec<RelocatedPackageDocsRowV1> = Vec::new();
    for name in RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1 {
        let surface_row = surface_by_name(name);
        let version = surface_row
            .get("version")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("0.2.0");
        let expected_sha = surface_row
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let expected_size = surface_row
            .get("size_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let expected_manifest = surface_row
            .pointer("/manifest/sha256")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let expected_files = surface_row
            .pointer("/file_list/sha256")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let crate_path = packages_dir.join(format!("{name}-{version}.crate"));
        let crate_bytes = std::fs::read(&crate_path).unwrap_or_else(|_| {
            rows.push(incomplete_row(
                name,
                version,
                &channel_digest,
                "exact .crate file missing from packages dir",
            ));
            println!("row {name}: Incomplete (crate file missing)");
            Vec::new()
        });
        if crate_bytes.is_empty() && rows.last().is_some_and(|row| row.name == *name) {
            continue;
        }
        let actual_sha = sha256_hex(&crate_bytes);
        if actual_sha != expected_sha {
            let mut row = incomplete_row(
                name,
                version,
                &channel_digest,
                "archive sha256 disagrees with the #3851 surface row",
            );
            row.crate_sha256.clone_from(&actual_sha);
            row.result = RelocatedPackageDocsResultV1::Mismatch;
            rows.push(row);
            println!("row {name}: Mismatch (sha256)");
            continue;
        }
        // Unpack outside the repository: workspace-path-dependent success is
        // impossible by construction for everything below.
        let unpack_root = relocated_dir.join(format!("{name}-{version}"));
        fresh_unpack(&crate_path, &unpack_root, name);
        let pkg_root = unpack_root.join(format!("{name}-{version}"));
        assert!(
            pkg_root.join("Cargo.toml").is_file(),
            "InstrumentFailure: unpacked {name} lacks Cargo.toml"
        );
        let manifest_text =
            std::fs::read_to_string(pkg_root.join("Cargo.toml")).unwrap_or_else(|err| {
                std::panic::panic_any(format!("InstrumentFailure: manifest read: {err}"))
            });
        // Declared assets come from the consumed surface row, never from
        // hardcoded filenames.
        let readme_decl_path = surface_row
            .pointer("/assets/readme/path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let readme_decl_sha = surface_row
            .pointer("/assets/readme/sha256")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let license_decl_path = surface_row
            .pointer("/assets/license/path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let license_decl_sha = surface_row
            .pointer("/assets/license/sha256")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let readme_bytes = std::fs::read(unpack_root.join(readme_decl_path)).unwrap_or_default();
        let license_bytes = std::fs::read(unpack_root.join(license_decl_path)).unwrap_or_default();
        let readme_text = String::from_utf8_lossy(&readme_bytes).to_string();
        let (marker_file, role_marker, limitation_marker) = expected_markers(name);
        let haystack = std::fs::read_to_string(pkg_root.join(marker_file)).unwrap_or_default();
        let role_ok = haystack.contains(role_marker);
        let limit_ok = haystack.contains(limitation_marker)
            || (marker_file == "README.md" && readme_text.contains(limitation_marker));
        let doc_scope = format!("{haystack}\n{readme_text}");
        let sibling_ok = !doc_scope.contains("cargo-intent") && !doc_scope.contains("cargo-proof");
        let (expected_feats, features_posture, wants_no_default) = expected_features(name);
        let features: Vec<String> = parse_features(&manifest_text);
        let features_ok = expected_feats
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            == features;

        let check = run_cargo(
            &cargo_bin,
            &["check", "--locked", "--offline"],
            &pkg_root,
            &cargo_home,
            &target_dir,
            &logs_dir.join(format!("{name}.check.log")),
        );
        let doc = run_cargo(
            &cargo_bin,
            &["doc", "--no-deps", "--locked", "--offline"],
            &pkg_root,
            &cargo_home,
            &target_dir,
            &logs_dir.join(format!("{name}.doc.log")),
        );
        let doctest = run_cargo(
            &cargo_bin,
            &["test", "--doc", "--locked", "--offline"],
            &pkg_root,
            &cargo_home,
            &target_dir,
            &logs_dir.join(format!("{name}.doctest.log")),
        );
        let doctests_passed = doctest_passed_count(&doctest.output);
        let doctest_posture = if doctests_passed > 0 {
            "has_doctests"
        } else {
            "no_public_doctests"
        };
        let has_examples = pkg_root.join("examples").is_dir();
        let (examples_posture, examples_exit) = if has_examples {
            let built = run_cargo(
                &cargo_bin,
                &["build", "--examples", "--locked", "--offline"],
                &pkg_root,
                &cargo_home,
                &target_dir,
                &logs_dir.join(format!("{name}.examples.log")),
            );
            (
                if built.exit == 0 {
                    "examples_built"
                } else {
                    "examples_failed"
                }
                .to_string(),
                Some(built.exit),
            )
        } else {
            ("NoExampleSelected".to_string(), None)
        };
        let no_default_check_exit = if wants_no_default {
            let checked = run_cargo(
                &cargo_bin,
                &["check", "--no-default-features", "--locked", "--offline"],
                &pkg_root,
                &cargo_home,
                &target_dir,
                &logs_dir.join(format!("{name}.no-default.log")),
            );
            Some(checked.exit)
        } else {
            None
        };

        let readme_present =
            !readme_decl_path.is_empty() && unpack_root.join(readme_decl_path).is_file();
        let license_present =
            !license_decl_path.is_empty() && unpack_root.join(license_decl_path).is_file();
        let external_repo_refs = external_refs(&readme_text, &channel_dir);
        let mut row_limitations: Vec<String> = Vec::new();
        if !readme_present {
            row_limitations.push(format!(
                "declared readme asset {readme_decl_path:?} absent from unpacked root"
            ));
        } else if sha256_hex(&readme_bytes) != readme_decl_sha {
            row_limitations.push(format!(
                "declared readme asset {readme_decl_path:?} bytes disagree with surface digest {readme_decl_sha}"
            ));
        }
        if !license_present {
            row_limitations.push(format!(
                "declared license asset {license_decl_path:?} absent from unpacked root"
            ));
        } else if sha256_hex(&license_bytes) != license_decl_sha {
            row_limitations.push(format!(
                "declared license asset {license_decl_path:?} bytes disagree with surface digest {license_decl_sha}"
            ));
        }
        if !role_ok {
            row_limitations.push("role marker absent in packaged docs".to_string());
        }
        if !limit_ok {
            row_limitations.push("limitation marker absent in packaged docs".to_string());
        }
        if !sibling_ok {
            row_limitations.push("sibling product named in user-facing docs".to_string());
        }
        if !features_ok {
            row_limitations.push("packaged feature set differs from expected".to_string());
        }
        if !reference_ok {
            row_limitations
                .push("product reference projection drifts from 0.1.11/0.2.0".to_string());
        }
        let published = product_published.clone();
        let candidate = product_candidate.clone();
        let mechanical_ok = check.exit == 0
            && doc.exit == 0
            && count_leading_token(&doc.output, "warning") == 0
            && (doctest.exit == 0 || doctest.output.contains("no library targets found"))
            && examples_exit.is_none_or(|exit| exit == 0)
            && no_default_check_exit.is_none_or(|exit| exit == 0);
        let result = if mechanical_ok && row_limitations.is_empty() {
            RelocatedPackageDocsResultV1::Complete
        } else {
            RelocatedPackageDocsResultV1::Incomplete
        };
        println!(
            "row {name}: {} (check={} doc={} doctest={} passed={} examples={})",
            match result {
                RelocatedPackageDocsResultV1::Complete => "Complete",
                _ => "Incomplete",
            },
            check.exit,
            doc.exit,
            doctest.exit,
            doctests_passed,
            examples_posture
        );
        rows.push(RelocatedPackageDocsRowV1 {
            name: (*name).to_string(),
            version: version.to_string(),
            crate_sha256: actual_sha,
            crate_size_bytes: expected_size,
            manifest_digest: expected_manifest.to_string(),
            file_list_digest: expected_files.to_string(),
            readme_present,
            readme_sha256: if readme_present {
                Some(sha256_hex(&readme_bytes))
            } else {
                None
            },
            license_assets_present: license_present,
            external_repo_refs,
            reference_projection: RelocatedPackageDocsReferenceProjectionV1 {
                published_version: published,
                candidate_version: candidate,
                channel_digest: channel_digest.clone(),
            },
            check_exit: check.exit,
            check_warnings: count_leading_token(&check.output, "warning"),
            doc_exit: doc.exit,
            doc_warnings: count_leading_token(&doc.output, "warning"),
            doc_posture: if doc.exit == 0 && count_leading_token(&doc.output, "warning") == 0 {
                "clean".to_string()
            } else {
                "unclean".to_string()
            },
            doctest_exit: doctest.exit,
            doctests_passed,
            doctest_posture: doctest_posture.to_string(),
            examples_posture,
            examples_exit,
            features,
            features_posture: features_posture.to_string(),
            no_default_check_exit,
            role_marker: role_marker.to_string(),
            limitation_marker: limitation_marker.to_string(),
            sibling_products_separate: sibling_ok,
            result,
            limitations: row_limitations,
        });
    }

    let mut receipt = CargoAllowRelocatedPackageDocsReceiptV1::new(RelocatedPackageDocsBasisV1 {
        exact_candidate_receipt_digest: candidate_digest,
        packaged_surface_digest: surface_digest,
        git_head: git_head.to_string(),
        toolchain,
        network_posture: "fetch_warm_may_use_crates_io".to_string(),
        isolation: "local_registry_offline".to_string(),
    });
    receipt.rows = rows;
    receipt.negative_controls = vec![
        RelocatedPackageDocsNegativeV1 {
            id: "stale_consumed_digest_rejected".to_string(),
            result_class: "Stale".to_string(),
            detail: "a receipt whose consumed input digests no longer match the current bytes is Stale, never current; rows reconcile archive sha256 against the #3851 surface row and record Mismatch on drift".to_string(),
        },
        RelocatedPackageDocsNegativeV1 {
            id: "missing_declared_asset_incomplete".to_string(),
            result_class: "Incomplete".to_string(),
            detail: "a missing declared README or license asset makes the row Incomplete with the exact reason in limitations".to_string(),
        },
        RelocatedPackageDocsNegativeV1 {
            id: "rc_line_inputs_rejected_as_final".to_string(),
            result_class: "Mismatch".to_string(),
            detail: "any row version containing a prerelease '-' segment is rejected as final identity, mirroring the rc-exclusion law of scripts/final-package-docs.py".to_string(),
        },
        RelocatedPackageDocsNegativeV1 {
            id: "workspace_path_success_impossible_by_construction".to_string(),
            result_class: "CheckoutIsolated".to_string(),
            detail: "every run executes from an unpacked .crate root outside the repository with crates.io replaced by the local registry; no workspace path is reachable".to_string(),
        },
        RelocatedPackageDocsNegativeV1 {
            id: "no_example_selected_explicit".to_string(),
            result_class: "Complete".to_string(),
            detail: "packages shipping no examples/ record the explicit NoExampleSelected posture, never a silent pass".to_string(),
        },
    ];
    receipt.limitations = vec![
        "fetch_warm_may_use_crates_io".to_string(),
        "relocated docs.rs rendering is not executed; rustdoc builds relocated instead".to_string(),
        "install-channel wording stays governed by the getting-started contract tests".to_string(),
        "deeper sibling-product mentions in spec/test modules (allow-policy support_tiers.rs, allow-inventory inventory tests) frame cargo-intent/cargo-proof as separate experimental products; user-facing docs were marker-checked".to_string(),
    ];
    receipt.refresh_aggregate();
    // Aggregate Incomplete rows still write the receipt and still pass: only
    // mechanical breakage fails this test.
    receipt
        .validate()
        .unwrap_or_else(|err| std::panic::panic_any(format!("receipt invalid: {err}")));
    let out_path = root.join("relocated-package-docs.receipt.json");
    std::fs::write(
        &out_path,
        serde_json::to_string_pretty(&receipt).unwrap_or_else(|err| {
            std::panic::panic_any(format!("InstrumentFailure: receipt render: {err}"))
        }),
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("InstrumentFailure: receipt write: {err}"))
    });
    println!("receipt: {}", out_path.display());
}

fn incomplete_row(
    name: &str,
    version: &str,
    channel_digest: &str,
    reason: &str,
) -> RelocatedPackageDocsRowV1 {
    RelocatedPackageDocsRowV1 {
        name: name.to_string(),
        version: version.to_string(),
        crate_sha256: String::new(),
        crate_size_bytes: 0,
        manifest_digest: String::new(),
        file_list_digest: String::new(),
        readme_present: false,
        readme_sha256: None,
        license_assets_present: false,
        external_repo_refs: Vec::new(),
        reference_projection: RelocatedPackageDocsReferenceProjectionV1 {
            published_version: String::new(),
            candidate_version: String::new(),
            channel_digest: channel_digest.to_string(),
        },
        check_exit: -1,
        check_warnings: 0,
        doc_exit: -1,
        doc_warnings: 0,
        doc_posture: "unrun".to_string(),
        doctest_exit: -1,
        doctests_passed: 0,
        doctest_posture: "unrun".to_string(),
        examples_posture: "unrun".to_string(),
        examples_exit: None,
        features: Vec::new(),
        features_posture: "unrun".to_string(),
        no_default_check_exit: None,
        role_marker: String::new(),
        limitation_marker: String::new(),
        sibling_products_separate: false,
        result: RelocatedPackageDocsResultV1::Incomplete,
        limitations: vec![reason.to_string()],
    }
}

/// Parse the `[features]` key set from a packaged manifest.
fn parse_features(manifest: &str) -> Vec<String> {
    let mut in_features = false;
    let mut features: Vec<String> = Vec::new();
    for line in manifest.lines() {
        let stripped = line.trim();
        if stripped.starts_with('[') {
            in_features = stripped == "[features]";
            continue;
        }
        if in_features
            && !stripped.is_empty()
            && !stripped.starts_with('#')
            && let Some((key, _)) = stripped.split_once('=')
        {
            features.push(key.trim().to_string());
        }
    }
    features.sort();
    features
}

/// Count passed doctests across `test result: ok. N passed` lines.
fn doctest_passed_count(output: &str) -> u32 {
    output
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("test result: ok. ")?;
            let count = rest.split_whitespace().next()?;
            count.parse::<u32>().ok()
        })
        .sum()
}

/// Repo-relative prose references (`docs/...`) in README text that resolve
/// to a real file in the canonical repo source: bounded external-repo
/// postures. Illustrative `docs/` strings that resolve to no repo file
/// (classifier examples) are not references and are dropped, never passed.
fn external_refs(readme: &str, channel_dir: &std::path::Path) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    for token in readme
        .split(|char: char| char.is_whitespace() || "`\"'()[]".contains(char))
        .map(str::trim)
        .filter(|token| token.starts_with("docs/"))
    {
        let cleaned = token.trim_end_matches(['.', ',', ';', ':']);
        if !refs.iter().any(|seen| seen == cleaned) && channel_dir.join(cleaned).is_file() {
            refs.push(cleaned.to_string());
        }
    }
    refs.sort();
    refs
}

/// Read the packaged `[package.metadata.cargo-allow.reference]` projection.
fn packaged_reference(manifest: &str) -> Option<(String, String)> {
    let mut published: Option<String> = None;
    let mut candidate: Option<String> = None;
    let mut in_reference = false;
    for line in manifest.lines() {
        let stripped = line.trim();
        if stripped.starts_with('[') {
            in_reference = stripped == "[package.metadata.cargo-allow.reference]";
            continue;
        }
        if in_reference {
            if let Some(value) = stripped
                .strip_prefix("published_version")
                .and_then(|rest| rest.split('=').nth(1))
            {
                published = Some(value.trim().trim_matches('"').to_string());
            }
            if let Some(value) = stripped
                .strip_prefix("candidate_version")
                .and_then(|rest| rest.split('=').nth(1))
            {
                candidate = Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    match (published, candidate) {
        (Some(published), Some(candidate)) => Some((published, candidate)),
        _ => None,
    }
}
