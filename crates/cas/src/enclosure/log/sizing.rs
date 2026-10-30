// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed-width sizing is a choice of work, not evidence about an analytic remainder.

use super::super::RefinementPolicy;

/// Initial or doubled bounded analytic schedule.
#[derive(Clone, Copy, Debug)]
pub(in crate::enclosure) struct Plan {
    /// Working dyadic exponent.
    pub grid: u32,
    /// Separate series use this conservative ceiling unless the series is exactly skipped.
    pub terms: u32,
    /// Bound on the magnitude of each binary reduction exponent.
    pub reduction: u32,
}

/// Exact ceiling of a base-two logarithm, with no floating estimate or shift.
pub(super) const fn ceil_log2(value: u32) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some((value - 1).bit_width())
    }
}

/// Use three bits per included term and the signed-combination amplification.
pub(in crate::enclosure) fn plan(policy: RefinementPolicy, reduction: u32) -> Option<Plan> {
    if policy.rounds == 0 || reduction > policy.reduction_ceiling {
        return None;
    }
    let amplification = ceil_log2(reduction.checked_add(1)?)?;
    let target = policy
        .precision
        .checked_add(4)?
        .checked_add(amplification)?;
    let terms = target.checked_add(2)?.checked_div(3)?.max(1);
    let grid = target
        .checked_add(4)?
        .checked_add(ceil_log2(terms.checked_add(1)?)?)?;
    if terms > policy.term_ceiling || grid > policy.grid_ceiling {
        return None;
    }
    Some(Plan {
        grid,
        terms,
        reduction,
    })
}

impl Plan {
    /// Double only after a missed bound or an unseparated computed domain.
    pub(in crate::enclosure) fn next(self, policy: RefinementPolicy) -> Option<Self> {
        let grid = self
            .grid
            .checked_sub(policy.precision)?
            .checked_mul(2)?
            .checked_add(policy.precision)?;
        let terms = self.terms.checked_mul(2)?;
        (grid <= policy.grid_ceiling && terms <= policy.term_ceiling).then_some(Self {
            grid,
            terms,
            ..self
        })
    }
}
