use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::parse_rust_syntax;

mod attributes;
mod collector;
mod fingerprint;
mod index;
mod panic;
mod scopes;
mod unsafe_constructs;

pub(crate) fn syntax_facts(source: &str) -> RustSyntaxFacts {
    let Ok(tree) = parse_rust_syntax(source) else {
        return RustSyntaxFacts::default();
    };

    let mut facts = RustSyntaxFacts::default();
    collector::collect_syntax_facts(tree.tree.root_node(), source, &mut facts);
    scopes::collect_line_scopes(tree.tree.root_node(), source, &mut facts.scopes);
    facts
}
