// SPDX-License-Identifier: AGPL-3.0-only
//! Structural reconstruction of retained diagnostic and length-pilot observations.

use super::CodecError;
use crate::{
    CaseRequest, ExecutionBounds,
    pilot::{PilotReport, PilotStop},
};

/// Validate fixed diagnostic counts, doubling identity and complete-pilot prerequisites.
pub(super) fn validate(
    report: &PilotReport,
    case: &CaseRequest,
    bounds: Option<ExecutionBounds>,
) -> Result<(), CodecError> {
    if report.empty_ns.len() > 256
        || report.batches.len() > 21
        || report.observed_tick_ns == Some(0)
        || report.declared_resolution_ns == crate::Evidence::Known(0)
    {
        return Err(CodecError::Invalid("pilot shape or resolution"));
    }
    let mut length = case.item_length;
    for (index, batch) in report.batches.iter().enumerate() {
        if batch.length != length || batch.elapsed_ns.len() > 8 || report.empty_ns.len() != 256 {
            return Err(CodecError::Invalid("pilot length identity"));
        }
        if index + 1 < report.batches.len() {
            if batch.elapsed_ns.len() != 8 {
                return Err(CodecError::Invalid("unfinished pilot before next length"));
            }
            let maximum = batch
                .elapsed_ns
                .iter()
                .copied()
                .max()
                .ok_or(CodecError::Invalid("empty pilot"))?;
            if let Some(bounds) = bounds
                && maximum
                    .checked_mul(2)
                    .is_none_or(|prediction| prediction > bounds.item_ns)
            {
                return Err(CodecError::Invalid("unadmitted pilot doubling"));
            }
            length = length
                .checked_mul(2)
                .ok_or(CodecError::Invalid("pilot length overflow"))?;
        }
    }
    let mut previous = None;
    for failure in &report.failures {
        if failure.attempt >= report.attempted
            || previous.is_some_and(|index| failure.attempt <= index)
        {
            return Err(CodecError::Invalid("pilot failure index"));
        }
        previous = Some(failure.attempt);
    }
    if report.stop == PilotStop::Complete {
        let batch = report
            .batches
            .last()
            .ok_or(CodecError::Invalid("complete pilot without workload"))?;
        if batch.elapsed_ns.len() != 8
            || report.accepted_length != Some(batch.length)
            || !report.failures.is_empty()
        {
            return Err(CodecError::Invalid("complete pilot evidence"));
        }
        let tick = report
            .observed_tick_ns
            .ok_or(CodecError::Invalid("missing observed tick"))?
            .max(match report.declared_resolution_ns {
                crate::Evidence::Known(value) => value,
                _ => 0,
            });
        let empty = median_twice(&report.empty_ns)?;
        let target = tick
            .checked_mul(100)
            .zip(empty.checked_mul(50))
            .map(|(r, c)| r.max(c).max(100_000))
            .ok_or(CodecError::Invalid("pilot target overflow"))?;
        if report.target_ns != Some(target)
            || median_twice(&batch.elapsed_ns)?
                < target
                    .checked_mul(2)
                    .ok_or(CodecError::Invalid("pilot target"))?
        {
            return Err(CodecError::Invalid("pilot target"));
        }
        if let Some(bounds) = bounds
            && report
                .empty_ns
                .iter()
                .chain(report.batches.iter().flat_map(|batch| &batch.elapsed_ns))
                .any(|time| *time > bounds.item_ns)
        {
            return Err(CodecError::Invalid("accepted observed bound overrun"));
        }
    } else if report.accepted_length.is_some() {
        return Err(CodecError::Invalid("failed accepted pilot"));
    }
    Ok(())
}

/// Exact central-pair sum over the fixed diagnostic population.
fn median_twice(values: &[u64]) -> Result<u64, CodecError> {
    if values.is_empty() || values.len() > 256 || values.len() % 2 != 0 {
        return Err(CodecError::Invalid("pilot median population"));
    }
    let mut sorted = [0_u64; 256];
    sorted[..values.len()].copy_from_slice(values);
    sorted[..values.len()].sort_unstable();
    sorted[values.len() / 2 - 1]
        .checked_add(sorted[values.len() / 2])
        .ok_or(CodecError::Invalid("pilot median overflow"))
}
