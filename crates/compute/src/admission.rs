// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit repository policy; no illustrative byte value is an adopted default.

mod size;
pub use size::{SizeBound, schema_size};

use crate::{SummaryDataset, codec::CodecError};
use serde::{Deserialize, Serialize};

/// Maintainer-selected metadata and repository storage maxima.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionPolicy {
    /// Maximum encoded bytes of each public metadata string, including JSON escaping.
    pub metadata_string_bytes: u64,
    /// Maximum entries of every variable metadata list.
    pub metadata_list_items: u64,
    /// Maximum source references in a summary envelope.
    pub source_references: u64,
    /// Maximum encoded bytes of one admitted summary.
    pub summary_bytes: u64,
    /// Maximum current index plus all admitted summary bytes.
    pub aggregate_bytes: u64,
}

/// Missing or exceeded repository policy, separate from measurement success.
#[derive(Debug, thiserror::Error)]
pub enum AdmissionError {
    /// Repository maintainers have not selected the required policy values.
    #[error("compute summary admission policy is undecided")]
    MissingPolicy,
    /// Metadata or encoded storage exceeds the explicit policy.
    #[error("compute summary exceeds {0}")]
    Capacity(&'static str),
    /// Structurally invalid summary cannot be admitted under any byte policy.
    #[error(transparent)]
    Schema(#[from] CodecError),
}

/// Validate a summary against explicit metadata and aggregate byte policy.
///
/// This performs no repository write, raw verification or implicit admission receipt.
/// # Errors
/// Missing policy refuses admission; metadata, encoding and aggregate overflows refuse.
pub fn check_admission(
    summary: &SummaryDataset,
    policy: Option<AdmissionPolicy>,
    index_bytes: u64,
    other_summary_bytes: u64,
) -> Result<u64, AdmissionError> {
    let policy = policy.ok_or(AdmissionError::MissingPolicy)?;
    if [
        policy.metadata_string_bytes,
        policy.metadata_list_items,
        policy.source_references,
        policy.summary_bytes,
        policy.aggregate_bytes,
    ]
    .into_iter()
    .any(|value| value > crate::decimal::MAX_INTEGER)
    {
        return Err(AdmissionError::Capacity("policy count domain"));
    }
    crate::codec::validate_summary(summary)?;
    let value = serde_json::to_value(summary).map_err(CodecError::from)?;
    metadata(&value, "", policy)?;
    if u64::try_from(summary.sources.len())
        .map_err(|_| AdmissionError::Capacity("source references"))?
        > policy.source_references
    {
        return Err(AdmissionError::Capacity("source references"));
    }
    let mut bytes = Vec::new();
    crate::write_summary(summary, &mut bytes)?;
    let size = u64::try_from(bytes.len()).map_err(|_| AdmissionError::Capacity("summary bytes"))?;
    let bound = schema_size(policy)?.report(
        u64::try_from(summary.payload.cases.len())
            .map_err(|_| AdmissionError::Capacity("case count"))?,
    )?;
    if size > bound {
        return Err(AdmissionError::Capacity("schema-derived bytes"));
    }
    if size > policy.summary_bytes {
        return Err(AdmissionError::Capacity("summary bytes"));
    }
    let aggregate = index_bytes
        .checked_add(other_summary_bytes)
        .and_then(|value| value.checked_add(size))
        .ok_or(AdmissionError::Capacity("aggregate bytes"))?;
    if aggregate > policy.aggregate_bytes {
        return Err(AdmissionError::Capacity("aggregate bytes"));
    }
    Ok(size)
}

/// Variable metadata budgets count encoded string escaping, never raw source character counts.
fn metadata(
    value: &serde_json::Value,
    field: &str,
    policy: AdmissionPolicy,
) -> Result<(), AdmissionError> {
    match value {
        serde_json::Value::String(text) => {
            let decimal = matches!(
                field,
                "point"
                    | "standard_error"
                    | "lower"
                    | "upper"
                    | "confidence"
                    | "fences"
                    | "control_median_seconds"
                    | "accepted_median_seconds"
            );
            if !decimal {
                let bytes = serde_json::to_vec(text).map_err(CodecError::from)?;
                if u64::try_from(bytes.len())
                    .map_err(|_| AdmissionError::Capacity("metadata string"))?
                    > policy.metadata_string_bytes
                {
                    return Err(AdmissionError::Capacity("metadata string"));
                }
            }
        }
        serde_json::Value::Array(values) => {
            let fixed = matches!(
                field,
                "version" | "raw_version" | "estimates" | "fences" | "outlier_counts" | "cases"
            );
            if !fixed
                && u64::try_from(values.len())
                    .map_err(|_| AdmissionError::Capacity("metadata list"))?
                    > policy.metadata_list_items
            {
                return Err(AdmissionError::Capacity("metadata list"));
            }
            for value in values {
                metadata(value, field, policy)?;
            }
        }
        serde_json::Value::Object(fields) => {
            for (field, value) in fields {
                metadata(value, field, policy)?;
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
    Ok(())
}
