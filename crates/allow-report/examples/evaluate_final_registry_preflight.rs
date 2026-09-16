//! JSON plumbing for the #3850 external observation adapter.
//!
//! The credential-free adapter collects crates.io observations in Python and
//! reconciles them here, through the production
//! `evaluate_final_registry_preflight_v1` model. This example performs no
//! network access, reads no credentials, and authorizes nothing: it parses a
//! `FinalRegistryPreflightInputV1` document, evaluates it, and writes the
//! canonical receipt bytes. Any non-`Complete` outcome is carried by the
//! receipt itself; this helper only fails on malformed input or I/O errors.

use std::path::PathBuf;

use allow_report::{
    FinalRegistryPreflightInputV1, evaluate_final_registry_preflight_v1,
    render_final_registry_preflight_v1,
};

fn flag_value(args: &[String], flag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    args.windows(2)
        .find_map(|window| match window {
            [name, value] if name == flag => Some(PathBuf::from(value)),
            _ => None,
        })
        .ok_or_else(|| {
            std::io::Error::other(format!("missing required flag {flag}")).into()
        })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let input_path = flag_value(&args, "--input")?;
    let receipt_path = flag_value(&args, "--receipt-out")?;
    let raw = std::fs::read_to_string(&input_path)?;
    let input: FinalRegistryPreflightInputV1 = serde_json::from_str(&raw)?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    let rendered = render_final_registry_preflight_v1(&receipt)?;
    if let Some(parent) = receipt_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&receipt_path, format!("{rendered}\n"))?;
    println!(
        "final registry preflight: result={:?} findings={} upload_rows={} shared={} surplus={}",
        receipt.result,
        receipt.findings.len(),
        receipt.upload_rows.len(),
        receipt.shared_prerequisites.len(),
        receipt.surplus_observations.len()
    );
    Ok(())
}
