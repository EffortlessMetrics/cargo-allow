use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::parse_rust_syntax;

mod attributes;
mod collector;
mod fingerprint;
mod index;
mod panic;
mod safety_comment_association;
mod safety_comments;
mod scopes;
mod unsafe_constructs;

pub(crate) struct SyntaxFactsOutcome {
    pub facts: RustSyntaxFacts,
    pub has_parse_error: bool,
}

pub(crate) fn syntax_facts_with_outcome(source: &str) -> SyntaxFactsOutcome {
    let Ok(tree) = parse_rust_syntax(source) else {
        return SyntaxFactsOutcome {
            facts: RustSyntaxFacts::default(),
            has_parse_error: true,
        };
    };

    let has_parse_error = tree.has_error();
    let mut facts = RustSyntaxFacts::default();
    let root = tree.tree.root_node();
    collector::collect_syntax_facts(root, source, &mut facts);
    scopes::collect_line_scopes(root, source, &mut facts.scopes);
    safety_comment_association::associate_safety_comments(root, source, &mut facts);
    SyntaxFactsOutcome {
        facts,
        has_parse_error,
    }
}
