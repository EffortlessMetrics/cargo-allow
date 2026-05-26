use allow_core::{
    CargoAllowError, CargoAllowResult, Finding, FindingKind, Span, StructuralIdentity,
    normalize_snippet, stable_hash_hex,
};
use std::fs;
use std::path::{Path, PathBuf};

pub fn scan_rust_files(
    root: impl AsRef<Path>,
    files: &[PathBuf],
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut out = Vec::new();
    for rel in files {
        if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let path = root.join(rel);
        let text = fs::read_to_string(&path)
            .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
        out.extend(scan_rust_source(rel, &text));
    }
    Ok(out)
}

pub fn scan_rust_source(path: impl AsRef<Path>, source: &str) -> Vec<Finding> {
    let path = path.as_ref().to_path_buf();
    let mut findings = Vec::new();
    let mut container: Option<String> = None;
    let mut container_depth: Option<i32> = None;
    let mut brace_depth: i32 = 0;
    let mut module_stack: Vec<String> = Vec::new();

    for (line_idx, raw_line) in source.lines().enumerate() {
        let line_no = (line_idx + 1) as u32;
        let line = raw_line;
        let trimmed = line.trim();
        if let Some(name) = parse_mod_name(trimmed) {
            module_stack.push(name);
        }
        if let Some(name) = parse_fn_name(trimmed) {
            container = Some(name);
            container_depth = Some(brace_depth + count_char(line, '{') - count_char(line, '}'));
        }

        scan_line(
            &path,
            line,
            line_no,
            &container,
            &module_stack,
            &mut findings,
        );

        brace_depth += count_char(line, '{') - count_char(line, '}');
        if let Some(depth) = container_depth {
            if brace_depth < depth {
                container = None;
                container_depth = None;
            }
        }
    }
    findings
}

fn scan_line(
    path: &Path,
    line: &str,
    line_no: u32,
    container: &Option<String>,
    module_stack: &[String],
    findings: &mut Vec<Finding>,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return;
    }

    if let Some(attr_text) = detect_attr(trimmed, "allow") {
        let lint = extract_first_lint(attr_text);
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: 1,
                container,
                module_stack,
            },
            FindingKind::LintException,
            "allow_attribute",
            "attribute",
            |id| {
                id.lint = lint;
                id.symbol = Some(trimmed.to_string());
            },
            findings,
        );
    }
    if let Some(attr_text) = detect_attr(trimmed, "expect") {
        let lint = extract_first_lint(attr_text);
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: 1,
                container,
                module_stack,
            },
            FindingKind::LintException,
            "expect_attribute",
            "attribute",
            |id| {
                id.lint = lint;
                id.symbol = Some(trimmed.to_string());
            },
            findings,
        );
    }

    if trimmed.contains("unsafe fn ")
        || trimmed.starts_with("unsafe fn ")
        || trimmed.starts_with("pub unsafe fn ")
    {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_fn",
            "unsafe_fn",
            |_| {},
            findings,
        );
    } else if trimmed.starts_with("unsafe impl") || trimmed.starts_with("pub unsafe impl") {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_impl",
            "unsafe_impl",
            |_| {},
            findings,
        );
    } else if trimmed.starts_with("unsafe trait") || trimmed.starts_with("pub unsafe trait") {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_trait",
            "unsafe_trait",
            |_| {},
            findings,
        );
    } else if trimmed.contains("unsafe extern") || trimmed.starts_with("unsafe extern") {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_extern_block",
            "unsafe_extern_block",
            |_| {},
            findings,
        );
    } else if trimmed.contains("unsafe {") || trimmed == "unsafe" {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_block",
            "unsafe_block",
            |_| {},
            findings,
        );
    }
    if trimmed.contains("#[unsafe(") {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_attr",
            "unsafe_attr",
            |_| {},
            findings,
        );
    }

    for (family, needle) in [("unwrap", ".unwrap("), ("expect", ".expect(")] {
        if let Some(pos) = line.find(needle) {
            let receiver = receiver_before(line, pos);
            push_finding(
                FindingSite {
                    path,
                    line,
                    line_no,
                    column: pos as u32 + 2,
                    container,
                    module_stack,
                },
                FindingKind::Panic,
                family,
                "method_call",
                |id| {
                    id.callee = Some(family.to_string());
                    id.receiver_fingerprint = Some(receiver);
                },
                findings,
            );
        }
    }

    for macro_name in ["panic", "todo", "unimplemented", "unreachable"] {
        let needle = format!("{macro_name}!(");
        if let Some(pos) = line.find(&needle) {
            let family = if macro_name == "panic" {
                "panic_macro"
            } else {
                macro_name
            };
            push_finding(
                FindingSite {
                    path,
                    line,
                    line_no,
                    column: pos as u32 + 1,
                    container,
                    module_stack,
                },
                FindingKind::Panic,
                family,
                "macro_call",
                |id| {
                    id.macro_name = Some(macro_name.to_string());
                },
                findings,
            );
        }
    }

    if looks_like_indexing(line) {
        let family = if line.contains("&") && line.contains("[") {
            "string_slice"
        } else {
            "indexing"
        };
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "["),
                container,
                module_stack,
            },
            FindingKind::Panic,
            family,
            "index_expr",
            |id| {
                id.symbol = Some(index_symbol(line));
                id.target_fingerprint = line.split('[').next().map(|s| {
                    normalize_snippet(s)
                        .chars()
                        .rev()
                        .take(40)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect()
                });
            },
            findings,
        );
    }
}

struct FindingSite<'a> {
    path: &'a Path,
    line: &'a str,
    line_no: u32,
    column: u32,
    container: &'a Option<String>,
    module_stack: &'a [String],
}

