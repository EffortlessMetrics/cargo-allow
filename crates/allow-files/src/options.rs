use allow_core::FileFamilyRule;

#[derive(Debug, Clone, Default)]
pub struct FileScanOptions {
    pub generated: Vec<String>,
    pub file_families: Vec<FileFamilyRule>,
}
