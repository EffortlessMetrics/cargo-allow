use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InventoryOptions {
    pub ignored: Vec<String>,
    pub generated: Vec<String>,
    pub include_untracked: bool,
}

impl Default for InventoryOptions {
    fn default() -> Self {
        Self {
            ignored: vec![".git/**".to_string(), "target/**".to_string()],
            generated: Vec::new(),
            include_untracked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventorySource {
    GitTracked,
    FilesystemFallback,
    FilesystemIncludeUntracked,
}

impl InventorySource {
    pub const ALL: &[Self] = &[
        Self::GitTracked,
        Self::FilesystemFallback,
        Self::FilesystemIncludeUntracked,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitTracked => "git_tracked",
            Self::FilesystemFallback => "filesystem_fallback",
            Self::FilesystemIncludeUntracked => "filesystem_include_untracked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub files: Vec<PathBuf>,
    pub source: InventorySource,
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod tests;
