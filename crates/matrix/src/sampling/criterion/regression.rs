// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/stats/bivariate/mod.rs`; `src/stats/bivariate/regression.rs`; `src/stats/mod.rs`; items: `Data::new/iter`, `Slope::fit`, dot.
//! Modifications: validated paired row views, exact count conversions and finite checked accumulation.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use super::{AnalysisError, AnalysisPhase, Control, SampleRow, count, finite, positive};

/// Finite, positive elapsed times paired with nonzero exact iteration counts.
#[derive(Clone, Copy, Debug)]
pub(super) struct Data<'a>(&'a [SampleRow]);

impl<'a> Data<'a> {
    /// Validate the complete paired slice without allocating or unzipping.
    pub(super) fn new(
        rows: &'a [SampleRow],
        control: &mut Control<'_>,
    ) -> Result<Self, AnalysisError> {
        if rows.len() < 2 {
            return Err(AnalysisError::InvalidSample);
        }
        for (index, row) in rows.iter().enumerate() {
            control.check(AnalysisPhase::Validation)?;
            let valid_time = row.elapsed_seconds > 0.0;
            let finite_time = row.elapsed_seconds.is_finite();
            if !(valid_time && finite_time) {
                return Err(AnalysisError::InvalidTime { index });
            }
            if row.iterations == 0 || count(row.iterations).is_err() {
                return Err(AnalysisError::InvalidIterations { index });
            }
        }
        Ok(Self(rows))
    }

    /// The paired data preserve original iteration order and identity.
    pub(super) const fn rows(self) -> &'a [SampleRow] {
        self.0
    }

    /// Through-origin least squares: sum(k t) / sum(k squared).
    pub(super) fn slope(self, control: &mut Control<'_>) -> Result<f64, AnalysisError> {
        let mut sums = Sums::default();
        for row in self.0 {
            control.check(AnalysisPhase::Statistics)?;
            sums.add(*row)?;
        }
        sums.slope()
    }
}

/// Separate dot accumulators preserve Criterion's sequential summation order.
#[derive(Debug, Default)]
pub(super) struct Sums {
    /// Iteration/elapsed dot product.
    numerator: f64,
    /// Squared iteration-count dot product.
    denominator: f64,
}

impl Sums {
    /// Accumulate one entire pair; resampling cannot separate its components.
    pub(super) fn add(&mut self, row: SampleRow) -> Result<(), AnalysisError> {
        let iterations = count(row.iterations)?;
        self.numerator = finite(self.numerator + finite(iterations * row.elapsed_seconds)?)?;
        self.denominator = finite(self.denominator + finite(iterations * iterations)?)?;
        Ok(())
    }

    /// A zero regression denominator is distinct from arithmetic overflow.
    pub(super) fn slope(&self) -> Result<f64, AnalysisError> {
        if self.denominator <= 0.0 {
            return Err(AnalysisError::ZeroRegressionDenominator);
        }
        positive(finite(self.numerator / self.denominator)?)
    }
}
