// SPDX-License-Identifier: AGPL-3.0-only
//! Checked fixed-width planning selects work; it supplies no analytic evidence.

use super::super::{RefinementPolicy, log::Plan};

/// Exact sum of floor-log-two over the positive integers through m.
pub(in crate::enclosure) fn factorial_bits(m: u32) -> Option<u64> {
    if m == 0 {
        return None;
    }
    let r = m.ilog2();
    Some(u64::from(r) * (u64::from(m) + 1) + 2 - (1_u64 << (r + 1)))
}

/// Bound the first omitted term by the universal reduced argument bound a <= 1/2.
pub(in crate::enclosure) fn plan(policy: RefinementPolicy, reduction: u32) -> Option<Plan> {
    if policy.rounds == 0 || reduction > policy.reduction_ceiling {
        return None;
    }
    let magnitude = 1_u32.checked_shl(reduction)?;
    let target = policy
        .precision
        .checked_add(4)?
        .checked_add(reduction)?
        .checked_add(magnitude)?;
    let threshold = u64::from(target.checked_add(1)?);
    let enough = |m: u32| factorial_bits(m).is_some_and(|f| u64::from(m) + f >= threshold);
    let mut upper = policy.term_ceiling.checked_add(1)?;
    if upper < 2 || !enough(upper) {
        return None;
    }
    let mut lower = 2;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if enough(middle) {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    let count = lower.checked_add(reduction)?.checked_add(1)?;
    let grid = target
        .checked_add(4)?
        .checked_add((count - 1).bit_width())?;
    (grid <= policy.grid_ceiling).then_some(Plan {
        grid,
        terms: lower - 1,
        reduction,
    })
}
