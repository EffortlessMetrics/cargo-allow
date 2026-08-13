use allow_core::FileFamilyRule;

#[derive(Debug, Clone, Default)]
pub struct FileScanOptions {
    pub generated: Vec<String>,
    pub file_families: Vec<FileFamilyRule>,
    /// When true, read the first 2 KiB of each file and classify it as
    /// generated if it contains a code-generation marker (@generated,
    /// DO NOT EDIT, Code generated). Default is path-only classification
    /// for performance (#1874).
    pub content_aware_generated: bool,
}
