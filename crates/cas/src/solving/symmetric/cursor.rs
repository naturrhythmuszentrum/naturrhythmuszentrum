// SPDX-License-Identifier: AGPL-3.0-only
//! Degree-generic cyclic powers followed by lexicographic restricted-growth partitions.

use super::AT;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::solving::coefficients::storage::{copied, invalid, vector};

/// Candidate descriptions contain no coefficient formula or degree-specific identity.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Seed {
    /// The k-th cyclic character raised to a power no larger than its character order.
    Cyclic {
        /// Nontrivial character index in increasing order.
        index: u32,
        /// Offered positive power bounded by the character order.
        power: u32,
    },
    /// Sum of block products for a canonical restricted-growth string.
    Partition(Vec<u32>),
}

/// An explicit continuation cursor avoids preallocating factorial or Bell-size inventories.
#[derive(Debug)]
pub(crate) struct Cursor {
    /// Number of formal root slots.
    degree: u32,
    /// Next nontrivial character.
    index: u32,
    /// Next offered power of that character.
    power: u32,
    /// One live partition, or exhausted enumeration.
    partition: Option<Vec<u32>>,
}

/// Small integer gcd uses a bounded, charged Euclidean route.
pub(crate) fn order(degree: u32, index: u32, budget: &mut Budget) -> Result<u32, Fault> {
    let mut left = degree;
    let mut right = index;
    while right != 0 {
        budget.solving(BudgetKind::GcdSteps, 1, AT)?;
        (left, right) = (right, left % right);
    }
    degree.checked_div(left).ok_or_else(invalid)
}

/// Increment one restricted-growth string in lexicographic order, with zeroed suffix.
fn advance(partition: &mut [u32], budget: &mut Budget) -> Result<bool, Fault> {
    for slot in (1..partition.len()).rev() {
        budget.work(crate::graph::size(slot, AT)?.max(1), AT)?;
        let bound = partition
            .get(..slot)
            .ok_or_else(invalid)?
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(invalid)?;
        let value = partition.get_mut(slot).ok_or_else(invalid)?;
        if *value < bound {
            *value += 1;
            partition.get_mut(slot + 1..).ok_or_else(invalid)?.fill(0);
            return Ok(true);
        }
    }
    Ok(false)
}

impl Cursor {
    /// Bound the variable frame before allocating the one live partition cursor.
    pub(crate) fn new(degree: u32, budget: &mut Budget) -> Result<Self, Fault> {
        if degree == 0 {
            return Err(invalid());
        }
        budget.check(BudgetKind::PolynomialVariables, u64::from(degree), AT)?;
        let width = usize::try_from(degree).map_err(|_| invalid())?;
        let mut partition = vector(width, budget)?;
        partition.resize(width, 0);
        Ok(Self {
            degree,
            index: 1,
            power: 1,
            partition: Some(partition),
        })
    }

    /// Offer increasing character indices and powers, then every bounded partition exactly once.
    pub(crate) fn next(&mut self, budget: &mut Budget) -> Result<Option<Seed>, Fault> {
        budget.work(1, AT)?;
        if self.index < self.degree {
            let bound = order(self.degree, self.index, budget)?;
            let seed = Seed::Cyclic {
                index: self.index,
                power: self.power,
            };
            if self.power == bound {
                self.index += 1;
                self.power = 1;
            } else {
                self.power += 1;
            }
            return Ok(Some(seed));
        }
        let Some(partition) = &mut self.partition else {
            return Ok(None);
        };
        let seed = Seed::Partition(copied(partition, budget)?);
        if !advance(partition, budget)? {
            self.partition = None;
        }
        Ok(Some(seed))
    }
}
