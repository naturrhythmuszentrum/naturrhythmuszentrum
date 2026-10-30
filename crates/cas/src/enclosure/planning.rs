// SPDX-License-Identifier: AGPL-3.0-only
//! Family schedules share a working grid while retaining their own term counts.

use super::{RefinementPolicy, log::Plan};

/// Preflighted analytic schedules, absent for exact rational and root-only requests.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Plans {
    /// Atanh schedule for logarithms.
    pub log: Option<Plan>,
    /// Taylor schedule for exponentials.
    pub exp: Option<Plan>,
}

impl Plans {
    /// Fixed-width resident metadata determines the initial cost choices.
    pub(super) fn new(target: &crate::graph::Expression, policy: RefinementPolicy) -> Option<Self> {
        let log = if target.enclosure_logs {
            Some(super::log::plan(
                policy,
                target
                    .enclosure_log_bound
                    .unwrap_or(policy.reduction_ceiling),
            )?)
        } else {
            None
        };
        let exp = if target.enclosure_exp.present {
            Some(super::exp::plan(
                policy,
                target
                    .enclosure_exp
                    .reduction
                    .unwrap_or(policy.reduction_ceiling),
            )?)
        } else {
            None
        };
        Some(Self { log, exp })
    }

    /// Every analytic operation uses the same enclosing transcript grid.
    pub(super) fn grid(self, base: u32) -> u32 {
        [self.log, self.exp]
            .into_iter()
            .flatten()
            .fold(base, |grid, plan| grid.max(plan.grid))
    }

    /// Retain each degree while using the maximum required working precision.
    pub(super) fn at_grid(self, grid: u32) -> Self {
        Self {
            log: self.log.map(|p| Plan { grid, ..p }),
            exp: self.exp.map(|p| Plan { grid, ..p }),
        }
    }

    /// Doubling is permitted only after a completed width miss.
    pub(super) fn next(self, policy: RefinementPolicy) -> Option<Self> {
        Some(Self {
            log: match self.log {
                Some(p) => Some(p.next(policy)?),
                None => None,
            },
            exp: match self.exp {
                Some(p) => Some(p.next(policy)?),
                None => None,
            },
        })
    }
}
