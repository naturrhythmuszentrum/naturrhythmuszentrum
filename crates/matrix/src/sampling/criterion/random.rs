// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/stats/rand_util.rs`; `src/stats/univariate/resamples.rs`; `src/stats/bivariate/resamples.rs`; items: `Rng/new_rng` and uniform resample index selection.
//! Modifications: sampler-owned explicit seed and bounded cancellable oorandom-compatible multiply-high rejection.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Index selection also adapts oorandom 11.1.5 `src/lib.rs`, `Rand64::rand_range`.
//! Copyright (c) 2019 Simon Heath; MIT notice: crates/matrix/LICENSE-OORANDOM.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use super::{AnalysisError, AnalysisPhase, Control};

/// Independent index source enables explicit-sequence statistical oracles.
pub(super) trait Indices {
    /// Draw uniformly from the nonempty index range.
    fn next(&mut self, upper: usize, control: &mut Control<'_>) -> Result<usize, AnalysisError>;
}

/// A generator belongs to one analysis call and never reads a clock.
#[derive(Debug)]
pub(super) struct SeededIndices(oorandom::Rand64);

impl SeededIndices {
    /// Initialize Criterion's generator with the recorded seed.
    pub(super) fn new(seed: u128) -> Self {
        Self(oorandom::Rand64::new(seed))
    }
}

impl Indices for SeededIndices {
    fn next(&mut self, upper: usize, control: &mut Control<'_>) -> Result<usize, AnalysisError> {
        uniform(upper, || self.0.rand_u64(), control)
    }
}

/// Lemire selection as in oorandom 11.1.5, with at most 128 attempts per index.
/// Rejected low products never bias the returned distribution; exhausting the
/// allowance returns an error rather than substituting a biased index.
pub(super) fn uniform(
    upper: usize,
    mut random: impl FnMut() -> u64,
    control: &mut Control<'_>,
) -> Result<usize, AnalysisError> {
    let upper = u64::try_from(upper).map_err(|_| AnalysisError::InvalidIndex)?;
    if upper == 0 {
        return Err(AnalysisError::InvalidIndex);
    }
    let threshold = upper.wrapping_neg() % upper;
    for _ in 0..128 {
        control.check(AnalysisPhase::Resampling)?;
        // A product of two u64 values always fits in u128.
        let product = u128::from(random()) * u128::from(upper);
        let leftover = u64::try_from(product & u128::from(u64::MAX))
            .map_err(|_| AnalysisError::InvalidIndex)?;
        if leftover >= threshold {
            return usize::try_from(product >> 64).map_err(|_| AnalysisError::InvalidIndex);
        }
    }
    Err(AnalysisError::IndexRejectionLimit)
}
