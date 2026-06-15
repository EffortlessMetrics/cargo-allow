use std::path::Path;

use super::LineContext;

#[test]
fn site_field_discriminator() {
    let path = Path::new("src/lib.rs");
    let line = "    unwrap();";
    let container = Some("Parser".to_string());
    let modules = vec!["crate".to_string(), "parser".to_string()];
    let context = LineContext {
        path,
        line,
        line_no: 12,
        container: &container,
        module_stack: &modules,
    };

    let site = context.site(5);

    assert_eq!(site.path, path);
    assert_eq!(site.line, line);
    assert_eq!(site.line_no, 12);
    assert_eq!(site.column, 5);
    assert_eq!(site.container, &container);
    assert_eq!(site.module_stack, modules.as_slice());
}