fn push_finding<F>(
    site: FindingSite<'_>,
    kind: FindingKind,
    family: &str,
    ast_kind: &str,
    enrich: F,
    findings: &mut Vec<Finding>,
) where
    F: FnOnce(&mut StructuralIdentity),
{
    let mut identity = StructuralIdentity::new("rust", ast_kind);
    identity.container = site.container.clone();
    if !site.module_stack.is_empty() {
        identity.module = Some(site.module_stack.join("::"));
    }
    identity.normalized_snippet_hash = Some(stable_hash_hex(&normalize_snippet(site.line)));
    identity.line_hint = Some(site.line_no);
    identity.column_hint = Some(site.column);
    enrich(&mut identity);
    findings.push(Finding {
        kind,
        family: Some(family.to_string()),
        path: site.path.to_path_buf(),
        span: Some(Span {
            line: site.line_no,
            column: site.column,
        }),
        identity,
        message: format!("{kind} {family} syntax found"),
    });
}

fn detect_attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let outer = format!("#[{name}(");
    let inner = format!("#![{name}(");
    if line.starts_with(&outer) {
        Some(&line[outer.len()..])
    } else if line.starts_with(&inner) {
        Some(&line[inner.len()..])
    } else {
        None
    }
}

fn extract_first_lint(text: &str) -> Option<String> {
    let until = text.split([',', ')']).next()?.trim();
    if until.is_empty() {
        None
    } else {
        Some(until.to_string())
    }
}

fn parse_fn_name(line: &str) -> Option<String> {
    let mut text = line;
    for prefix in [
        "pub(crate) ",
        "pub(super) ",
        "pub ",
        "async ",
        "const ",
        "unsafe ",
        "extern \"C\" ",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest;
        }
    }
    let idx = text.find("fn ")?;
    let rest = &text[idx + 3..];
    let name = rest
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_mod_name(line: &str) -> Option<String> {
    let text = line
        .strip_prefix("mod ")
        .or_else(|| line.strip_prefix("pub mod "))?;
    if !line.contains('{') {
        return None;
    }
    let name = text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn count_char(line: &str, ch: char) -> i32 {
    line.chars().filter(|c| *c == ch).count() as i32
}

fn column(line: &str, needle: &str) -> u32 {
    line.find(needle).map(|idx| idx as u32 + 1).unwrap_or(1)
}

fn receiver_before(line: &str, pos: usize) -> String {
    let prefix = &line[..pos];
    let trimmed = normalize_snippet(prefix);
    trimmed
        .chars()
        .rev()
        .take(80)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn looks_like_indexing(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("#[") || trimmed.starts_with("#![") || trimmed.starts_with("//") {
        return false;
    }
    if !trimmed.contains('[') || !trimmed.contains(']') {
        return false;
    }
    if trimmed.starts_with('[') || trimmed.contains("vec![") || trimmed.contains("format![") {
        return false;
    }
    if trimmed.contains("use ") && trimmed.contains("::") {
        return false;
    }
    line.match_indices('[').any(|(idx, _)| {
        line[..idx]
            .chars()
            .rev()
            .find(|ch| !ch.is_whitespace())
            .is_some_and(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ')' | ']' | '}'))
    })
}

fn index_symbol(line: &str) -> String {
    let norm = normalize_snippet(line);
    norm.chars().take(100).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_panic_family() {
        let src = r#"
        fn load() {
            let x = std::fs::read_to_string("x").unwrap();
            let y = items[0];
            panic!("bad");
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        assert!(
            findings
                .iter()
                .any(|f| f.family.as_deref() == Some("unwrap"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.family.as_deref() == Some("indexing"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.family.as_deref() == Some("panic_macro"))
        );
    }

    #[test]
    fn detects_unsafe_and_attrs() {
        let src = r#"
        #[allow(clippy::unwrap_used)]
        unsafe fn read() {
            unsafe { core::ptr::read(0 as *const u8); }
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::Unsafe
                    && f.family.as_deref() == Some("unsafe_block"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::LintException)
        );
    }

    #[test]
    fn indexing_heuristic_ignores_common_bracket_false_positives() {
        let src = r#"
        #[allow(dead_code)]
        fn load(xs: &[u8]) {
            let literal = [1, 2, 3];
            let nested_type: Vec<[u8; 4]> = Vec::new();
            let macro_vec = vec![1, 2, 3];
            let macro_custom = custom![1, 2, 3];
            use crate::{alpha, beta};
            let actual = xs[0];
            let call_index = xs.as_ref()[0];
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        let indexing = findings
            .iter()
            .filter(|f| f.family.as_deref() == Some("indexing"))
            .count();

        assert_eq!(indexing, 2);
    }

    #[test]
    fn indexing_heuristic_detects_true_positive_shapes() {
        let lb = char::from(91);
        let rb = char::from(93);
        let src = format!(
            r#"
        fn load(xs: &Vec<u8>, matrix: &Vec<Vec<u8>>) {{
            let direct = xs{lb}0{rb};
            let nested = matrix{lb}0{rb}{lb}1{rb};
            let call = xs.as_ref(){lb}0{rb};
        }}
        "#
        );
        let findings = scan_rust_source("src/lib.rs", &src);
        let indexing = findings
            .iter()
            .filter(|f| f.family.as_deref() == Some("indexing"))
            .count();

        assert_eq!(indexing, 3);
    }

    #[test]
    fn index_symbol_truncates_on_character_boundaries() {
        let line = format!("let actual = values[{}];", "\u{00e9}".repeat(120));

        assert_eq!(index_symbol(&line).chars().count(), 100);
    }
}
