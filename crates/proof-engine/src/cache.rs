//! Proof receipt cache for proof-engine orchestration (#2589-A).

use proof_protocol::{ProofReceiptSetV1, validate_receipt_set};

pub const PROOF_CACHE_SCHEMA_ID: &str = "proof.cache.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheEntryV1 {
    pub cache_key: String,
    pub receipt_set: ProofReceiptSetV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheV1 {
    pub schema_id: String,
    pub entries: Vec<ProofCacheEntryV1>,
}

impl ProofCacheV1 {
    pub fn new() -> Self {
        Self {
            schema_id: PROOF_CACHE_SCHEMA_ID.to_string(),
            entries: Vec::new(),
        }
    }

    pub fn insert(
        &mut self,
        cache_key: impl Into<String>,
        receipt_set: ProofReceiptSetV1,
    ) -> Result<(), CacheError> {
        validate_receipt_set(&receipt_set).map_err(CacheError::Receipt)?;
        let cache_key = cache_key.into();
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|entry| entry.cache_key == cache_key)
        {
            existing.receipt_set = receipt_set;
            return Ok(());
        }
        self.entries.push(ProofCacheEntryV1 {
            cache_key,
            receipt_set,
        });
        Ok(())
    }

    pub fn get(&self, cache_key: &str) -> Option<&ProofReceiptSetV1> {
        self.entries
            .iter()
            .find(|entry| entry.cache_key == cache_key)
            .map(|entry| &entry.receipt_set)
    }
}

impl Default for ProofCacheV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheError {
    InvalidSchemaId { observed: String },
    Receipt(proof_protocol::ProofReceiptError),
}

impl CacheError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::Receipt(_) => "receipt_invalid",
        }
    }
}

pub fn validate_proof_cache(cache: &ProofCacheV1) -> Result<(), CacheError> {
    if cache.schema_id != PROOF_CACHE_SCHEMA_ID {
        return Err(CacheError::InvalidSchemaId {
            observed: cache.schema_id.clone(),
        });
    }
    for entry in &cache.entries {
        validate_receipt_set(&entry.receipt_set).map_err(CacheError::Receipt)?;
    }
    Ok(())
}

pub fn cache_key_for_plan(plan_id: &str, digest: &str) -> String {
    format!("{plan_id}::{digest}")
}
