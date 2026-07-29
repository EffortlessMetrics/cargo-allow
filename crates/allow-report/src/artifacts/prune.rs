#[derive(Debug, Clone, Copy)]
pub struct PruneModeContext<'a> {
    pub explicit_dry_run: bool,
    pub write_requested: bool,
    pub written_path: Option<&'a str>,
    /// Total entries in the policy before pruning. Used for the "K remaining" line.
    pub total_entries: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct PruneCandidate<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub owner: &'a str,
    pub classification: &'a str,
    pub scope: &'a str,
    pub reason: &'a str,
}
