// SPDX-License-Identifier: AGPL-3.0-only
//! Admission refusals are independent of collection timing and illustration values.

use nrz_compute::admission::{AdmissionError, AdmissionPolicy, check_admission};

/// Fixed shared corpus record, never a live measurement.
fn fixture() -> Result<nrz_compute::SummaryDataset, nrz_compute::codec::CodecError> {
    let bytes = include_bytes!("../../../data/fixtures/compute/valid-complete.json");
    nrz_compute::read_summary(bytes.as_slice(), u64::MAX - 1)
}

/// Explicit test policy, deliberately not adopted by repository inputs.
const fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        metadata_string_bytes: 4096,
        metadata_list_items: 64,
        source_references: 64,
        summary_bytes: 1_048_576,
        aggregate_bytes: 2_097_152,
    }
}

#[test]
fn missing_policy_refuses_even_structurally_valid_summary() -> Result<(), Box<dyn std::error::Error>>
{
    assert!(matches!(
        check_admission(&fixture()?, None, 0, 0),
        Err(AdmissionError::MissingPolicy)
    ));
    Ok(())
}

#[test]
fn aggregate_bytes_include_index_and_existing_summaries() -> Result<(), Box<dyn std::error::Error>>
{
    let summary = fixture()?;
    let size = check_admission(&summary, Some(policy()), 0, 0)?;
    let mut policy = policy();
    policy.aggregate_bytes = size + 149;
    assert!(matches!(
        check_admission(&summary, Some(policy), 100, 50),
        Err(AdmissionError::Capacity("aggregate bytes"))
    ));
    policy.aggregate_bytes += 1;
    assert_eq!(check_admission(&summary, Some(policy), 100, 50)?, size);
    Ok(())
}

#[test]
fn encoded_metadata_budget_counts_json_escaping() -> Result<(), Box<dyn std::error::Error>> {
    let mut summary = fixture()?;
    summary.payload.execution.command = vec!["\\".repeat(2050)];
    assert!(matches!(
        check_admission(&summary, Some(policy()), 0, 0),
        Err(AdmissionError::Capacity("metadata string"))
    ));
    Ok(())
}

#[test]
fn schema_bound_depends_on_cases_and_metadata_not_observation_counts()
-> Result<(), Box<dyn std::error::Error>> {
    let summary = fixture()?;
    let bound = nrz_compute::admission::schema_size(policy())?;
    let actual = check_admission(&summary, Some(policy()), 0, 0)?;
    assert!(actual <= bound.report(1)?);
    assert_eq!(bound.report(0)?, bound.header);
    assert_eq!(bound.report(70)?, bound.header + 70 * bound.case + 69);
    assert!(bound.report(u64::MAX).is_err());
    let mut expanded = policy();
    expanded.metadata_string_bytes += 1;
    assert!(nrz_compute::admission::schema_size(expanded)?.case > bound.case);
    Ok(())
}
