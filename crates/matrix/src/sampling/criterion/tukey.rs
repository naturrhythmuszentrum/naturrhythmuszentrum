// SPDX-License-Identifier: AGPL-3.0-only AND MIT
//! Adapted from Criterion 0.8.2, revision
//! 7f0d745532e3c7b2e11bbf9de9b911f91790d3b1.
//! Upstream: `src/stats/univariate/outliers/tukey.rs`; items: classify, Label, `LabeledSample` fences/counts and iteration.
//! Modifications: finite f64 fences and owned public diagnostic records; no pruning.
//! Copyright (c) 2014 Jorge Aparicio; MIT option elected.
//! Attribution: crates/matrix/src/sampling/criterion/NOTICE;
//! upstream license: crates/matrix/LICENSE-CRITERION; nrz additions: LICENSE-CODE.

use super::{AnalysisError, TukeyFences, TukeyLabel, finite, percentiles};

/// Quartile fences use Criterion's 1.5 and 3 interquartile multipliers.
pub(super) fn fences(sorted: &[f64]) -> Result<TukeyFences, AnalysisError> {
    let first = percentiles::at(sorted, 25.0)?;
    let third = percentiles::at(sorted, 75.0)?;
    let spread = finite(third - first)?;
    let inner = finite(1.5 * spread)?;
    let outer = finite(3.0 * spread)?;
    Ok(TukeyFences {
        lower_outer: finite(first - outer)?,
        lower_inner: finite(first - inner)?,
        upper_inner: finite(third + inner)?,
        upper_outer: finite(third + outer)?,
    })
}

/// Fence equality is ordinary at inner fences and mild at outer fences.
pub(super) const fn label(value: f64, fences: TukeyFences) -> TukeyLabel {
    if value < fences.lower_outer {
        TukeyLabel::LowSevere
    } else if value > fences.upper_outer {
        TukeyLabel::HighSevere
    } else if value < fences.lower_inner {
        TukeyLabel::LowMild
    } else if value > fences.upper_inner {
        TukeyLabel::HighMild
    } else {
        TukeyLabel::NotAnOutlier
    }
}
