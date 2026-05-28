use allow_core::{CargoAllowError, CargoAllowResult, Finding};
use std::fs;
use std::path::{Path, PathBuf};

mod finding_builder;
mod line_facts;
mod line_findings;
mod line_scan;
mod package;
mod safety_comments;
mod syntax_facts;
mod syntax_kinds;
mod syntax_tree;
mod text;

use line_scan::scan_source_lines;
use package::source_package_contexts;
use syntax_facts::syntax_facts;

pub use package::{
    SourcePackageContext, apply_source_package_context, source_package_contexts_from_sources,
};
pub use syntax_tree::{RustSyntaxContainer, RustSyntaxTree, parse_rust_syntax};

pub fn scan_rust_files(
    root: impl AsRef<Path>,
    files: &[PathBuf],
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut out = Vec::new();
    let packages = source_package_contexts(root, files)?;
    for rel in files {
        if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let path = root.join(rel);
        let text = fs::read_to_string(&path)
            .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
        let mut findings = scan_rust_source(rel, &text);
        apply_source_package_context(rel, &packages, &mut findings);
        out.extend(findings);
    }
    Ok(out)
}

pub fn scan_rust_source(path: impl AsRef<Path>, source: &str) -> Vec<Finding> {
    let path = path.as_ref().to_path_buf();
    let syntax = syntax_facts(source);
    scan_source_lines(&path, source, syntax)
}

#[cfg(test)]
mod tests;
