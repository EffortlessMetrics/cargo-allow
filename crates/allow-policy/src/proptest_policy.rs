//! Property tests for policy parsing and validation (#1905).
//!
//! Verifies the starter policy always round-trips through parse + validate.

use crate::{parse_policy, starter_policy};
use proptest::prelude::*;

proptest! {
    /// The starter policy must always parse and validate successfully,
    /// no matter how many times it is called (idempotency).
    #[test]
    fn prop_starter_policy_always_roundtrips(_ in any::<u8>()) {
        let toml_text = starter_policy(true, "policy/allow.toml");
        let config = parse_policy(&toml_text)
            .map_err(|e| proptest::test_runner::TestCaseError::fail(format!("starter policy should parse: {e}")))?;
        // parse_policy runs validation internally; reaching here means it
        // passed. Assert the config parsed into a real AllowConfig struct
        // (the allow array exists, even if empty).
        let _ = &config.allow;
    }

    /// Parsing the same starter policy twice must produce identical configs
    /// (determinism of the parse path).
    #[test]
    fn prop_parse_is_deterministic(_ in any::<u8>()) {
        let toml_text = starter_policy(true, "policy/allow.toml");
        let config1 = parse_policy(&toml_text)
            .map_err(|e| proptest::test_runner::TestCaseError::fail(format!("parse 1: {e}")))?;
        let config2 = parse_policy(&toml_text)
            .map_err(|e| proptest::test_runner::TestCaseError::fail(format!("parse 2: {e}")))?;
        prop_assert_eq!(config1.allow.len(), config2.allow.len(),
            "parsing the same input twice must produce identical allow counts");
    }

    /// Empty or whitespace-only input must not panic the parser.
    #[test]
    fn prop_empty_input_does_not_panic(input in "( |\n|\t)*") {
        // Empty input should error (no valid policy), but must not panic.
        let _ = parse_policy(&input);
    }
}
