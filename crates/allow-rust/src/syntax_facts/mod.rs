use crate::syntax_kinds::RustSyntaxFacts;
use crate::syntax_tree::parse_rust_syntax;
use crate::text::SourceLineIndex;

mod attributes;
mod collector;
mod fingerprint;
mod index;
mod panic;
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
    let line_index = SourceLineIndex::new(source);
    let mut facts = RustSyntaxFacts::default();
    collector::collect_syntax_facts(tree.tree.root_node(), source, &line_index, &mut facts);
    scopes::collect_line_scopes(tree.tree.root_node(), source, &mut facts.scopes);
    SyntaxFactsOutcome {
        facts,
        has_parse_error,
    }
}
