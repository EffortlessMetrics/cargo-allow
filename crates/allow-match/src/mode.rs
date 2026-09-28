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

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audit => "audit",
            Self::NoNew => "no-new",
            Self::Strict => "strict",
            Self::Release => "release",
        }
    }

    pub fn is_advisory(self) -> bool {
        matches!(self, Self::Audit)
    }

    /// Mode law for one match outcome.
    ///
    /// - Audit never fails.
    /// - Strict/Release fail on everything except a clean match or a
    ///   location-drift annotation.
    /// - No-new fails on new, ambiguous, invalid-selector,
    ///   missing-required-field, and evidence-missing outcomes. Since
    ///   #4238, `Expired` is annotation-only here: fixed evaluator + fixed
    ///   subject + fixed policy + fixed findings produce the same verdict
    ///   before, on, and after lifecycle dates. Exact, structural, and
    ///   occurrence-bounded matches stay authorized when expired; expired
    ///   broad matchers re-raise their findings as `New`, which still fails.
    ///   A repository can restore the pre-#4238 posture with
    ///   `requirements.calendar_expiry_blocks_no_new = true`, enforced at the
    ///   gate layer (see `allow_core::Requirements`), not in this law table.
    pub fn fails(self, status: MatchStatus) -> bool {
        match self {
            Self::Audit => false,
            Self::NoNew => status.is_failure_in_no_new(),
            Self::Strict | Self::Release => status.is_failure_in_strict(),
        }
    }
}
