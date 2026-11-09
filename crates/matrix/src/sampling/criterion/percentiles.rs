// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/stats/univariate/percentiles.rs`; `src/stats/univariate/sample.rs`; items: `Percentiles::at`, median, quartiles, `at_unchecked` interpolation; `Sample::percentiles`.
//! Modifications: safe indexing, f64 specialization and cancellable allocation-free heapsort.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use super::{AnalysisError, AnalysisPhase, Control, finite, push};

/// Copy into admitted scratch, checking cancellation at every element.
pub(super) fn copy_sort(
    values: &[f64],
    sorted: &mut Vec<f64>,
    control: &mut Control<'_>,
) -> Result<(), AnalysisError> {
    sorted.clear();
    for &value in values {
        control.check(AnalysisPhase::Sorting)?;
        push(sorted, finite(value)?)?;
    }
    sort(sorted, control)
}

/// Order finite values in-place; each sift level is a fixed-size checked chunk.
pub(super) fn sort(values: &mut [f64], control: &mut Control<'_>) -> Result<(), AnalysisError> {
    control.check(AnalysisPhase::Sorting)?;
    let length = values.len();
    for root in (0..length / 2).rev() {
        sift(values, root, length, control)?;
    }
    for end in (1..length).rev() {
        control.check(AnalysisPhase::Sorting)?;
        swap(values, 0, end)?;
        sift(values, 0, end, control)?;
    }
    Ok(())
}

/// Safe swap with explicit extent validation before the slice operation.
const fn swap(values: &mut [f64], left: usize, right: usize) -> Result<(), AnalysisError> {
    let left_valid = left < values.len();
    let right_valid = right < values.len();
    if left_valid && right_valid {
        values.swap(left, right);
        Ok(())
    } else {
        Err(AnalysisError::InvalidIndex)
    }
}

/// Restore a heap path, bounded by the finite buffer length.
fn sift(
    values: &mut [f64],
    mut root: usize,
    end: usize,
    control: &mut Control<'_>,
) -> Result<(), AnalysisError> {
    while root < end / 2 {
        control.check(AnalysisPhase::Sorting)?;
        let mut child = root
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(AnalysisError::InvalidIndex)?;
        let right = child.checked_add(1).ok_or(AnalysisError::InvalidIndex)?;
        if right < end {
            let left_value = values.get(child).ok_or(AnalysisError::InvalidIndex)?;
            let right_value = values.get(right).ok_or(AnalysisError::InvalidIndex)?;
            if left_value < right_value {
                child = right;
            }
        }
        let parent_value = values.get(root).ok_or(AnalysisError::InvalidIndex)?;
        let child_value = values.get(child).ok_or(AnalysisError::InvalidIndex)?;
        if parent_value >= child_value {
            break;
        }
        swap(values, root, child)?;
        root = child;
    }
    Ok(())
}

/// Linear interpolation at rank (p/100)(n-1), including both exact endpoints.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rank is finite and checked inside the nonnegative u32 extent before conversion"
)]
pub(super) fn at(sorted: &[f64], percentile: f64) -> Result<f64, AnalysisError> {
    if !percentile.is_finite() || !(0.0..=100.0).contains(&percentile) {
        return Err(AnalysisError::InvalidPercentile);
    }
    let last = sorted
        .len()
        .checked_sub(1)
        .ok_or(AnalysisError::InvalidSample)?;
    let last_count = u32::try_from(last).map_err(|_| AnalysisError::InvalidSample)?;
    let rank = finite((percentile / 100.0) * f64::from(last_count))?;
    if rank >= f64::from(last_count) {
        return finite(*sorted.get(last).ok_or(AnalysisError::InvalidIndex)?);
    }
    let integer = rank.floor();
    let index = usize::try_from(integer as u32).map_err(|_| AnalysisError::InvalidIndex)?;
    let fraction = rank - integer;
    let floor = *sorted.get(index).ok_or(AnalysisError::InvalidIndex)?;
    let ceiling = *sorted
        .get(index.checked_add(1).ok_or(AnalysisError::InvalidIndex)?)
        .ok_or(AnalysisError::InvalidIndex)?;
    finite(floor + finite(finite(ceiling - floor)? * fraction)?)
}
