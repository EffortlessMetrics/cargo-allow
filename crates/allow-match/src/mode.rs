use allow_core::MatchStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    Audit,
    NoNew,
    Strict,
    Release,
}

impl CheckMode {
    pub fn parse(input: &str) -> Self {
        match input {
            "strict" => Self::Strict,
            "release" => Self::Release,
            "audit" => Self::Audit,
            _ => Self::NoNew,
        }
    }

    pub fn fails(self, status: MatchStatus) -> bool {
        match self {
            Self::Audit => false,
            Self::NoNew => status.is_failure_in_no_new(),
            Self::Strict | Self::Release => status.is_failure_in_strict(),
        }
    }
}
