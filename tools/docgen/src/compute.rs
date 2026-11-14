// SPDX-License-Identifier: AGPL-3.0-only
//! Structural compute-summary validation without raw reports or engine dependencies.

mod admission;
mod schema;
mod validate;

/// Validate a current compute summary without collecting or reanalyzing observations.
/// # Errors
/// Rejects unknown/duplicate fields, malformed variants and structural/count/grid invariants.
pub fn validate_summary_bytes(bytes: &[u8]) -> Result<(), String> {
    let summary: schema::SummaryDataset =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    validate::summary(&summary)
}

/// Derive deterministic display context from the committed admitted-summary index.
pub(crate) fn derive(root: &std::path::Path) -> Result<(Vec<u8>, serde_json::Value), crate::Error> {
    let payload = admission::read(root)?;
    let fields = payload
        .as_object()
        .ok_or_else(|| crate::Error::Payload {
            path: "data/inputs/compute/index.json".to_owned(),
            detail: "invalid compute context".to_owned(),
        })?
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let sources = ["data/inputs/compute/index.json".to_owned()];
    let bytes = crate::envelope::encode(
        "data/datasets/compute.json",
        "org.naturrhythmuszentrum.docs.dataset.compute",
        "NRZ",
        &sources,
        &[],
        &fields,
    )?;
    let value = serde_json::from_slice(&bytes).map_err(|error| crate::Error::Payload {
        path: "data/datasets/compute.json".to_owned(),
        detail: error.to_string(),
    })?;
    Ok((bytes, value))
}

/// Recheck admitted bytes/receipts when rendering already committed display context.
pub(crate) fn validate_dataset(
    root: &std::path::Path,
    value: &serde_json::Value,
) -> Result<(), crate::Error> {
    let (_, expected) = derive(root)?;
    if value != &expected {
        return Err(crate::Error::Payload {
            path: "data/datasets/compute.json".to_owned(),
            detail: "stale compute display context".to_owned(),
        });
    }
    Ok(())
}

/// Validate the committed compute index, byte policy and exact summary receipts.
/// # Errors
/// Refuses undeclared artifacts, missing policy, stale receipts and invalid summaries.
pub fn validate_inputs(root: &std::path::Path) -> Result<(), String> {
    admission::read(root)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
