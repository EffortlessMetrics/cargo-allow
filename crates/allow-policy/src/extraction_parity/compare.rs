use super::config::ParityComparisonResult;
use sha2::{Digest, Sha256};

/// Canonical output produced by an old or new operation adapter.
///
/// Adapters own conversion of their surface-specific result into a stable
/// representation. The comparison kernel deliberately does not execute the
/// operation or infer semantic equivalence from an opaque result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityObservation {
    pub source_identity: String,
    pub canonical_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityComparison {
    pub result: ParityComparisonResult,
    pub source_identity: String,
}

/// Compare two adapter observations from the same source subject.
///
/// A source identity mismatch is always non-clean. Equal canonical outputs
/// are the only result this kernel may promote automatically; differences stay
/// explicitly unreviewed until a surface-specific adapter supplies a reviewed
/// normalization or intentional-difference rule.
pub fn compare_observations(old: &ParityObservation, new: &ParityObservation) -> ParityComparison {
    if old.source_identity != new.source_identity {
        return ParityComparison {
            result: ParityComparisonResult::SourceIdentityMismatch,
            source_identity: old.source_identity.clone(),
        };
    }

    let result = if old.canonical_output == new.canonical_output {
        ParityComparisonResult::SemanticallyEquivalent
    } else {
        ParityComparisonResult::UnreviewedDifference
    };
    ParityComparison {
        result,
        source_identity: old.source_identity.clone(),
    }
}

/// Compute a stable SHA-256 digest for comparison records.
///
/// Records are sorted by case id and encoded with length-prefixed fields so
/// case ordering and delimiter characters cannot create ambiguous digests.
pub fn corpus_digest(records: &[(String, ParityComparison, String, String)]) -> String {
    let mut records = records.to_vec();
    records.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (case_id, comparison, old_output, new_output) in records {
        update_field(&mut hasher, &case_id);
        update_field(&mut hasher, comparison.result.as_str());
        update_field(&mut hasher, &comparison.source_identity);
        update_field(&mut hasher, &old_output);
        update_field(&mut hasher, &new_output);
    }
    hex_digest(hasher.finalize().as_slice())
}

fn update_field(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_string().as_bytes());
    hasher.update([b':']);
    hasher.update(value.as_bytes());
    hasher.update([b'|']);
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(identity: &str, output: &str) -> ParityObservation {
        ParityObservation {
            source_identity: identity.to_string(),
            canonical_output: output.to_string(),
        }
    }

    #[test]
    fn equal_outputs_promote_to_semantic_equivalence() -> Result<(), String> {
        let result = compare_observations(
            &observation("tree:abc", "{\"path\":\"src/lib.rs\"}"),
            &observation("tree:abc", "{\"path\":\"src/lib.rs\"}"),
        );
        if result.result != ParityComparisonResult::SemanticallyEquivalent {
            return Err(format!("unexpected result: {:?}", result.result));
        }
        Ok(())
    }

    #[test]
    fn stale_source_identity_is_not_a_current_parity_result() -> Result<(), String> {
        let result = compare_observations(
            &observation("tree:old", "same"),
            &observation("tree:current", "same"),
        );
        if result.result != ParityComparisonResult::SourceIdentityMismatch {
            return Err(format!("unexpected result: {:?}", result.result));
        }
        Ok(())
    }

    #[test]
    fn differences_remain_unreviewed() -> Result<(), String> {
        let result = compare_observations(
            &observation("index:abc", "old"),
            &observation("index:abc", "new"),
        );
        if result.result != ParityComparisonResult::UnreviewedDifference {
            return Err(format!("unexpected result: {:?}", result.result));
        }
        if result.result.satisfies_migration() {
            return Err("unreviewed difference satisfied migration".to_string());
        }
        Ok(())
    }

    #[test]
    fn corpus_digest_is_independent_of_record_order() -> Result<(), String> {
        let equivalent = ParityComparison {
            result: ParityComparisonResult::SemanticallyEquivalent,
            source_identity: "tree:abc".to_string(),
        };
        let different = ParityComparison {
            result: ParityComparisonResult::UnreviewedDifference,
            source_identity: "tree:abc".to_string(),
        };
        let first = corpus_digest(&[
            (
                "b".to_string(),
                different.clone(),
                "x".to_string(),
                "y".to_string(),
            ),
            (
                "a".to_string(),
                equivalent.clone(),
                "same".to_string(),
                "same".to_string(),
            ),
        ]);
        let second = corpus_digest(&[
            (
                "a".to_string(),
                equivalent,
                "same".to_string(),
                "same".to_string(),
            ),
            ("b".to_string(), different, "x".to_string(), "y".to_string()),
        ]);
        if first != second {
            return Err("record order changed corpus digest".to_string());
        }
        if first.len() != 64 {
            return Err(format!("unexpected digest length: {}", first.len()));
        }
        Ok(())
    }
}
