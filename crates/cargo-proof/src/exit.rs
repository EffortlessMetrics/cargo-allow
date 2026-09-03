//! Process exit mapping for cargo-proof (#2589-B).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessExitFamilyV1 {
    Success,
    Blocking,
    Usage,
    InstrumentFailure,
}

impl ProcessExitFamilyV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Blocking => "blocking",
            Self::Usage => "usage",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

pub fn exit_code_for_family(family: ProcessExitFamilyV1) -> i32 {
    match family {
        ProcessExitFamilyV1::Success => 0,
        ProcessExitFamilyV1::Blocking => 1,
        ProcessExitFamilyV1::Usage => 2,
        ProcessExitFamilyV1::InstrumentFailure => 1,
    }
}

pub fn exit_family_for_result_class(result_class: &str) -> ProcessExitFamilyV1 {
    match result_class {
        "completed" => ProcessExitFamilyV1::Success,
        "findings" => ProcessExitFamilyV1::Blocking,
        "repository_decision_required" => ProcessExitFamilyV1::Blocking,
        "malformed_input" => ProcessExitFamilyV1::Usage,
        // The proof-corpus vocabulary's provider-unavailable state
        // (ProofResultStateV1::ProviderUnavailable, proof-protocol) maps to
        // the instrument-failure family — distinct from usage — so a
        // missing provider is never confused with a malformed command
        // (#3598 exit-family follow-up).
        "provider_unavailable"
        | "stale_input"
        | "unsupported"
        | "instrument_failure"
        | "partial_data"
        | "not_proven"
        | "cancelled"
        | "conflict" => ProcessExitFamilyV1::InstrumentFailure,
        _ => ProcessExitFamilyV1::InstrumentFailure,
    }
}

pub fn exit_code_for_result_class(result_class: &str) -> i32 {
    exit_code_for_family(exit_family_for_result_class(result_class))
}
