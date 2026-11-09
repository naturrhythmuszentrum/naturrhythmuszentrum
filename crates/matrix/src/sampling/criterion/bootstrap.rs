// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/stats/univariate/sample.rs`; `src/stats/univariate/resamples.rs`; `src/stats/bivariate/mod.rs`; `src/stats/bivariate/resamples.rs`; items: sequential single-sample and paired bootstrap; `Resamples::new/next`.
//! Modifications: concrete fallible distributions, reusable buffers, injectable indices and cancellation.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use super::{AnalysisError, AnalysisPhase, Control, Memory, push};
use super::{
    random::Indices,
    regression::{Data, Sums},
    sample::Sample,
};

/// Four univariate distributions and the independently paired slope bootstrap.
#[derive(Debug)]
pub(super) struct Distributions {
    /// Arithmetic mean for each univariate resample.
    pub(super) mean: Vec<f64>,
    /// Sample standard deviation for each univariate resample.
    pub(super) deviation: Vec<f64>,
    /// Median for each univariate resample.
    pub(super) median: Vec<f64>,
    /// Scaled MAD for each univariate resample.
    pub(super) mad: Vec<f64>,
    /// Through-origin fit for each paired resample.
    pub(super) slope: Vec<f64>,
}

impl Distributions {
    /// Admit every distribution before ordinary bootstrap work starts.
    pub(super) fn new(
        length: usize,
        memory: &mut Memory,
        control: &mut Control<'_>,
    ) -> Result<Self, AnalysisError> {
        Ok(Self {
            mean: memory.reserve(length, control)?,
            deviation: memory.reserve(length, control)?,
            median: memory.reserve(length, control)?,
            mad: memory.reserve(length, control)?,
            slope: memory.reserve(length, control)?,
        })
    }

    /// Complete all single-sample draws before starting the paired bootstrap.
    pub(super) fn univariate(
        &mut self,
        sample: &[f64],
        count: usize,
        scratch: (&mut Vec<f64>, &mut Vec<f64>),
        indices: &mut impl Indices,
        control: &mut Control<'_>,
    ) -> Result<(), AnalysisError> {
        let (stage, sorted) = scratch;
        for _ in 0..count {
            control.check(AnalysisPhase::Resampling)?;
            resample(sample, stage, indices, control)?;
            let [mean, deviation, median, mad] =
                Sample::new(stage, control)?.statistics(sorted, control)?;
            push(&mut self.mean, mean)?;
            push(&mut self.deviation, deviation)?;
            push(&mut self.median, median)?;
            push(&mut self.mad, mad)?;
        }
        Ok(())
    }

    /// Every draw selects both count and elapsed time from the same row.
    pub(super) fn paired(
        &mut self,
        data: Data<'_>,
        count: usize,
        indices: &mut impl Indices,
        control: &mut Control<'_>,
    ) -> Result<(), AnalysisError> {
        for _ in 0..count {
            control.check(AnalysisPhase::Resampling)?;
            let mut sums = Sums::default();
            for _ in data.rows() {
                control.check(AnalysisPhase::Resampling)?;
                let index = indices.next(data.rows().len(), control)?;
                let row = data.rows().get(index).ok_or(AnalysisError::InvalidIndex)?;
                sums.add(*row)?;
            }
            push(&mut self.slope, sums.slope()?)?;
        }
        Ok(())
    }
}

/// Reuse the admitted stage without growing it or silently shortening a draw.
fn resample(
    values: &[f64],
    stage: &mut Vec<f64>,
    indices: &mut impl Indices,
    control: &mut Control<'_>,
) -> Result<(), AnalysisError> {
    stage.clear();
    for _ in values {
        control.check(AnalysisPhase::Resampling)?;
        let index = indices.next(values.len(), control)?;
        push(
            stage,
            *values.get(index).ok_or(AnalysisError::InvalidIndex)?,
        )?;
    }
    Ok(())
}
