use std::path::Path;

use crate::finding_builder::FindingSite;

#[derive(Clone, Copy)]
pub(crate) struct LineContext<'a> {
    pub(crate) path: &'a Path,
    pub(crate) line: &'a str,
    pub(crate) line_no: u32,
    pub(crate) container: &'a Option<String>,
    pub(crate) module_stack: &'a [String],
}

impl<'a> LineContext<'a> {
    pub(crate) fn site(self, column: u32) -> FindingSite<'a> {
        FindingSite {
            path: self.path,
            line: self.line,
            line_no: self.line_no,
            column,
            container: self.container,
            module_stack: self.module_stack,
        }
    }
}

#[cfg(test)]
#[path = "line_context_tests.rs"]
mod tests;
